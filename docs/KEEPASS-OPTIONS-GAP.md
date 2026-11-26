# KeePass Options ↔ SecPivot Settings Gap Analysis

> 本文只覆盖 Options 对话框可见项。KeePass `KeePass.config.xml` 的完整配置面（源码级穷举，字段过百）见 [KEEPASS-CONFIG-INVENTORY.md](KEEPASS-CONFIG-INVENTORY.md)。

证据来源：KeePass 2.57 官方手册镜像 [keepass.cool 2.57](https://keepass.cool/2.x/2.57/Lapersonnalisation.html)、[keepass.info search 帮助](https://keepass.info/help/base/search.html#mode_se)、官方 changelog 转载（[2.39 Large Entries](https://www.neowin.net/news/keepass-password-safe-2391/#1)、[2.53 报告对话框筛选框](https://www.3dfxzone.it/files/?object=html&objid=21228)）、开源镜像 [dlech/KeePass2.x](https://github.com/dlech/KeePass2.x)。外部资料只作导航，最终以 SecPivot 本地源码为准。

已对齐 `UI.RepeatPasswordOnlyWhenHidden`（Interface → Advanced，默认开）：Security 页设置同时控制本地/远程建库和修改主密钥。显示密码时清空并禁用重复栏，切回隐藏时同步一次；关闭选项后无论显隐都需要重复校验。依据官方 [AceUI.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/App/Configuration/AceUI.cs)、[PwInputControlGroup.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/UI/PwInputControlGroup.cs) 与 [KeyCreationForm.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/Forms/KeyCreationForm.cs)。前端真实提交函数/设置重载、原生 ConfigStore 旧配置/双值重载和实际浅色/深色/320px 渲染覆盖此项；密码不进入配置，原有凭据 IPC 不变。

## KeePass Options 六个 tab 与 SecPivot 现状

`AceSecurity.KeyTransformWeakWarning` 默认 true，Security 页开关在 TS/Rust
持久化。数据库设置保存偏弱 KDF 成本、更换弱参数库主密钥前，使用共享取消/
继续提示；当前建库默认值已达标。按官方
[AesKdf.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePassLib/Cryptography/KeyDerivation/AesKdf.cs)
的 600000 轮与
[Argon2Kdf.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePassLib/Cryptography/KeyDerivation/Argon2Kdf.cs)
的 iterations × memory < 2 × 64 MiB 判断，不以算法名称或浮点截断代替。
确认绑定具体成本并在写入前消费，取消不丢草稿；改主密钥预检和提示绑定原会话/
视图代次。ConfigStore 旧配置/双值重载、实际提交函数与服务回归、独立提示组件
渲染覆盖该单元；真实 WebView/provider 仍待验证。

打开库的升级流程按官方
[KeyUtil.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/Util/KeyUtil.cs)
与 [MainForm_Functions.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/Forms/MainForm_Functions.cs)
接入：展示当前/推荐成本，同意后保留算法并应用 SecPivot 建库默认成本
（AES600000；Argon2/Argon2id3 ×64 MiB、4 lanes、v1.3，比官方告警门槛更高）。
与官方 UpdateUIState(true) 一样只产生未保存状态，普通保存才写入文件/远程；
原生寿命与参数校验、只读及保存串行化防止旧提示/旧保存覆盖新会话或成本。
提示按解密寿命只展示一次，返回标签页不会重复。临时 KDBX 保存/重开、离线
MemoryStorage、旧保存/停放会话及实际前端 controller/host/service 回归已覆盖；
真实 WebView/provider 与官方客户端互操作继续保留为待验。

`AceCloseDb.AutoSave` 已实现原生保存流程与 Security 页开关（默认 false）。
官方 [AceApplication.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/App/Configuration/AceApplication.cs)
定义该默认值；[MainForm_Functions.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/Forms/MainForm_Functions.cs)
的 CloseDocument 在关闭/锁定/退出前保存或询问，取消/保存失败则中止。
SecPivot 在多库关闭中额外验证整个会话集合及并发修改，失败时保持所有会话打开；
先前成功的保存仍然有效。原生临时 KDBX、离线 remote、配置重载、实际前端服务和
设置浅色/深色/320px 渲染已有证据；真实 WebView 关窗/托盘及 Android 运行验证未关闭。

### Security（安全）

主密钥过期所需的 KDBX `MasterKeyChanged` 时间已修复：本地/远程建库
初始化 UTC 秒级时间，更换主密钥成功后将实际写入的时间回填会话；并发编辑
继续保留，普通保存/另存为/改 KDF 不更新主密钥年龄。临时 KDBX 与离线
MemoryStorage 回归覆盖保存重开、仅密钥文件和失败路径。
`MasterKeyChangeForceOnce` 的未设置/False/True 已在依赖模型和 XML 转换中
保留，实际 KDBX/离线远程保存、另存为、改 KDF 与只读回归通过。
成功更换主密钥时，同次权威保存将 True 消费为 False，再回填会话；失败/
只读拒绝保留原请求。真实 KDBX、仅密钥文件、并发及离线远程回归通过。
全局 `MasterKeyExpiryRec` / `MasterKeyExpiryForce` 字符串默认禁用并在 TS/
原生配置中保留。原生只读评估已接入数据库天数与全局 XML 时长/日期截止规则，
强制优先；无效规则单独反馈，有效规则继续判断。真实临时 KDBX/离线 remote、
.NET Framework `XmlConvert` 独立参考值及实际前端配置/会话守卫回归通过。
设置界面、打开时提醒/强制更换与原生 WebView/provider/官方客户端运行仍待，
不能据此标记整个条目完成。

`AceSecurity.MasterKeyTries` 按官方
[AceSecurity.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/App/Configuration/AceSecurity.cs)
与 [MainForm_Functions.cs](https://github.com/dlech/KeePass2.x/blob/VS2022/KeePass/Forms/MainForm_Functions.cs)
的打开提示循环接入：默认 3，正 int32 范围，每个新打开意图重新计次。
本地/最近/远程打开与锁屏手动/Hello 解锁均在提交前预留一次，提交失败
（含 I/O）计次，Hello 取凭据失败不计次；耗尽后清空字段并结束本次提示。
新提示与组件销毁使旧回调失效，重复提交不另占次数。配置与计次不保存凭据，
不增加原生 IPC 限流或永久锁库，也不构成离线防破解。
实际生产提交函数的合成边界回归与 ConfigStore 旧配置/范围/重载测试覆盖此项；
共享设置卡浅色/深色/320px 渲染为外观证据，真实 WebView/provider 仍待验。

| KeePass 项                                | SecPivot 状态                                                                                                  | 证据                                          |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| Lock workspace after inactivity (seconds) | ✅ `security.autoLockMinutes`                                                                                  | SecuritySettingsPanel SettingRangeCard        |
| Lock after **global user inactivity**     | ❌ 未实现（需要系统级空闲检测，WebView 内不可靠）                                                              | —                                             |
| Clipboard auto-clear time                 | ✅ `security.clipboardClearSeconds`                                                                            | security.ts copyText                          |
| By default, new entries expire in N days  | ✅ **`security.defaultExpireDays`**（本轮 510416c 新增）                                                       | EntryEditorDialog create mode 预填            |
| Lock when minimizing to tray              | ✅ `security.lockOnMinimizeToTray`（本批实现）：隐藏到托盘时复用 `tray-lock` 事件锁定；需先启用 minimizeToTray | src-tauri/src/lib.rs handle_close_requested   |
| Lock when minimizing to taskbar           | ⚠️ `lockOnFocusLoss` 靠 window blur 覆盖                                                                       | security.ts installFocusLock                  |
| Lock on workstation lock / user switch    | ⚠️ blur 通常伴随，非显式事件                                                                                   | —                                             |
| Lock on suspend / remote-control change   | ❌ 需原生系统事件                                                                                              | —                                             |
| Exit instead of lock / Always exit        | ❌ 未实现                                                                                                      | handle_close_requested 恒为退出/托盘          |
| Secure desktop for master key             | ❌ 未实现（Windows 安全桌面，成本高）                                                                          | —                                             |
| Master key tries                          | ⚠️ `security.masterKeyTries`（默认 3）：每次打开提示预算；真实 WebView/provider 待验                           | MasterKeyAttempts / VaultWelcome / LockScreen |
| Warn on weak key transformation           | ⚠️ `security.keyTransformWeakWarning`：改成本/主密钥及打开弱库升级提示；真实 WebView/provider 待验             | SecuritySettingsPanel / KdfUpgradePrompt      |

### Policy（策略）

KeePass 的 Policy tab 是**管理员强制策略**（最小密码长度、字符类、主密码最小质量/长度、过期强制、复制/导出权限等），面向企业部署。SecPivot 是个人本地优先客户端：

- ✅ 密码生成器已有 `length / includeUpper / includeLower / includeDigits / includeSymbols / excludeSimilar / excludeAmbiguous / customCharset / excludeChars / requiredChars / pattern` 与命名配置档（Database 页）。
- ✅ 数据库历史/回收站策略在 **Database Settings**（KeePass 官方也在 Database Settings → Advanced/Compression/Recycle Bin，不在 Options），已完整实现（`history_max_items` / `history_max_size` / `recyclebin_enabled`）。
- ❌ 强制策略执行（enforce）无对应需求：强加会给现有库造成"改不了密码"的锁死风险。**建议不实现纯 UI 壳**。

### Interface (1) / (2)（界面）

| KeePass 项                                      | SecPivot 状态                                                                                             |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| UI language / theme                             | ✅ `general.language` / `theme`                                                                           |
| Minimize to tray                                | ✅ `security.minimizeToTray`                                                                              |
| Show recent files / max count                   | ✅ `general.recentFiles`（上限 8）                                                                        |
| Always on top                                   | ✅ `general.alwaysOnTop`（本批实现）：Appearance → Window 开关，经 `set_always_on_top` Tauri 命令即时生效 |
| Drop to background after copy                   | ❌ 未实现（复制后切回上次窗口）                                                                           |
| Confirm before cmd:// / {CMD:...} / {REF:P@...} | ❌ 未实现（TCATO/URL 执行确认）                                                                           |
| Expiry soon days                                | ❌ 未实现（KeePass 默认 7 天；影响"即将过期"高亮）                                                        |
| Toolbar / columns config                        | ✅ `toolbarItems` / `entryColumns`                                                                        |
| Font sizes / density                            | ✅ `fontSizes` / `density`                                                                                |
| Window effect / opacity                         | ✅ `windowEffect` / `windowOpacity`                                                                       |

### Integration（集成）

| KeePass 项                                  | SecPivot 状态                                                                                                                                                                                                                            |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Auto-Type global hotkey                     | ✅ `keyboard.autoTypeGlobal`                                                                                                                                                                                                             |
| TCATO summon hotkey                         | ✅ `keyboard.tcatoSummonGlobal`（KeePass 双通道混淆对等）                                                                                                                                                                                |
| Browser integration                         | ✅ `bridge` (KeePassHttp) / `rpc` (KeePassRPC)                                                                                                                                                                                           |
| URL overrides                               | ✅ `integration.urlOverrideEnabled` / `urlOverride`（单一全局模板）/ `builtInOverridesEnabled`（27 内置）/ `customOverrides`；桌面 `open_entry_url`：条目模板优先于启用的全局模板，展开后匹配一次内置/自定义协议规则；开关仅控制全局模板 |
| Auto-Type inter-key delay / abort-on-window | ❌ 未实现（KeePass.config.xml-only 高级项）                                                                                                                                                                                              |

### Advanced（高级）

| KeePass 项                                                                        | SecPivot 状态                                                                                           |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Auto-save after modifying an entry（KeePass Options → Advanced → Start and Exit） | ✅ `security.autoSaveAfterEdit`（本批实现）：编辑/新增成功后走页面守卫的 `vault.save()`；失败保留脏状态 |
| Backup on save / count / template                                                 | ⚠️ 仅 remote profiles 有 `backupCount/backupTemplate`；本地保存备份未实现                               |
| Auto-reload changed files                                                         | ❌ 未实现                                                                                               |
| History retention                                                                 | ✅ 数据库级（Database Settings）                                                                        |
| Recycle bin                                                                       | ✅ 数据库级（Database Settings）                                                                        |
| Plugin / trigger system                                                           | ❌ 不适用                                                                                               |

## 推荐落地顺序（可验证、不造壳）

1. ✅ **defaultExpireDays**（完成，510416c）
2. ~~Expiry soon days~~：放弃——查证后 SecPivot 无"即将过期"消费点，强加会变成纯 UI 壳，违反"不造壳"门禁。
3. ✅ **Always on top**（完成，0b4a790）：`general.alwaysOnTop` 默认 false，Appearance → Window 开关，settings-bootstrap 经 `set_always_on_top` Tauri 命令即时应用。
4. ✅ **Auto-save after edit**（本批完成）：`security.autoSaveAfterEdit` 默认 false，编辑/新增成功后调用页面守卫的 `vault.save()`，失败保留脏状态可手动重试。
5. ✅ **Lock on minimize to tray**（本批完成）：`security.lockOnMinimizeToTray` 默认 false，隐藏到托盘时复用 `tray-lock` 事件锁定，UI 依赖 minimizeToTray 开关。
6. **Drop to background after copy**：前端复制后聚焦上一个窗口，需要平台支持（Win32 可在 Rust 侧实现），风险中等。
7. **URL overrides / auto-type inter-key delay**：低优先级，需新增 IPC 与执行链改造。

不建议实现：Policy 强制策略（会把个人用户锁死）、Secure desktop（高风险高成本）、纯 UI 无行为的占位开关。
