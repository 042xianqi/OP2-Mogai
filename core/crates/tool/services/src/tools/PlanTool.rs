// PlanTool —— 任务计划看板工具（移植自太墟 harness 的 plan 工具契约）
//
// 设计取舍：
//   plan 不进入 BuiltinToolName / ToolResultMap，而是走 registerTool 注册为普通公开工具。
//   原因：core/crates/tool/services/build.rs 的 check_tool_registration_contract 强制
//   ToolRegistration 与 SDK 的 ToolResultMap 严格一一对应，新增内置工具需要同步改
//   tool_types.rs（会牵动 TS 声明与 codegen）。走普通工具注册可零侵入接入。
//
// 状态存储：无状态。
//   Operit2 已把 tool_call 参数结构化持久化在 MessagePart{kind:ToolCall, toolName, attributes}，
//   Dart 侧可直接从消息流回放 plan 调用序列派生当前看板，无需建表、无需 migration。

use std::collections::BTreeMap;

use operit_tools::tools::AIToolHandler::{AIToolHandler, FnToolExecutor};
use operit_tools::tools::ToolResultDataClasses::stringResultData;
use operit_tools::ConversationMarkupManager::ToolResult;
use operit_tools::ToolExecutionManager::{AITool, ToolEffect, ToolValidationResult};

/// 计划生命周期状态（与太墟 plan 工具契约一致）。
const PLAN_LIFECYCLE_STATUSES: [&str; 3] = ["active", "completed", "cancelled"];

/// 单个步骤的合法状态（与太墟一致）。
const PLAN_STEP_STATUSES: [&str; 4] = ["pending", "in_progress", "completed", "failed"];

/// 工具对外名称。
pub const PLAN_TOOL_NAME: &str = "plan";

fn arguments_of(tool: &AITool) -> BTreeMap<String, String> {
    tool.parameters
        .iter()
        .map(|parameter| (parameter.name.clone(), parameter.value.clone()))
        .collect()
}

/// 把模型给的 steps 解析成规范化的 JSON 文本。
///
/// 容错范围（对齐太墟 plan 对模型输出不稳定的防御）：
/// - 规范对象数组、别名字段、布尔完成位、纯字符串数组、{"steps":[...]} 包裹
/// - 非 JSON 的 markdown 列表按行拆
/// - 以 `{` / `[` 开头却解析失败 → 坏数据，降级空列表（绝不把残片当步骤）
#[allow(non_snake_case)]
pub fn normalize_steps(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return "[]".to_string();
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "[]".to_string();
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(serde_json::Value::Array(items)) => normalize_step_array(items),
        Ok(serde_json::Value::Object(object)) => match object.get("steps") {
            Some(serde_json::Value::Array(items)) => normalize_step_array(items.clone()),
            _ => "[]".to_string(),
        },
        _ => {
            // 本该是 JSON 却坏了：不能把 "{oops" 这类残片当成一条步骤。
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                return "[]".to_string();
            }
            let items: Vec<serde_json::Value> = trimmed
                .lines()
                .map(|line| {
                    line.trim()
                        .trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace())
                        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')')
                        .trim()
                        .to_string()
                })
                .filter(|line| !line.is_empty())
                .map(serde_json::Value::String)
                .collect();
            normalize_step_array(items)
        }
    }
}

