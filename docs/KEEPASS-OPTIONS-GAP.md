# KeePass Options ↔ SecPivot Settings Gap Analysis

> 本文只覆盖 Options 对话框可见项。KeePass `KeePass.config.xml` 的完整配置面（源码级穷举，字段过百）见 [KEEPASS-CONFIG-INVENTORY.md](KEEPASS-CONFIG-INVENTORY.md)。

证据来源：KeePass 2.57 官方手册镜像 [keepass.cool 2.57](https://keepass.cool/2.x/2.57/Lapersonnalisation.html)、[keepass.info search 帮助](https://keepass.info/help/base/search.html#mode_se)、官方 changelog 转载（[2.39 Large Entries](https://www.neowin.net/news/keepass-password-safe-2391/#1)、[2.53 报告对话框筛选框](https://www.3dfxzone.it/files/?object=html&objid=21228)）、开源镜像 [dlech/KeePass2.x](https://github.com/dlech/KeePass2.x)。外部资料只作导航，最终以 SecPivot 本地源码为准。

## KeePass Options 六个 tab 与 SecPivot 现状

### Security（安全）

| KeePass 项                                | SecPivot 状态                                                                                                  | 证据                                        |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| Lock workspace after inactivity (seconds) | ✅ `security.autoLockMinutes`                                                                                  | SecuritySettingsPanel SettingRangeCard      |
| Lock after **global user inactivity**     | ❌ 未实现（需要系统级空闲检测，WebView 内不可靠）                                                              | —                                           |
| Clipboard auto-clear time                 | ✅ `security.clipboardClearSeconds`                                                                            | security.ts copyText                        |
| By default, new entries expire in N days  | ✅ **`security.defaultExpireDays`**（本轮 510416c 新增）                                                       | EntryEditorDialog create mode 预填          |
| Lock when minimizing to tray              | ✅ `security.lockOnMinimizeToTray`（本批实现）：隐藏到托盘时复用 `tray-lock` 事件锁定；需先启用 minimizeToTray | src-tauri/src/lib.rs handle_close_requested |
| Lock when minimizing to taskbar           | ⚠️ `lockOnFocusLoss` 靠 window blur 覆盖                                                                       | security.ts installFocusLock                |
| Lock on workstation lock / user switch    | ⚠️ blur 通常伴随，非显式事件                                                                                   | —                                           |
| Lock on suspend / remote-control change   | ❌ 需原生系统事件                                                                                              | —                                           |
| Exit instead of lock / Always exit        | ❌ 未实现                                                                                                      | handle_close_requested 恒为退出/托盘        |
| Secure desktop for master key             | ❌ 未实现（Windows 安全桌面，成本高）                                                                          | —                                           |
| Warn on weak key transformation           | ❌ 未实现（创建/改密钥时告警）                                                                                 | —                                           |

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
