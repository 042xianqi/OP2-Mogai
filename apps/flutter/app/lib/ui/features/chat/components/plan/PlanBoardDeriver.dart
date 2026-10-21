// ignore_for_file: file_names
// apps/flutter/app/lib/ui/features/chat/components/plan/PlanBoardDeriver.dart
//
// 从已持久化的消息流里派生「当前活跃计划」。
//
// 为什么不建表（方案 A）：
//   Operit2 的 tool_call part 已经把参数持久化了 ——
//   operit_model::MessagePart { kind: ToolCall, toolName, attributes: BTreeMap<String,String> }
//   （core/crates/foundation/model/src/MessagePart.rs:17-26，Dart 侧镜像为
//    core_proxy.MessagePart，见 StructuredMessagePartRenderer.dart:177-196）
//   所以不需要新表 / 不需要 proxy 方法 / 不需要 MIGRATION，重启后天然恢复。
//
// 派生规则（与太墟 executePlan 的写入语义一一对应）：
//   - 从后往前扫 tool_call part，toolName == 'plan'
//   - 命中 replace_active / create / set_active → 用其 goal+steps 建计划（覆盖）
//   - 命中 advance / update_steps            → 用其 steps 更新，goal 沿用上一次
//   - 命中 clear_active / clear             → 计划清空，停止扫描
//   - 顶层 status == completed / cancelled   → 计划视为已结束，不显示
//   - 同一 plan 调用若被重试（同 toolCallId）只算一次

import '../../../../../core/proxy/generated/CoreProxyModels.g.dart'
    as core_proxy;
import 'PlanStepParser.dart';

/// 一份活跃计划。
class ActivePlan {
  const ActivePlan({
    required this.goal,
    required this.steps,
    required this.status,
  });

  final String goal;
  final List<PlanStepUi> steps;

  /// 计划生命周期：active / completed / cancelled。
  final String status;
}

/// 从消息流派生活跃计划；无计划返回 null。
class PlanBoardDeriver {
  PlanBoardDeriver._();

  /// plan 工具在 Operit2 侧注册的工具名。
  static const String toolName = 'plan';

  static const Set<String> _replaceActions = <String>{
    'replace_active',
    'set_active',
    'create',
  };
  static const Set<String> _advanceActions = <String>{
    'advance',
    'update_steps',
  };
  static const Set<String> _clearActions = <String>{'clear_active', 'clear'};

  /// 逆序扫描消息，遇到最后一个改变计划的 plan 调用即为当前状态。
  ///
  /// [messages] 传 `ChatUiMessage` 列表（= core_proxy.ChatMessage）。
  /// 只看 assistant 消息的 tool_call part，成本是 O(消息数)，不需要额外索引。
  static ActivePlan? derive(List<core_proxy.ChatMessage> messages) {
    final Set<String> seenCallIds = <String>{};

    for (int index = messages.length - 1; index >= 0; index--) {
      final core_proxy.ChatMessage message = messages[index];
      final List<core_proxy.MessagePart> parts = message.parts;
      // part 在存储里按 sequence 排序，倒序扫以取「最后一次调用」
      for (int partIndex = parts.length - 1; partIndex >= 0; partIndex--) {
        final core_proxy.MessagePart part = parts[partIndex];
        if (part.kind != core_proxy.MessagePartKind.toolCall) continue;
        if (part.toolName != toolName) continue;

        final String? callId = part.toolCallId;
        if (callId != null && !seenCallIds.add(callId)) continue;

        final Map<String, String> args = part.attributes;
        final String action = (args['action'] ?? '').trim().toLowerCase();

        if (_clearActions.contains(action)) {
          return null;
        }
        if (_replaceActions.contains(action)) {
          final String goal = (args['goal'] ?? '').trim();
          final String status = _normalizeLifecycle(args['status']);
          if (status == 'completed' || status == 'cancelled') return null;
          final List<PlanStepUi> steps = PlanStepParser.parse(args['steps']);
          if (steps.isEmpty && goal.isEmpty) continue;
          return ActivePlan(goal: goal, steps: steps, status: status);
        }
        if (_advanceActions.contains(action)) {
          final String status = _normalizeLifecycle(args['status']);
          if (status == 'completed' || status == 'cancelled') return null;
          final List<PlanStepUi> steps = PlanStepParser.parse(args['steps']);
          if (steps.isEmpty) continue;
          // advance 不带 goal，需向前回溯找最近一次 replace_active 的 goal
          return ActivePlan(
            goal: _backfillGoal(messages, index),
            steps: steps,
            status: status,
          );
        }
        // 未知动作（含 get_active）不影响计划状态，继续往前扫
      }
    }
    return null;
  }

  /// 往前找最近一次 replace_active 的 goal，供 advance 显示标题用。
  static String _backfillGoal(
    List<core_proxy.ChatMessage> messages,
    int fromIndex,
  ) {
    for (int index = fromIndex; index >= 0; index--) {
      final List<core_proxy.MessagePart> parts = messages[index].parts;
      for (int partIndex = parts.length - 1; partIndex >= 0; partIndex--) {
        final core_proxy.MessagePart part = parts[partIndex];
        if (part.kind != core_proxy.MessagePartKind.toolCall) continue;
        if (part.toolName != toolName) continue;
        final String action = (part.attributes['action'] ?? '')
            .trim()
            .toLowerCase();
        if (!_replaceActions.contains(action)) continue;
        final String goal = (part.attributes['goal'] ?? '').trim();
        if (goal.isNotEmpty) return goal;
      }
    }
    return '';
  }

  /// 顶层 status 白名单化：与 Rust 侧 PLAN_LIFECYCLE_STATUSES 同口径。
  static String _normalizeLifecycle(String? raw) {
    final String value = (raw ?? '').trim().toLowerCase();
    if (value == 'completed' || value == 'cancelled' || value == 'active') {
      return value;
    }
    return 'active';
  }
}
