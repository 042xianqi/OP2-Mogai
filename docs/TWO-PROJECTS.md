# 两个独立项目说明（重要，勿混淆）

本文档用于防止后续任何 AI/协作者把两个项目当成同一个而误做「合并」。

---

## 一句话结论

**这是两个完全独立的项目，不能合并，也不需要合并。**

| 编号 | 项目 | 包名 | 技术栈 | 状态 |
|---|---|---|---|---|
| **版本一** | 用户魔改的 1.12.2 | `com.ai.assistance.operit` | Kotlin + Jetpack Compose | **已停更，不再开发** |
| **版本二** | 官方 Operit2 2.0 内测 | `app.operit`（官方）/ `app.operit.mogai`（共存魔改） | Dart/Flutter + Rust core | **当前主线，官方仍在快速迭代** |

版本一与版本二的代码没有任何继承关系：一个是 Kotlin Compose，一个是 Flutter + Rust。把版本一的代码「合并」进版本二在技术上不成立，只能**在版本二里重新实现**同名功能。

---

## 版本一：用户魔改的 1.12.2（已停更）

- 上游基线：Operit v1.12.2（`versionCode=51`）
- 源码路径（历史）：proot `/opt/op1moga`
- 分支：`mogai/v1`、`mogai/v1-build`、`mogai/v1-upstream`
- 远端历史分支：GitHub `042xianqi/Operit2` 的 `mogai/build`

### 版本一实际做过的魔改（Kotlin/Compose）

| 功能 | 主要文件 |
|---|---|
| 底栏 4 tab（对齐太墟范式） | `OperitBottomBar.kt`（173 行）— **后被要求取消** |
| 皮肤 6 套（青澜/墨黑/松绿/空紫/晓橙/玄青） | `OperitSkins.kt`（265 行） |
| 皮肤不生效修复（`operit_skin_id` 未进白名单） | `UserPreferencesManager.kt` |
| 终端时区系统级注入 | `LocalTerminalProvider.buildEnvironment()` |
| 删除时钟按钮 + 后续按钮左移 | `ChatHeader.kt` |
| 对话历史并入抽屉 | `DrawerContent.kt` + `DrawerChatHistorySection` |
| plan 工具 + `complete_step` 简写动作 | `AgentContextExecutor.kt`（+89） |
| 结构化任务规划看板 | `PlanBoard.kt`（328 行，含 `PlanStepParser`） |
| 自动上下文压缩修复（`maxTokens:0` 死代码） | `MessageCoordinationDelegate` / `EnhancedAIService` |

> 这些是**版本一的成果**，仅作为设计参考存在。它们**不属于**版本二，也不应被搬进版本二的仓库。

---

## 版本二：官方 Operit2 2.0（当前主线）

- 上游：`AAswordman/Operit2`（AGPL-3.0），Flutter + Rust
- 本地源码：proot `/root/op2/repo`
- 魔改分支：`mogai/v2-build`
- 共存包名：`app.operit.mogai`

### 版本二至今只改了两块：**群组** + **签名/共存**

这一点是用户明确确认的，也是本文档最重要的记录。

#### ① 群组（group-execution）

提交 `959d851b` — `fix(group-execution): drop targetRuntime=main so IPC routes to local registry`

改动文件（只 2 行）：
```
plugins/packages/buildin/character_cards/src/group-execution/control.ts
plugins/packages/buildin/character_cards/dist/ui/group-execution/index.ui.js
```

改动实质：去掉 `targetRuntime=main`，让群组执行的 IPC 走本地 registry，而不是路由到 main runtime。

#### ② 签名 / 共存

| 提交 | 内容 |
|---|---|
| `045898ee` | `applicationId` 从 `app.operit` 改为 `app.operit.mogai`，实现与官方版并存。`namespace` 保持 `app.operit` 不变（决定 Kotlin 类路径），只有 `applicationId` 决定安装身份与数据目录 |
| `e01bf0be` | 应用显示名加标识，区分魔改版（`AndroidManifest.xml`、`main.dart`、`AboutOperitScreen.dart`） |
| `481321ac` | NDK 版本可用 `OPERIT_NDK_VERSION` 环境变量覆盖（本地 ARM64 PRoot 只装了 r29）；补 `build-env.sh` / `do-build.sh` |

签名材料（自签，无法覆盖官方包）：
```
keystore : /root/op2/keys/mogai.jks   (PKCS12, 2710 B)
alias    : mogai
storePwd : mogai2026
keyPwd   : mogai2026
SHA-256  : 84:4A:41:3E:FB:F4:88:A7:11:BA:CA:46:D2:82:76:EB:BE:2B:C5:CC:A8:CC:7D:07:56:2C:59:7D:83:96:10:5C
```

---

## 常见误解（必须避免）

1. **误解：把 `mogai/build`（版本一的 6 个提交）合并进官方 2.0 `main`。**
   - 错误原因：这是两个项目。那 6 个提交改的是版本二的 Flutter/Rust 文件，但它们属于**版本二早期的实验性移植**，落后官方 172 个提交，且与官方 10-24 的改动冲突（冲突状态 `dirty`）。
   - 正确做法：如果要在版本二上做功能，应在官方最新 `main` 上**重新实现**，而不是合并旧提交。

2. **误解：版本一的 `PlanBoard.kt` 可以直接搬进版本二。**
   - 错误原因：版本一是 Compose（`.kt`），版本二是 Flutter（`.dart`）+ Rust（`.rs`），无法直接移植，只能重写。

3. **误解：两个包的数据目录可以互相覆盖。**
   - 错误原因：`com.ai.assistance.operit` 与 `app.operit` / `app.operit.mogai` 是不同包名，数据完全隔离。历史上强行改名映射曾导致两个包同时崩溃，**不要重复此操作**。

---

## 各自如何构建

| | 版本一（1.12.2） | 版本二（2.0） |
|---|---|---|
| 工具链 | Gradle + Android SDK + NDK r29 | Flutter + Rust + NDK r29 + V8 |
| 构建命令 | `./gradlew :app:assembleClone` | `python3 tools/build_scripts/build_flutter_android.py` |
| 环境 | proot Ubuntu（aarch64） | 官方 CI 走 `ubuntu-24.04` + `windows-2025` 两段式 |
| 云端 | 不适用 | GitHub Actions（非 fork 仓库才能触发） |

---

## 仓库边界

- 本文档所在仓库：版本二（Operit2 2.0）的构建/魔改工作区
- 版本一的代码/分支**不属于**本仓库主线，仅作历史留存
- 任何时候修改版本二，请先确认改动是否落在 `mogai/v2-build` 分支，且不要引入版本一的 Kotlin 文件
