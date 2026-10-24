# 仓库分组索引

GitHub 账号：`042xianqi`　共 **19** 个仓库，按 5 个族（P1–P5）分组。

命名规则：`族前缀-名称`。前缀决定仓库列表的排序，所以同族永远排在一起。

---

## P1 · Operit 1.12.2 魔改（已停更）

| 仓库 | 状态 | 说明 |
|---|---|---|
| `OP1-Mogai` | 🗄 归档 | Operit 1.12.2 魔改（Kotlin/Compose）。**与 P2 是两个独立项目，不可合并** |

> 1.12.2 已停止更新。所有新功能都在 P2 的 2.0 上做。

---

## P2 · Operit2 2.0（当前主线）

| 仓库 | 状态 | 说明 |
|---|---|---|
| `OP2-Mogai` | ✅ **当前开发** | 魔改构建。只含**群组修复** + **签名共存**（`app.operit.mogai`） |
| `OP2-Upstream` | 🔀 上游镜像 | 官方 `AAswordman/Operit2` 的 fork，仅用于同步，不在此开发 |

**关键文档**：`OP2-Mogai/docs/TWO-PROJECTS.md` —— 两个项目的边界、版本二只改了什么、三条必须避免的误解。

> ⚠️ 注意：GitHub 对 **fork 仓库禁用 API 触发 Actions**。`OP2-Upstream` 是 fork，`OP2-Mogai` 不是 fork，所以云端构建只能在 `OP2-Mogai` 上跑。

---

## P3 · OMP（Operit 跨设备 Agent 核心 + 工具链）

| 仓库 | 状态 | 说明 |
|---|---|---|
| `OMP-Core` | ✅ 活跃 | 跨设备 Agent 核心（Rust），含 7K 大脑 + 小说引擎 |
| `OMP-CodingAgent` | ✅ 活跃 | coding-agent 源码 + 改动流程规范（改动铁律、交付流程） |
| `OMP-MiniPRoot` | 📖 参考 | 手机上的 PRoot Ubuntu 24.04 容器 + 中英 AI 终端 |
| `OMP-Archive` | 🗄 归档 | omp-native（N-API novel 桥）+ 旧插件 |
| `OMP-Archive-napi` | 🗄 归档 | 原生 Node 模块 napi（cdylib）源码 |

---

## P4 · 小说（《残响之河》）

| 仓库 | 状态 | 说明 |
|---|---|---|
| `NOVEL-System` | ✅ 活跃 | 小说生成管线（纯 Rust，21 个 Python 工具链），~3.2MB |
| `NOVEL-SystemEngine` | ✅ 活跃 | 六层漏斗 L0-L5 + 门禁 + MCP 的 Rust 引擎，~5MB |
| `NOVEL-ResidualRiver` | ⚠️ **空仓** | 残响之河全 Rust 引擎（计划中） |
| `NOVEL-Materials` | ⚠️ **空仓** | 小说素材库，当前为空 |
| `NOVEL-Materials-Legacy` | 🗄 待删 | 旧素材库，只有 `.gitignore`，已被 `NOVEL-Materials` 取代 |

---

## P5 · 临时测试仓（可随时删）

命名以 `ZZZ-` 开头，永远排在列表最末。

| 仓库 | 说明 |
|---|---|
| `ZZZ-Test-ci-probe` | CI 触发探测 |
| `ZZZ-Test-ci-selftest` | CI 自检 |
| `ZZZ-Test-mogai-build` | 早期构建探测（有 `proxytest` 分支） |
| `ZZZ-Test-operit2-android` | Android CI 探测 |
| `ZZZ-Test-operit2-ci` | CI 探测 |
| `ZZZ-Test-operit2-ci-probe` | CI 探测（曾证明非 fork 仓库 Actions 可触发） |

**这 6 个仓全部是我做 CI 验证时创建的，不含任何项目代码，可以直接删。**

---

## 如何在 GitHub 网页上按组分

GitHub 的 **Lists** 功能没有 REST API，只能在网页上操作，一次设置永久生效：

1. 打开 <https://github.com/042xianqi?tab=repositories>
2. 点右上角 **「New list」**
3. 建 5 个列表，名称分别填：`P1-Operit1`、`P2-Operit2`、`P3-OMP`、`P4-小说`、`P5-临时`
4. 对每个仓库，点它右侧的 **「⋯」→「Add to list」**，按上表勾选

由于仓库名已按 `P1/OP1`、`OP2`、`OMP`、`NOVEL`、`ZZZ` 排序，即使不建 List，**默认列表也已经是分好组的**。

### 备选：按 Topic 筛选

每个仓库已打好 topics，点任一 topic 标签可筛出同族：

| 族 | 筛选链接 |
|---|---|
| P1 | <https://github.com/search?q=user%3A042xianqi+topic%3Ap1-archived&type=repositories> |
| P2 | <https://github.com/search?q=user%3A042xianqi+topic%3Ap2-current&type=repositories> |
| P3 | <https://github.com/search?q=user%3A042xianqi+topic%3Ap3-omp&type=repositories> |
| P4 | <https://github.com/search?q=user%3A042xianqi+topic%3Ap4-novel&type=repositories> |
| P5 | <https://github.com/search?q=user%3A042xianqi+topic%3Ap5-temp&type=repositories> |

---

## 一句话记忆

- **改 Operit2 功能** → `OP2-Mogai`
- **同步官方更新** → `OP2-Upstream`（只同步，不改）
- **看 OMP 核心** → `OMP-Core`
- **写小说管线** → `NOVEL-System` / `NOVEL-SystemEngine`
- **名字带 ZZZ 的** → 都能删