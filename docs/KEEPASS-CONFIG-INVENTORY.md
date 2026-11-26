# KeePass 2.x 完整配置清单 ↔ SecPivot 对照（源码级穷举）

> 之前的 [KEEPASS-OPTIONS-GAP.md](KEEPASS-OPTIONS-GAP.md) 只覆盖了 Options 对话框可见项（约 30 个）。KeePass 的完整配置面在 `KeePass.config.xml`，由 `AppConfigEx` 下十余个 `Ace*` 类构成，**字段总数过百**。本文按源码类逐项穷举并映射到 SecPivot。
>
> 证据来源（均为 KeePass 2.x 官方源码镜像 dlech/KeePass2.x `VS2022` 分支，`KeePass/App/Configuration/` 目录）：[AppConfigEx.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AppConfigEx.cs)、[AceApplication.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceApplication.cs)、[AceMainWindow.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceMainWindow.cs)、[AceUI.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceUI.cs)、[AceSecurity.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceSecurity.cs)、[AceIntegration.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceIntegration.cs)、[AceDefaults.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceDefaults.cs)、[AcePasswordGenerator.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AcePasswordGenerator.cs)、[AceSearch.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceSearch.cs)、[AceNative.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceNative.cs)、[AceLogging.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceLogging.cs)、[AceToolBar.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceToolBar.cs)、[AceTrayIcon.cs](https://raw.githubusercontent.com/dlech/KeePass2.x/VS2022/KeePass/App/Configuration/AceTrayIcon.cs)。官方 customization 文档：[keepass.info/help/v2_dev/customize.html](https://keepass.info/help/v2_dev/customize.html)、镜像 [2.61 La personnalisation](https://keepass.cool/2.x/2.61/Lapersonnalisation.html)。
>
> 状态图例：✅ 已有 · ⚠️ 部分/不同形态 · ❌ 未实现 · ➖ 不适用（平台/架构不同或纯状态缓存）。

SecPivot 的独立本地保存备份扩展使用 `localBackup.enabled/count/template`：默认关闭，保留 1–50 份原有加密文件，普通保存、另存为、改主密钥、关闭自动保存及新建覆盖共用覆盖前备份路径。备份失败中止写入，保存成功后才清理本功能创建的旧备份；真实 WebView、其他 OS 与 Android provider 仍待验证。这是对保存流程的增强，不新增虚构的官方 `Ace*` 配置节点；与 remote profiles 的镜像备份独立。

## 顶层结构（AppConfigEx）

自动加载外部数据库变化使用 SecPivot 独立的 `general.autoReloadChangedFiles`
（默认关闭）。本次对官方 `VS2022/KeePass/App/Configuration/` 全目录源码
检索未找到 `AutoReloadChangedFiles` 字段，因此按增强功能记录。获得焦点、
切换当前库和可见时每30秒检查加密字节；未变跳过解密，变更先准备，再以
会话/修订/哈希及一次性10秒 token 应用。干净会话重载，脏会话在内存合并；
冲突、损坏或新密钥保留当前数据。弹窗、内联编辑/备注草稿和拖动期间暂缓，
不写外部文件/远程镜像。原生合成 KDBX/离线 remote、实际 controller/service/
活动守卫与双主题320px预览已有证据；真实 WebView/provider/跨客户端仍待验。

| 节点                              | 类                   | SecPivot 对应                                            |
| --------------------------------- | -------------------- | -------------------------------------------------------- |
| `Configuration/Meta`              | AceMeta              | ➖ 内部元数据（版本/DPI/配置位置），非用户设置           |
| `Configuration/Application`       | AceApplication       | `general` + `keyboard` 部分                              |
| `Configuration/Logging`           | AceLogging           | ⚠️ 可选本地脱敏诊断日志；运行时平台验证待补              |
| `Configuration/MainWindow`        | AceMainWindow        | `general.window*` / `toolbarItems` / `entryColumns` 部分 |
| `Configuration/UI`                | AceUI                | 主题/字体部分                                            |
| `Configuration/Security`          | AceSecurity          | `security` 大部分                                        |
| `Configuration/Native`            | AceNative            | ➖ 平台加速开关                                          |
| `Configuration/PasswordGenerator` | AcePasswordGenerator | `database.generator*`                                    |
| `Configuration/Search`            | AceSearch            | `general.savedSearches` 部分                             |
| `Configuration/Defaults`          | AceDefaults          | `database` + `general` 部分                              |
| `Configuration/Integration`       | AceIntegration       | `keyboard` / `bridge` / `rpc` 部分                       |
| `Configuration/Custom`            | AceKvp[]             | ➖ 插件自定义配置                                        |

---

## 1. AceMeta（5，内部元数据，全部 ➖）

Version · PreferUserConfiguration · OmitItemsWithDefaultValues · DpiFactorX · DpiFactorY

## 2. AceApplication（23 + 子对象）

| 字段                                  | 默认  | SecPivot                                                                                                                               |
| ------------------------------------- | ----- | -------------------------------------------------------------------------------------------------------------------------------------- |
| ConfigSave                            | true  | ➖ 配置是否落盘，内部                                                                                                                  |
| LanguageFile                          | ""    | ✅ `general.language`（语言内建双字典，非文件）                                                                                        |
| HelpUseLocal                          | false | ➖                                                                                                                                     |
| HelpUrl                               | ""    | ➖                                                                                                                                     |
| LastUpdateCheck                       | ""    | ➖                                                                                                                                     |
| LastUsedFile                          | —     | ✅ `general.recentFiles`                                                                                                               |
| MostRecentlyUsed (AceMru)             | —     | ✅ `general.recentFiles`（上限 8，KeePass 默认 12）⚠️ 上限不同                                                                         |
| RememberWorkingDirectories            | true  | ➖                                                                                                                                     |
| WorkingDirectoriesSerialized          | —     | ➖                                                                                                                                     |
| Start (AceStartUp)                    | —     | 见下                                                                                                                                   |
| FileOpening (AceOpenDb)               | —     | 见下                                                                                                                                   |
| VerifyWrittenFileAfterSaving          | true  | ➖ 写盘校验，内部                                                                                                                      |
| UseTransactedFileWrites               | true  | ➖ 原子写，SecPivot 已内建 atomic_write                                                                                                |
| UseTransactedConfigWrites             | true  | ➖                                                                                                                                     |
| FileTxExtra                           | false | ➖                                                                                                                                     |
| UseFileLocks                          | false | ➖                                                                                                                                     |
| SaveForceSync                         | false | ➖                                                                                                                                     |
| **AutoSaveAfterEntryEdit**            | false | ✅ `security.autoSaveAfterEdit`（本会话批次3）                                                                                         |
| FileClosing (AceCloseDb)              | —     | 见下                                                                                                                                   |
| TriggerSystem                         | —     | ➖ 触发器系统，不适用                                                                                                                  |
| PluginCachePath / PluginCompatibility | —     | ➖                                                                                                                                     |
| **ExpirySoonDays**                    | 7     | ✅ `security.expirySoonDays`（B3 批次）：开库提醒把 7 天内到期计入——`showExpiredOnOpen` 开关给了它消费点，之前"无消费点"的取舍随之解除 |

### AceStartUp（6）

OpenLastFile ✅ `rememberLastDatabase` · CheckForUpdate ➖ · CheckForUpdateConfigured ➖ · MinimizedAndLocked ❌ · PluginCacheDeleteOld ➖ · PluginCacheClearOnce ➖

### AceOpenDb（2）

ShowExpiredEntries ✅ `security.showExpiredOnOpen`（B3 批次，默认 on，关掉则开库不提醒）· ShowSoonToExpireEntries ✅ 并入同一提醒（`expirySoonDays` 天内到期计数）

### AceCloseDb（1）

AutoSave ⚠️ `security.autoSaveOnClose`（默认 false，与官方一致）：原生关闭标签页、锁定和退出共用先保存流程；关闭选项则提供保存/放弃/取消。取消或保存失败保留会话，并检查保存期间的新修改/新标签页。临时 KDBX/离线 remote 与前端生命周期测试通过；真实 WebView 关窗/托盘退出及 Android provider/后台运行仍待验证。

### AceMru（2）

MaxItemCount ✅ `recentFiles`（上限 12，本批对齐 KeePass 默认）· Items ✅

---

## 3. AceMainWindow（44 + 列类型枚举 22）

### 窗口/行为（本类主体）

| 字段                                     | 默认    | SecPivot                                                                                         |
| ---------------------------------------- | ------- | ------------------------------------------------------------------------------------------------ |
| X / Y / Width / Height / Maximized       | —       | ✅ `windowWidth/Height` + 位置由 Tauri 管                                                        |
| SplitterHorizontalFrac / VerticalFrac    | —       | ✅ `panelWidths`                                                                                 |
| Layout (Default/SideBySide)              | Default | ➖ 两种布局，SecPivot 单一布局                                                                   |
| AlwaysOnTop                              | false   | ✅ `general.alwaysOnTop`（批次2）                                                                |
| CloseButtonMinimizesWindow               | false   | ✅ `security.minimizeToTray`                                                                     |
| EscMinimizesToTray（旧兼容）             | false   | ➖                                                                                               |
| EscAction (None/Lock/Min/MinTray/Exit)   | Lock    | ❌ Esc 行为可配（SecPivot 固定关弹窗）                                                           |
| MinimizeToTray                           | false   | ✅ `security.minimizeToTray`                                                                     |
| ShowFullPathInTitle                      | false   | ⚠️ `general.showFullPathInTitle`：配置/开关及原生 setTitle 已接入；平台运行待验                  |
| DropToBackAfterClipboardCopy             | false   | ❌（旧 gap 列为候选）                                                                            |
| MinimizeAfterClipboardCopy               | false   | ❌                                                                                               |
| MinimizeAfterAutoType                    | false   | ❌（当前无条件最小化）                                                                           |
| MinimizeAfterLocking                     | true    | ✅ `security.minimizeAfterLocking`（A2 批次，默认 on 与 KeePass 一致）                           |
| MinimizeAfterOpeningDatabase             | false   | ❌                                                                                               |
| QuickFindSearchInPasswords               | false   | ❌（搜索不含密码）                                                                               |
| QuickFindExcludeExpired                  | false   | ⚠️ `general.quickFindExcludeExpired`：配置/显示页/实际快速搜索与边界回归已接入；真实 WebView待验 |
| QuickFindDerefData                       | false   | ❌                                                                                               |
| FocusResultsAfterQuickFind               | false   | ❌                                                                                               |
| FocusQuickFindOnRestore / OnUntray       | false   | ❌                                                                                               |
| CopyUrlsInsteadOfOpening                 | false   | ✅ `general.copyUrlsInsteadOfOpening`（A2 批次）：点击 URL 走敏感复制路径                        |
| EntrySelGroupSel                         | true    | ✅ 选中关联分组行为                                                                              |
| DisableSaveIfNotModified                 | false   | ➖ 语义细节                                                                                      |
| HideCloseDatabaseButton                  | true    | ➖                                                                                               |
| ToolBar (AceToolBar: Show)               | true    | ✅ `toolbarItems`                                                                                |
| EntryView (AceEntryView: Show)           | true    | ✅ 详情面板开关                                                                                  |
| TanView (UseSimpleView/ShowIndices)      | true    | ➖ TAN 视图不适用                                                                                |
| EntryListColumns                         | —       | ✅ `entryColumns`                                                                                |
| EntryListColumnDisplayOrder              | —       | ✅ `entryColumns` 顺序                                                                           |
| EntryListAutoResizeColumns               | false   | ❌                                                                                               |
| EntryListAlternatingBgColors / Color     | true/0  | ⚠️ `general.alternatingRowColors` 默认关，实际虚拟行斑马纹已接入；自定义颜色非独立设置           |
| EntryListShowDerefData / Async / AndRefs | false   | ❌ 字段引用显示                                                                                  |
| ListSorting (ListSorter)                 | —       | ✅ `sortCol/sortDir`                                                                             |
| ListGrouping (Auto/On/Off)               | Auto    | ✅ `showGroupSeparators` 近似                                                                    |
| ShowEntriesOfSubGroups                   | false   | ❌ 显示子分组条目                                                                                |

### AceColumnType 枚举（22 种列类型）

Title · UserName · Password · Url · Notes · CreationTime · LastModificationTime · LastAccessTime · ExpiryTime · Uuid · Attachment · CustomString · PluginExt · OverrideUrl · Tags · ExpiryTimeDateOnly · Size · HistoryCount · AttachmentCount · LastPasswordModTime · AutoTypeEnabled · AutoTypeSequences

SecPivot 内置列 20 种：title/username/password/url/totp/notes/tags/created/modified/expires/size + historyCount/attachmentCount/autoTypeEnabled/overrideUrl/uuid（B1 批次五种，默认隐藏 + custom:*），外加 lastAccessTime / attachment / lastPasswordModTime / autoTypeSequences（本次补齐）。`lastAccessTime` 直接取 `Times.LastAccess`；`lastPasswordModTime` 按官方 `EntryUtil.GetLastPasswordModTime` 从历史快照反推（当前密码与历史一致则继续向前，取最旧一致快照的时间；无历史时回退到条目修改时间），仅时间值入快照、密码不外泄。KeePass 的 CustomString（用自定义列 `custom:*` 覆盖）、PluginExt（通用插件元数据只读，不单独列展示）与 ExpiryTimeDateOnly（沿用 `expires` 的日期展示）不再列为缺失。

---

## 4. AceUI（31 + 子对象 + 枚举）

| 字段                                                               | 默认    | SecPivot                                                            |
| ------------------------------------------------------------------ | ------- | ------------------------------------------------------------------- |
| TrayIcon (AceTrayIcon)                                             | —       | 见下                                                                |
| Hiding (AceHiding)                                                 | —       | 见下                                                                |
| RepeatPasswordOnlyWhenHidden                                       | true    | ✅ `security.repeatPasswordOnlyWhenHidden`：本地/远程建库与改密钥   |
| StandardFont / PasswordFont (AceFont)                              | —       | ⚠️ `fontSizes` 仅字号，无字体族/样式/单位                           |
| ForceSystemFontUnix                                                | true    | ➖                                                                  |
| BannerStyle                                                        | Dark    | ➖                                                                  |
| ShowImportStatusDialog                                             | false   | ❌                                                                  |
| ShowRecycleConfirmDialog                                           | true    | ✅ 回收站确认                                                       |
| ShowCmdUriConfirmDialog                                            | true    | ❌（cmd:// 确认，SecPivot 无 cmd:// 协议）                          |
| ShowCmdPlhConfirmDialog                                            | true    | ❌（{CMD:...} 占位符确认）                                          |
| ShowRefPPlhConfirmDialog                                           | true    | ❌（{REF:P@...} 确认）                                              |
| ShowDbMntncResultsDialog                                           | true    | ❌                                                                  |
| ShowEmSheetDialog                                                  | true    | ❌                                                                  |
| ShowDbOpenUnkVerDialog                                             | true    | ❌                                                                  |
| ToolStripRenderer                                                  | ""      | ➖                                                                  |
| TreeViewShowLines                                                  | false   | ⚠️ `general.groupTreeLines`：开关/组件链/连接线已接入；平台运行待验 |
| OptimizeForScreenReader                                            | false   | ❌                                                                  |
| DataViewerRect / DataEditorRect / CharPickerRect / AutoTypeCtxRect | —       | ➖ 窗口位置缓存                                                     |
| DataEditorFont (AceFont)                                           | —       | ❌ 字体族                                                           |
| DataEditorWordWrap                                                 | true    | ❌                                                                  |
| AutoTypeCtxFlags                                                   | Default | ❌                                                                  |
| AutoTypeCtxColumnWidths                                            | ""      | ➖                                                                  |
| UIFlags（13 位标志）                                               | None    | ❌ 管理强制 UI 状态（个人版不适用）                                 |
| KeyCreationFlags / KeyPromptFlags（16 位标志）                     | None    | ❌ 密钥 UI 强制状态                                                 |
| ShowAdvAutoTypeCommands                                            | false   | ❌                                                                  |
| SecureDesktopPlaySound                                             | true    | ➖ 安全桌面不适用                                                   |

### AceTrayIcon（3）

ShowOnlyIfTrayedEx ❌ · GrayIcon ❌ · SingleClickDefault ❌（托盘交互）

### AceHiding（5）

RememberHidingPasswordsMain · SeparateHidingSettings · HideInEntryWindow · UnhideButtonAlsoUnhidesSource · UnhideEmptyData —— 全部 ❌（密码显示/隐藏细粒度行为）

### AceFont（5，×4 处引用）

Family · Size · GraphicsUnit · Style · OverrideUIDefault —— SecPivot 仅 `fontSizes` 数字，无族/样式

---

## 5. AceSecurity（16 + 子对象）

| 字段                                   | 默认  | SecPivot                                                                                                          |
| -------------------------------------- | ----- | ----------------------------------------------------------------------------------------------------------------- |
| WorkspaceLocking (AceWorkspaceLocking) | —     | 见下                                                                                                              |
| Policy (AppPolicyFlags)                | —     | ❌ 管理强制策略（见旧 gap 取舍）                                                                                  |
| MasterPassword (AceMasterPassword)     | —     | 见下                                                                                                              |
| MasterKeyTries                         | 3     | ⚠️ `security.masterKeyTries`：每次打开提示的提交预算，本地/远程/锁屏手动与 Hello 接入；真实 WebView/provider 待验 |
| MasterKeyOnSecureDesktop               | false | ➖ 安全桌面不适用                                                                                                 |
| MasterKeyExpiryRec / Force             | ""    | ⚠️ 规则草稿校验/设置/打开提醒与强制更换已接入；真实 WebView/provider/官方客户端待验                               |
| KeyTransformWeakWarning                | true  | ⚠️ `security.keyTransformWeakWarning`：改 KDF/主密钥及打开库升级提醒已接入；真实 WebView/provider 待验            |
| ClipboardClearOnExit                   | true  | ✅ `clearOnLock` 近似（退出/锁定）                                                                                |
| ClipboardClearAfterSeconds             | 12    | ✅ `clipboardClearSeconds`（默认 20）⚠️ 默认值不同                                                                |
| ClipboardNoPersist                     | true  | ➖ 剪贴板持久化，平台细节                                                                                         |
| UseClipboardViewerIgnoreFormat         | true  | ➖                                                                                                                |
| ClearKeyCommandLineParams              | true  | ➖                                                                                                                |
| SslCertsAcceptInvalid                  | false | ➖ 证书校验                                                                                                       |
| PreventScreenCapture                   | false | ✅ `screenCaptureGuard`                                                                                           |
| ProtectProcessWithDacl                 | false | ➖ 官方明确不推荐，高危                                                                                           |

### AceWorkspaceLocking（9）

| 字段                          | 默认  | SecPivot                                    |
| ----------------------------- | ----- | ------------------------------------------- |
| LockOnWindowMinimize          | false | ⚠️ `lockOnFocusLoss` 近似                   |
| LockOnWindowMinimizeToTray    | false | ✅ `security.lockOnMinimizeToTray`（批次4） |
| LockOnSessionSwitch           | false | ❌ 需系统事件                               |
| LockOnSuspend                 | false | ❌ 需系统事件                               |
| LockOnRemoteControlChange     | false | ❌ 需系统事件                               |
| LockAfterTime                 | 0     | ✅ `autoLockMinutes`（秒 vs 分，单位不同）  |
| LockAfterGlobalTime           | 0     | ❌ 全局空闲锁                               |
| ExitInsteadOfLockingAfterTime | false | ❌                                          |
| AlwaysExitInsteadOfLocking    | false | ❌                                          |

### AceMasterPassword（3）

MinimumLength ❌ · MinimumQuality ❌（Policy 类）· RememberWhileOpen ✅ `rememberPassword`

---

## 6. AceIntegration（33 + URL overrides）

| 字段                                                        | 默认             | SecPivot                                                                                                                                                          |
| ----------------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HotKeyGlobalAutoType                                        | Ctrl+Alt+A       | ✅ `keyboard.autoTypeGlobal`                                                                                                                                      |
| HotKeyGlobalAutoTypePassword                                | Ctrl+Alt+Shift+A | ❌ 第二个全局热键（仅密码）                                                                                                                                       |
| HotKeySelectedAutoType                                      | None             | ❌ 选中条目自动填充热键                                                                                                                                           |
| HotKeyShowWindow                                            | Ctrl+Alt+K       | ✅ `keyboard.showWindowGlobal`（A3 批次）：统一 `register_global_hotkeys` 注册集                                                                                  |
| HotKeyEntryMenu                                             | None             | ❌ 条目菜单热键                                                                                                                                                   |
| CheckHotKeys                                                | true             | ➖ 热键冲突检查                                                                                                                                                   |
| UrlOverrideEnabled                                          | true             | ✅ `integration.urlOverrideEnabled`：仅控制全局模板；条目覆盖和协议规则独立生效，默认 on                                                                          |
| UrlOverride（全局 KeeUrlOverride 字符串）                   | ""               | ✅ `integration.urlOverride` 独立输入，精确保留命令空格；条目模板优先，展开后再匹配一次协议规则；桌面运行链                                                       |
| UrlSchemeOverrides（27 内置 + 自定义）                      | 全禁用           | ✅ `integration.builtInOverridesEnabled`（位掩码）与 `customOverrides`；27 条官方内置表为代码常量，运行链 `open_entry_url` 按「条目级 → 内置 → 自定义」应用       |
| SearchKeyFiles                                              | true             | ⚠️ Windows 根目录候选搜索/安全页设置/打开与锁屏已接入；不读内容、不自动选择；真实驱动器/WebView/其他平台待验                                                      |
| SearchKeyFilesOnRemovableMedia                              | false            | ⚠️ 原生移动盘范围与默认关配置已接入；主搜索关闭时忽略，真实移动盘/WebView 待验                                                                                    |
| LimitToSingleInstance                                       | true             | ✅ 已实现（07e1936 后批次）：`tauri-plugin-single-instance` 首位注册，第二次启动唤起已有窗口；原生行为固定开启（KeePass 默认同 on），无设置键，进程级行为无法单测 |
| AutoTypeMatchByTitle                                        | true             | ✅ 匹配逻辑内建                                                                                                                                                   |
| AutoTypeMatchByUrlInTitle                                   | false            | ❌                                                                                                                                                                |
| AutoTypeMatchByUrlHostInTitle                               | false            | ❌                                                                                                                                                                |
| AutoTypeMatchByTagInTitle                                   | false            | ❌                                                                                                                                                                |
| AutoTypeMatchNormDashes                                     | true             | ❌ 相似破折号视为相同                                                                                                                                             |
| AutoTypeExpiredCanMatch                                     | false            | ❌ 过期条目可匹配                                                                                                                                                 |
| AutoTypeAlwaysShowSelDialog                                 | false            | ❌ 总是显示候选对话框                                                                                                                                             |
| AutoTypePrependInitSequenceForIE                            | true             | ➖ IE 不适用                                                                                                                                                      |
| AutoTypeReleaseAltWithKeyPress                              | true             | ➖ 平台细节                                                                                                                                                       |
| AutoTypeAdjustKeyboardLayout                                | true             | ❌                                                                                                                                                                |
| AutoTypeAllowInterleaved                                    | false            | ❌                                                                                                                                                                |
| AutoTypeCancelOnWindowChange                                | false            | ❌                                                                                                                                                                |
| AutoTypeCancelOnTitleChange                                 | false            | ❌                                                                                                                                                                |
| AutoTypeInterKeyDelay                                       | -1               | ❌ enigo 0.6 仅 Linux X11 支持，Windows 假开关                                                                                                                    |
| AutoTypeAbortOnWindows                                      | []               | ❌ 禁用自动填充的窗口列表                                                                                                                                         |
| ProxyType / Address / Port / AuthType / UserName / Password | System/...       | ⚠️ SecPivot 远程走系统代理，无独立代理配置                                                                                                                        |

---

## 7. AceDefaults（20 + 子对象）

| 字段                                                                       | 默认  | SecPivot                                                       |
| -------------------------------------------------------------------------- | ----- | -------------------------------------------------------------- |
| NewEntryExpiresInDays                                                      | -1    | ✅ `security.defaultExpireDays`（默认 0，语义同 -1=继承/不设） |
| OptionsTabIndex                                                            | 0     | ➖                                                             |
| TanCharacters / TanExpiresOnUse                                            | —     | ➖ TAN 不适用                                                  |
| FileSaveAsDirectory                                                        | ""    | ❌ 另存为默认目录                                              |
| RememberKeySources                                                         | true  | ⚠️ `rememberPassword` 部分                                     |
| KeySources (AceKeyAssoc[])                                                 | —     | ➖ 密钥源关联                                                  |
| CustomColors                                                               | ""    | ➖                                                             |
| RecycleBinCollapse                                                         | false | ❌                                                             |
| Duplication（ExtendTitle/CreateFieldReferences/CopyHistory）               | —     | ❌ 复制条目行为                                                |
| Print（约 20 项：Layout/Include*/Color*/Fonts/Sort）                       | —     | ❌ 打印系统不适用                                              |
| ExportMasterKeySpec / ExportParentGroups / ExportPostOpen / ExportPostShow | false | ❌ 导出细节                                                    |
| WinFavsBaseFolderName / FileNamePrefix / FileNameSuffix                    | —     | ➖ Windows 收藏夹导出                                          |

---

## 8. AcePasswordGenerator（3 + PwProfile）

ProfilesEnabled ✅ · AutoGeneratedPasswordsProfile ✅ `database.generator` · UserProfiles ✅ `generatorProfiles`
（PwProfile 内部字段：Length/字符类/排除/必含/Pattern/收集项 —— SecPivot 已镜像 [旧 gap 文档]）

## 9. AceSearch（2 + SearchParameters）

LastUsedProfile / UserProfiles ⚠️ `savedSearches`（SecPivot 有命名搜索，无"上次使用"概念）
SearchParameters 字段（SearchString/Field/Regex/Exclude/OnlyExpired/OnlyFavorites/Tags/RequireQualityCheck 等）—— SecPivot `AdvancedSearchQuery` 基本镜像。

## 10. AceLogging（1）

Enabled ⚠️ `logging.enabled` 默认关：设置/原生启动/运行时同步与实际脱敏日志已接入；真实 WebView/Android/框架口径待验。
官方 AppLogEx.Open 在常规构建中被注释；SecPivot 增强为实际本地 Logs 文件，
只记录静态脱敏模板及类型限定的配置错误类别/位置。关闭无文件，开启独占新
文件，1 MiB 上限且保留旧日志；原生状态反馈不返回日志内容。

## 11. AceNative（1）

NativeKeyTransformations ❌（原生 KDF 加速开关，SecPivot 固定用最优实现）

---

## 汇总统计（按源码字段，含子对象与枚举）

| 类别                       | 字段数(约) | ✅ 已有 | ⚠️ 部分 | ❌ 未实现 | ➖ 不适用 |
| -------------------------- | ---------- | ------- | ------- | --------- | --------- |
| AceMeta                    | 5          | 0       | 0       | 0         | 5         |
| AceApplication + 子        | 40         | ~7      | 2       | ~5        | ~26       |
| AceMainWindow + 列类型     | 66         | ~7      | 1       | ~20       | ~38       |
| AceUI + 子 + 枚举          | 70         | ~2      | 1       | ~30       | ~37       |
| AceSecurity + 子           | 28         | ~6      | 2       | ~14       | ~6        |
| AceIntegration + overrides | 60         | ~4      | 1       | ~25       | ~30       |
| AceDefaults + 子           | 45         | ~3      | 1       | ~10       | ~31       |
| AcePasswordGenerator       | 3+         | 3       | 0       | 0         | 0         |
| AceSearch                  | 2+         | 0       | 1       | 0         | 0         |
| AceLogging / AceNative     | 2          | 0       | 1       | 1         | 0         |
| **合计**                   | **~320**   | **~32** | **~9**  | **~110**  | **~170**  |

> 说明：把 28 个内置 URL override、22 种列类型、各类 Flags 枚举位都算进去，源码配置面约 320 项；仅算"用户可配的标量设置"也在 120+ 项，与你记忆的"近百个"一致。上表数量保留初次调查的粗估口径，不能作为当前已交付数量；功能状态以逐项表和当前源码/验证为准。

## 可行性分层（后续若要继续补）

**A 层 — 小改动、真行为、可验证（建议优先）**

- 斑马纹、分组树连线、完整路径与 `QuickFindExcludeExpired` 已接入配置/消费点，剩余运行验证见逐项表
- `CopyUrlsInsteadOfOpening` 已接入；`QuickFindSearchInPasswords` 尚未实现，需保持默认不读取密码的搜索边界
- `MinimizeAfterLocking` / `MinimizeAfterAutoType`：需谨慎，前者低风险，后者涉及焦点交接
- `HotKeyGlobalAutoTypePassword` / `HotKeyShowWindow` / `HotKeyEntryMenu`：复用现有全局热键注册机制

**B 层 — 中等改动**

- 完整 URL scheme overrides（28 内置 + 自定义表）+ IPC + 打开链路改造
- 列类型扩充（HistoryCount / AttachmentCount / LastModificationTime 等展示列）
- 高级搜索参数与 SecPivot savedSearches 对齐
- 单实例、代理配置、打开库时过期筛选

**C 层 — 不建议（平台/架构/安全）**

- Policy 强制策略 / UIFlags / KeyCreationFlags（个人库，且会锁死用户）
- Secure desktop / ProtectProcessWithDacl（高危，官方自己不推荐）
- 打印系统 / TAN 视图 / Windows 收藏夹导出 / 触发器系统（架构不适用）
- 各种 Rect/DPI/PluginCache 等内部状态缓存（非用户设置）