#[allow(non_snake_case)]
fn normalize_step_array(items: Vec<serde_json::Value>) -> String {
    let normalized: Vec<serde_json::Value> = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| match item {
            serde_json::Value::String(title) => serde_json::json!({
                "id": (index + 1).to_string(),
                "title": title,
                "status": "pending",
            }),
            serde_json::Value::Object(object) => {
                let title = [
                    "title", "step", "name", "description", "desc", "task", "content", "action",
                ]
                .iter()
                .find_map(|key| object.get(*key).and_then(|value| value.as_str()))
                .unwrap_or("")
                .to_string();
                let is_done = ["completed", "isCompleted", "is_completed", "done", "finished"]
                    .iter()
                    .any(|key| object.get(*key).and_then(|value| value.as_bool()) == Some(true));
                let status = object
                    .get("status")
                    .or_else(|| object.get("state"))
                    .and_then(|value| value.as_str())
                    .map(|value| value.trim().to_lowercase())
                    .filter(|value| PLAN_STEP_STATUSES.contains(&value.as_str()))
                    .unwrap_or_else(|| {
                        if is_done {
                            "completed".to_string()
                        } else {
                            "pending".to_string()
                        }
                    });
                let id = object
                    .get("id")
                    .map(|value| match value {
                        serde_json::Value::String(text) => text.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_else(|| (index + 1).to_string());
                serde_json::json!({ "id": id, "title": title, "status": status })
            }
            _ => serde_json::Value::Null,
        })
        .filter(|value| !value.is_null())
        .collect();
    serde_json::Value::Array(normalized).to_string()
}

/// 归一化 action 名，兼容模型常见的别名写法。
#[allow(non_snake_case)]
pub fn plan_action(arguments: &BTreeMap<String, String>) -> String {
    let raw = arguments
        .get("action")
        .map(|value| value.trim().to_lowercase())
        .unwrap_or_default();
    match raw.as_str() {
        "" => "get_active".to_string(),
        "set_active" | "create" => "replace_active".to_string(),
        "get" => "get_active".to_string(),
        "update_steps" => "advance".to_string(),
        "clear" => "clear_active".to_string(),
        other => other.to_string(),
    }
}

/// 执行 plan 工具，返回 (success, 回显文本)。
#[allow(non_snake_case)]
pub fn execute_plan(arguments: &BTreeMap<String, String>) -> (bool, String) {
    match plan_action(arguments).as_str() {
        "replace_active" => {
            let goal = arguments.get("goal").map(|value| value.trim()).unwrap_or("");
            if goal.is_empty() {
                return (false, "plan replace_active 必须提供 goal 目标描述".to_string());
            }
            let steps = normalize_steps(arguments.get("steps").map(|value| value.as_str()));
            (
                true,
                format!(
                    "已成功创建/更新任务执行规划：\n目标：{goal}\n步骤：{steps}\n\n\
                     提示：请按规划推进执行。每个步骤完成时调用 \
                     plan(action=\"advance\", steps=[...]) 更新状态；遇到阻碍时用 \
                     replace_active 调整方案；全部完成时调用 clear_active。"
                ),
            )
        }
        "get_active" => (
            true,
            "当前计划状态请以本会话此前的 plan 调用记录为准（见上方 replace_active / advance 回显）。"
                .to_string(),
        ),
        "advance" => {
            let steps = normalize_steps(arguments.get("steps").map(|value| value.as_str()));
            if steps == "[]" {
                return (
                    false,
                    "plan advance 必须提供 steps（完整步骤列表，含最新 status），否则无法更新看板"
                        .to_string(),
                );
            }
            let requested = arguments
                .get("status")
                .map(|value| value.trim().to_lowercase())
                .unwrap_or_default();
            // 顶层状态做白名单收敛：模型写自然语言进去不能污染生命周期状态机。
            let new_status = if PLAN_LIFECYCLE_STATUSES.contains(&requested.as_str()) {
                requested
            } else {
                "active".to_string()
            };
            (
                true,
                format!("【规划进度已同步更新】\n状态：{new_status}\n当前步骤：{steps}\n请继续推进后续任务。"),
            )
        }
        "clear_active" => (true, "已清空当前会话的任务规划".to_string()),
        other => (false, format!("未知的 plan 动作: {other}")),
    }
}

/// 构造 plan 工具的 executor。
#[allow(non_snake_case)]
pub fn build_plan_executor() -> FnToolExecutor {
    FnToolExecutor {
        // plan 只承载会话内的规划文本，不触碰文件系统与设备。
        effect: ToolEffect::READ,
        validate: std::sync::Arc::new(|_tool: &AITool| ToolValidationResult {
            valid: true,
            errorMessage: String::new(),
        }),
        invoke: std::sync::Arc::new(|tool: &AITool| {
            let arguments = arguments_of(tool);
            let (success, output) = execute_plan(&arguments);
            ToolResult {
                toolName: tool.name.clone(),
                success,
                result: stringResultData(output.clone()),
                error: if success { None } else { Some(output) },
            }
        }),
    }
}

/// 在 handler 上注册 plan 工具（普通公开工具，绕开 BuiltinToolName 契约）。
#[allow(non_snake_case)]
pub fn registerPlanTool(handler: &mut AIToolHandler) {
    handler.registerTool(PLAN_TOOL_NAME.to_string(), Box::new(build_plan_executor()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn replace_active_requires_goal() {
        let (ok, message) = execute_plan(&args(&[("action", "replace_active")]));
        assert!(!ok);
        assert!(message.contains("必须提供 goal"));
    }

    #[test]
    fn replace_active_accepts_canonical_steps() {
        let (ok, message) = execute_plan(&args(&[
            ("action", "create"),
            ("goal", "修好自动压缩"),
            (
                "steps",
                r#"[{"id":"1","title":"读代码","status":"completed"},{"id":"2","title":"改触发","status":"in_progress"}]"#,
            ),
        ]));
        assert!(ok);
        assert!(message.contains("修好自动压缩"));
        assert!(message.contains("\"status\":\"in_progress\""));
    }

    #[test]
    fn plain_string_steps_become_pending() {
        let out = normalize_steps(Some(r#"["甲","乙"]"#));
        assert!(out.contains("\"title\":\"甲\""));
        assert!(out.contains("\"status\":\"pending\""));
    }

    #[test]
    fn wrapped_steps_are_unwrapped() {
        let out = normalize_steps(Some(r#"{"goal":"x","steps":["甲"]}"#));
        assert!(out.contains("\"title\":\"甲\""));
    }

    #[test]
    fn advance_requires_steps() {
        let (ok, _) = execute_plan(&args(&[("action", "advance")]));
        assert!(!ok);
    }

    #[test]
    fn advance_whitelists_lifecycle_status() {
        let (ok, message) = execute_plan(&args(&[
            ("action", "update_steps"),
            ("steps", r#"[{"id":"1","title":"A","status":"completed"}]"#),
            ("status", "Rust 37 项已过，进入 APK 构建"),
        ]));
        assert!(ok);
        assert!(message.contains("状态：active"), "got: {message}");
    }

    #[test]
    fn broken_json_degrades_to_empty() {
        assert_eq!(normalize_steps(Some("{oops")), "[]");
        assert_eq!(normalize_steps(Some("[1,")), "[]");
        assert_eq!(normalize_steps(None), "[]");
    }

    #[test]
    fn markdown_lines_are_parsed() {
        let out = normalize_steps(Some("- 已完成项\n- 未完成项\n2. 编号项"));
        assert!(out.contains("\"title\":\"已完成项\""));
        assert!(out.contains("\"title\":\"未完成项\""));
        assert!(out.contains("\"title\":\"编号项\""));
    }

    #[test]
    fn boolean_done_and_alias_keys() {
        let out = normalize_steps(Some(r#"[{"step":"甲","done":true}]"#));
        assert!(out.contains("\"status\":\"completed\""));
        assert!(out.contains("\"title\":\"甲\""));
    }

    #[test]
    fn clear_and_unknown_actions() {
        assert!(execute_plan(&args(&[("action", "clear")])).0);
        let (ok, message) = execute_plan(&args(&[("action", "wat")]));
        assert!(!ok);
        assert!(message.contains("未知的 plan 动作"));
    }

    #[test]
    fn action_aliases_normalize() {
        assert_eq!(plan_action(&args(&[("action", "create")])), "replace_active");
        assert_eq!(plan_action(&args(&[("action", "get")])), "get_active");
        assert_eq!(plan_action(&args(&[("action", "update_steps")])), "advance");
        assert_eq!(plan_action(&args(&[("action", "clear")])), "clear_active");
        assert_eq!(plan_action(&args(&[])), "get_active");
    }
}
