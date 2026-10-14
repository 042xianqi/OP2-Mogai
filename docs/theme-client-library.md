# 客户端使用 ToolPkg 的 theme 模板

主题列表是已安装 ToolPkg 的模板能力视图。归档使用 `ExtensionStore` 的普通 `package` 记录及 `runtime/extensions/{device|space}/packages` 存储，主题没有独立的归档目录、安装事务或同步规则。

每个列表条目对应一个 `toolpkg_id:template_id` 身份，包含所属包 ID、模板 ID、包版本及归档摘要。同一包中的多个模板共享包字节；预览和背景缓存使用模板身份区分。

导入时 `inspectThemeArchive` 枚举模板回执，多个模板由用户明确选择。确认后 `importToolPkgArchive` 核对字节数与 SHA-256，再调用通用包管理流程安装整个 ToolPkg。导入保持本节点当前选择；用户可以继续明确应用某个模板。

`previewThemeArchive` 接收包字节和明确的模板身份，只返回该模板的规范化视觉 JSON、资源和校验回执。`applyTheme` 从普通安装目录重新读取包、校验所选模板，再写入本节点的选择。包更新不会自动替换原先确认的归档摘要。

节点状态保存在 `runtime/config/preferences/theme_selection.preferences.json`，包括 `selection` 和 `privateAppearanceJson`。选择丢失或包未同步到达时保留节点选择并报告不可用，用户可以明确切换至个人外观。

卸载通过普通包管理系统删除整个 ToolPkg，因此同时移除该包贡献的脚本和所有模板。当前节点使用该包的任一模板时，主题页面拒绝直接卸载，要求先明确切换个人外观。

| Core 方法 | 返回值／用途 |
| --- | --- |
| `inspectThemeArchive(archiveBytes)` | `Vec<ThemeValidationReceipt>`：枚举包声明的模板 |
| `importToolPkgArchive(archiveBytes, expectedArchiveBytes, expectedArchiveSha256)` | `bool`：通用安装；表示完全相同的包已存在 |
| `previewThemeArchive(archiveBytes, themeId)` | `ThemePreviewReceipt`：所选模板的渲染载荷 |
| `readToolPkgForThemeTemplate(themeId, archiveSha256)` | 普通 ToolPkg 原始字节 |
| `listThemeLibrary()` | `Vec<ThemeLibraryItem>`：普通包贡献的模板列表 |
| `listThemes()` | `Vec<ThemeLibraryEntry>`：可用模板 |
| `applyTheme(themeId, archiveSha256)` | `ThemeInstallReceipt`：本节点应用结果 |
| `deleteTheme(themeId, archiveSha256)` | 卸载模板所属的整个 ToolPkg |
| `nodeThemeState()` / `nodeThemeStateFlow()` | 本节点外观状态及其订阅 |
| `selectPersonalThemeAppearance()` | 清除节点模板选择，使用个人外观 |

Flutter 通过 `ThemeLibraryService` 准备媒体，使用 `operit_ui` 的视觉模型、资源作用域和字体注册器渲染。Rust 的 Host API 负责读取包字节；客户端不添加平台专用文件系统或 HTTP 路径。
