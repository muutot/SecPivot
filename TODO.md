# SecPivot Roadmap

Status legend: `[ ]` pending · `[x]` delivered (with direct evidence) · `[~]` partial/blocked.

## Stage 1 — Project scaffold (delivered)

- [x] Svelte 5 + SvelteKit (adapter-static SPA) + Tauri 2 project skeleton
- [x] Rust backend crate (config + vault session module wiring)
- [x] Theme token system (20 semantic colors, dark/light presets, custom mapping)
- [x] Shared settings primitives (`settings-shared.css`)
- [x] Settings shell + General/Security/Database/About panels
- [x] Welcome/unlock flow with open + create database modals
- [x] Three-pane main layout: group tree / entry list / detail
- [x] Entry editor (create/edit/delete), group create/rename/delete
- [x] Search across title/username/url/notes, group filter
- [x] Password generator with entropy readout; copy with scheduled clipboard clear
- [x] Skill + references + repository documentation

## Stage 2 — Backend vault engine

- [x] `open_vault` / `create_vault` / `save_vault` / `close_vault` via `keepass` crate (KDBX 4.0)
- [x] Entry and group CRUD commands with in-memory session
- [x] Rust tests: round-trip save, CRUD, wrong-password rejection, session clear
- [x] Wire `src/lib/services/vault.ts` to real commands behind `isTauriRuntime()`

## Stage 3 — Lock & clipboard security (delivered)

- [x] Idle auto-lock timer driven by `autoLockMinutes` (`armIdleLock`/`installAutoLock`, `src/lib/services/security.ts`)
- [x] Lock clears clipboard when `clearOnLock` (`lockVault`)
- [x] `lockAfterAction` after password copy (`copySensitive`, wired for password copies only)
- [x] Frontend lock screen + reopen with remembered path only (`LockScreen.svelte`, `vault.remembered`)

## Stage 4 — Search & productivity

- [x] TOTP display with countdown for `otp`/`totp` fields (`totp_code` command + `TotpWidget`)
- [x] Password strength meter in entry editor (`estimateEntropy`/`entropyLabel`, `password.ts`)
- [x] URL quick-open via `@tauri-apps/plugin-opener` (detail + list rows)
- [x] Autotype sequence runner (`auto_type` + `autotype.rs`: KeePass placeholders/keys, `enigo` replay, 7 parser tests)
- [x] Favorite/pin entries with `--warning-color` accent (`toggle_favorite` + `SecPivot.Favorite` field)

## Stage 5 — Packaging & release

- [x] App icons (committed `src-tauri/icons/*`), bundle branding metadata (`publisher`/`copyright`/descriptions), custom NSIS template (`src-tauri/windows/installer.nsi`) — verified: `tauri build` produced `SecPivot_0.1.0_x64-setup.exe`
- [ ] GitHub Actions CI mirroring `npm run verify` (`.github/workflows/ci.yml` added; unverified in this environment — no `origin` remote to run it)
- [ ] Release workflow via version-release skill (`skills/version-release` + `scripts/*.mjs` added; `release.mjs --dry-run` verified through step 3, tag/push unverified — no remote)

## Stage 6 — Remote vaults (S3 / WebDAV)

- [x] Remote settings panel (`RemoteSettingsPanel.svelte`; S3/WebDAV 二级页签与各自 endpoint/credentials/prefix/backup 配置，凭据在 Windows 上经 DPAPI 加密落盘)
- [x] Remote transport: `RemoteStorage` trait + `S3Storage` (rust-s3 0.34, path-style for MinIO) + `WebDavStorage` + `MemoryStorage` fake
- [x] `open_remote_vault` / `create_remote_vault` / `s3_list_objects` commands; `save()` uploads back through the selected S3/WebDAV transport for remote sessions
- [x] Save modes: `memory` (upload back only) / `local` (mirror to `Storage/remote/<kind>/<config>` with timestamped `.bak` rotation, `backupCount`)
- [x] Welcome-screen remote browser: list S3/WebDAV files, open (password + keyfile) and create remote vaults
- [~] Live S3 end-to-end verification (no docker/minio/aws in this environment; transport now covered by a local mock HTTP S3 server test: ListObjectsV2 XML parsing, path-style signing, get/put, bounded-timeout behavior — `remote::tests::*`; real-provider behavior still unverified)
- [x] 多 profile 远程配置 (`remoteProfiles` + `activeRemote`: S3/WebDAV 二级分组,单 profile 单协议字段,规范路径 `s3/config_1` / `webdav/config_1`,命令按路径从 `ConfigStore` 解析凭据且不跨 IPC,设置页与欢迎页均可添加/重命名/删除/切换)
- [x] 任意文件可作数据库/密钥文件:远程打开/创建键名不再要求 `.kdbx` 后缀(由 KDBX 解析判定,非库文件报「无法打开数据库」),本地打开/创建选择器加「所有文件」选项,密钥文件选择器(欢迎页 + 改主密钥)放开任意类型

## Stage 7 — Feature gap list (priority order)

- [x] 主密钥变更 (`change_master_key`, 支持密码/密钥文件与 Aes/Argon2id/Argon2、解密验证、会话保持)
- [x] 回收站 (条目/分组删除移入回收站,恢复条目/分组,清空回收站,跨重开持久化)
- [x] 条目过期提醒 (打开库时 flash 过期数量,列表过期标记,详情过期状态)
- [x] 条目历史版本 (每次修改自动快照,查看/恢复,最多保留 10 版)
- [x] 条目/分组图标与颜色标记 (KeePass 内置图标 0–68 + `#RRGGBB` 颜色,树/列表/详情/编辑器)
- [x] 条目拖拽移动分组 + 多选批量删除
- [x] 全局 Auto-Type 热键 (`tauri-plugin-global-shortcut`,按前台窗口标题匹配条目网址域名/标题,回收站条目不参与;设置项 `globalAutoTypeShortcut`)
- [x] 字段引用 `{REF:...}` 支持 + TCATO (two-channel auto-type;REF 支持 UUID/标准字段/自定义字段名检索,跳过回收站;TCATO 覆盖层窗口 + `WM_CHAR` 通道注入,密码不离开后端)
- [x] 防截屏 (窗口守卫):库打开期间主窗口 `WDA_EXCLUDEFROMCAPTURE` 守卫(窗口保持可见但不出现在截屏/录屏/共享中),锁定/关闭释放 (`shield.rs`);默认关闭,欢迎页可配置 (`security.screenCaptureGuard`,后端在 open/create 时读取;`WDA_MONITOR` 会导致物理屏黑块/窗口消失,已列入 PITFALLS)
- [x] DPAPI 加密本地配置:S3 密钥 `CryptProtectData` 加密落盘(`dpapi1:` 前缀),旧明文配置兼容读入,`remote_secrets_never_persist_in_plaintext` 测试
- [x] 便携版打包:`scripts/package-portable.ps1`(tauri build + 复制 exe + README,输出 `dist/SecPivot-<version>-portable.zip`,已验证 zip 内容)
- [x] 全选 + 多选批量编辑:条目列表 Ctrl+A 全选当前视图(分组/搜索过滤后)全部条目;右键「编辑所选条目」打开批量编辑器,字段值不一致时输入框显示「多个值」占位(KeePass 语义:未修改字段保持各条目原值,含密码/TOTP 不加载不覆盖),图标/颜色/过期/分组等可选属性支持显式清除;后端 `update_entries` 单事务原子应用(任一 uuid 无效整批不生效),174 后端测试通过
- [x] 另存为:工具栏「另存为」按钮 + 空白区右键菜单「另存为…」,系统对话框选路径后以当前密钥写入新文件并将会话切换到新目标(后续保存写新文件,原文件不变);远程会话另存后转为本地会话(S3 不再接收后续保存);保存失败会话不变,177 后端测试通过

## Stage 8 — 结构、性能与 KeePass 2.x 原生能力差距（按实施优先级）

产品约束：暂不接入通用插件系统；优秀插件能力仅在安全边界、维护成本和产品价值明确时作为原生功能实现。KDBX `CustomData` 继续只读保真，不提供任意插件元数据编辑器。

### P0 — 当前架构放大器与前端一致性

- [x] 批量分组展开/折叠：新增单次 IPC 的批量命令，一次事务写入所有目标分组的 `isExpanded`，前端「全部展开/全部折叠」不得再为每个分组接收一份完整 `VaultState`；补后端原子性/未知 UUID 测试与前端 browser fallback 等价行为
- [x] 条目列表窗口化：提取 `EntryTable`/表格状态逻辑，仅挂载可视区及缓冲行；保持固定列宽、横向滚动、列排序/拖拽/缩放、多选、Ctrl+A、键盘导航、条目拖拽和移动端摘要布局语义
- [ ] 统一弹窗基础设施：新增使用现有 theme tokens 的 `ModalShell`（header/body/actions、size/tone），迁移重复的 `.modal-head/.modal-icon/.modal-actions/.modal-button/.text-input`，禁止形成第二套圆角、阴影、按钮和输入框样式
- [ ] 统一 viewport 菜单与凭据表单：抽取 `ViewportMenuShell`（viewport clamp、Escape、click-outside）供右键菜单/列配置复用；抽取欢迎页与锁屏共用的 `StandaloneVaultShell`/`VaultCredentialFields`
- [x] 低风险性能批次：复用单例 `Intl.Collator` 并预计算当前排序列 key；`GroupPicker` 改用一次性 entry-count map；导入路径建立 group-path 索引；favicon 每次命令复用一个 `reqwest::Client`/连接池
- [x] mutation 自定义图标拆分缓存：后端 mutation 快照省略 `customIcons` 图像负载（`Option::None`），前端 `vault.ts` 维护图标缓存并在权威快照/轻量结果间合并；favorite/展开/CRUD 不再跨 IPC 重传全部 favicon，附 `light_mutation_snapshots_omit_custom_icons` 测试
- [x] `VaultState.revision`：后端在每次 mutation 递增并随快照（含轻量 mutation 结果）返回，前端类型/浏览器回退同步，测试断言 `mutated.revision > full.revision`
- [x] mutation result/delta：`toggle_favorite`/`set_group_expanded`/`set_groups_expanded` 改为返回 `MutationDelta`（`kind` + `revision` + 受影响 id），前端 `applyBackendDelta` 在本地 `VaultState` 上应用，收藏/展开不再重建、编码并跨 IPC 传输完整树；附 favorite/group-expand delta 测试

### P1 — 数据安全、核心 KeePass 工作流与高价值原生能力

- [~] 官方同步/冲突语义：已完成 KeePass 条目级 merge、冲突历史保留和外部修改检测调研；待实现本地/远程版本检测、同步或覆盖提示、条目级合并及冲突测试。当前远程配置/transport 重构由用户并行处理，完成前不得修改或提交相关文件
- [x] Auto-Type 后端读写与窗口关联：`VaultEntry.autoType`/`VaultGroup.autoType` 暴露、`update_entry_autotype`/`update_group_autotype` 命令、`resolve_autotype_sequence_for_window`（关联优先 + `*` 通配），全局热键改用窗口关联解析；round-trip/继承/关联选择测试通过
- [x] Auto-Type 编辑器 UI（条目）：条目编辑器新增「自动填充」页签（enable 开关、默认序列、窗口关联增删改），保存后调用 `update_entry_autotype` 持久化
- [x] Auto-Type 编辑器 UI（分组）：分组右键「自动填充设置」对话框（继承/启用/禁用 + 默认序列），调用 `update_group_autotype` 持久化
- [x] Auto-Type 全局热键多命中选择：多条目命中时 `autotype-pick-request` 事件 + 候选列表 + `autotype_pick` 命令 + 前端选择对话框，回收站条目排除
- [x] 当前数据库设置（读取）：`get_database_settings` 返回 KDF/cipher/compression/historyMaxItems/recycleBinEnabled，覆盖 Aes/Argon2/Argon2id 与 Aes256/Twofish/ChaCha20/None/Gzip 映射及关闭会话 `None`
- [x] 当前数据库设置（修改·部分）：`update_database_settings` 支持 `historyMaxItems`/`recycleBinEnabled` 部分写入与 `null` 重置，保存/重开 round-trip 测试通过
- [x] 当前数据库设置（修改·KDF/cipher/compression）：`update_database_settings` 对克隆库应用新存储配置并同密钥重加密，成功后采纳；Aes→Argon2/ChaCha20/Gzip 保存重开 round-trip 测试通过
- [x] 当前数据库设置（UI）：「数据库设置」对话框（空白区右键 + More 菜单入口）展示并编辑 KDF/cipher/compression/history 上限/回收站开关，按差异调用 `update_database_settings`
- [x] 当前数据库设置（修改·history size/模板组）：`historyMaxSize`/`entryTemplatesGroup` 读取、写入与 `null` 重置，UUID 校验，保存/重开 round-trip 测试通过；UI 已含对应输入
- [x] 高级搜索过滤引擎：`matchesAdvancedSearch` 支持字段范围（含自定义字段）、正则、排除取反、过期/收藏/标签/质量条件；Node 行为测试覆盖
- [x] 高级搜索 UI：搜索框旁「高级搜索」入口 + 过滤对话框（字段范围/正则/排除/过期/收藏/标签/质量），应用于当前视图，快速搜索保持轻量
- [x] 保存搜索：命名搜索配置持久化（设置契约 + 列表加载/删除）
- [x] 密码生成器规则引擎：`customCharset`/`excludeChars`/`requiredChars`/`pattern`（u/l/d/s/a + 字面量）已入设置契约（TS+Rust serde 均保留，round-trip 测试）并在 `generatePassword` 中执行；Node 行为测试覆盖自定义池/必含/排除/pattern/非法必含
- [x] 密码生成器配置档（存储）：`DatabaseDefaults.generatorProfiles` 命名 profiles（TS+Rust serde 均保留，长度归一化，round-trip 测试）
- [x] 密码生成器配置档（UI）：数据库设置面板「密码配置档」支持新建/编辑/删除/设为默认（含自定义字符集、排除、必含、pattern）
- [x] 密码生成器规则引擎（Rust）：`generate_password_with(&PasswordGeneratorSettings)` 镜像 TS 规则（自定义字符集/排除/必含/pattern），OS RNG 取随机，覆盖默认包装与错误必含测试
- [x] 密码生成器配置接线：`BridgeState`/`RpcState` 在配置同步时保存生成器设置，KeePassHttp/RPC `GeneratePassword` 经 `handle_request_with_generator`/`handle_jsonrpc_with_generator` 使用用户规则；含 `generate_password_honors_configured_generator` 测试
- [x] 多数据库标签页·会话注册表：`VaultSessions` 托管状态停放非活动会话；`open_vault`/`create_vault`/远端 open/create 返回 `sessionId` 并切换 active；`close_vault`/`get_vault_state` 支持按 sessionId（缺省 active，关闭 active 后自动提升最后停放会话）；round-trip 测试覆盖两库并存
- [x] 多数据库标签页·会话切换：`set_active_session` 命令在 active 与停放会话间交换（parked ↔ active），保存/数据库设置/条目/分组/favicon/tcato 等命令天然作用于切换后的 active；前端 vault.ts 增加 `setActiveSession(id)` 并同步 remembered
- [x] 多数据库标签页·前端标签状态：vault.ts 增加 `tabs`/`activeId` store 与 `setActiveSession`/`closeTab`，后端 `list_sessions` 返回标签列表（含 dirty），`VaultTabs` 标签栏（文件名/dirty 标记/关闭/切换，多于一个标签时显示）
- [ ] 多数据库标签页·锁定与可见性：锁定/关闭策略与标签联动，bridge/RPC 仅服务 active 会话，quick-reopen 联动

### P2 — KDBX 属性完整度与数据交换

- [ ] 完整条目/分组属性编辑：开放 `OverrideURL`、`QualityCheck`、前景/背景色、group notes/tags、enableSearching、group Auto-Type 等可写契约；`CustomData` 仍只读并覆盖跨客户端保真测试
- [ ] 安全附件预览/临时打开：文本/图片等优先内存预览；外部打开需显式确认、受控临时目录、关闭后导入或丢弃修改并可靠清理，不记录附件内容或密码
- [ ] 扩充导入/导出：优先支持 Bitwarden、1Password、LastPass，随后增加 KDBX/XML/HTML/打印/应急表；所有明文导出必须给出明确安全提示
- [ ] 数据库维护：相似密码、历史清理、过期维护、损坏库修复/尽力恢复，并为不可恢复写入设计只读失败路径

### P3 — 受约束自动化与可选安全增强

- [ ] 原生事件规则：提供有限的事件—条件—动作（打开/保存/锁定/定时、备份/同步/显示筛选），初期不开放任意脚本、任意命令执行或动态代码加载
- [ ] HIBP 泄露检查：严格 opt-in，仅使用 k-anonymity 前缀查询，绝不发送密码或完整散列；支持离线关闭和隐私说明
- [ ] 密钥文件生成/纸质备份与安全主密钥输入；Windows 用户账户密钥、YubiKey challenge-response 等在兼容性和恢复方案明确后逐项原生评估

## 后续候选(差距清单已清空)

- 账户绑定 (Hardware-bound, TPM) —— 用户暂缓
- 其余见 roadmap

## 浏览器集成(调研完成,提案见 `docs/browser-integration.md`)

- [~] Phase 1 — KeePassHttp 兼容协议:后台 loopback HTTP 服务(`associate`/`test-associate`/`get-logins`/`get-logins-count`/`set-login`/`generate-password`),AES-256-CBC 逐字段加密 + HMAC-SHA256;匹配复用 `VaultSession::autotype_match` 评分,skips 回收站;associate 密钥存会话内、锁定即销毁。已交付:`bridge.rs`(协议核心,19 测试含 NIST SP 800-38A/RFC 4231 向量)、`bridge_server.rs`(127.0.0.1:19455 服务 + 审批板 + 生命周期,7 测试)、`VaultSession` 桥接(5 测试)、设置「集成」面板(开关/状态/已授权客户端管理)+ 全局关联审批提示,共 132 后端测试通过 (🚧 真机浏览器扩展验证不可行,离线仅协议级测试)
- [~] Phase 2 — KeePassRPC 兼容(SRP-6a + WebSocket :12546 + AES JSON-RPC)——**已立项**:协议规格已从 Kee 4.0.7 官方扩展源码提取(`docs/browser-integration.md` Phase 2 细则)。实施顺序:rpc.rs 协议核心(SRP-6a 1024-bit 群 + 密钥认证 + AES-256-CBC/HMAC-SHA1 帧 + v1 方法/DTO)→ rpc_server.rs(WS loopback + 生命周期 + 会话密钥驻留)→ 前端(设置分区 + SRP 旁路密码对话框)→ 文档与验证。已交付后端:`rpc.rs`(协议核心,8 测试含 JS 镜像客户端 SRP 往返)、`rpc_server.rs`(127.0.0.1:12546 WSS + 握手状态机 + 旁路密码事件 `rpc-side-channel-request` + 原始 socket WS 冒烟测试,6 测试)、`VaultSession` RpcHost(2 测试)、`rpc` 配置开关 + `rpc_status` 命令,共 152 后端测试通过;已交付前端:`RpcSettingsPanel`(集成页 KeePassRPC 开关/状态,`rpc` 设置类型/归一化/`updateRpc`)、`RpcSideChannelPrompt`(旁路密码弹窗,倒计时/复制,挂载于 `+layout.svelte`,TCATO 覆盖层跳过) (🚧 真机 Kee 扩展验证不可行,离线仅协议级自洽测试 + 手动 UI 验证)
- [~] Phase 2b (post) — KeePassRPC 写路径(AddLogin/UpdateLogin)、BROWSER_SETTINGS_SYNC 评估——**已交付**:规格已从 KeePassRPC 1.12 插件源码(`KeePassRPCService.JSONRPC.cs`/`KeePassRPCService.cs` `MergeEntries`/`MergeInNewURLs`/`KeePassRPCService.DTOV1.cs` `setPwEntryFromEntry`)与 Kee 4.0.7 扩展源码提取(`docs/browser-integration.md` Phase 2b 细则)。已交付:`rpc.rs` 写路径 DTO(`RpcLoginWrite`/`RpcFieldWrite`,serde 精确还原 `uRLs`/`hTTPRealm`/`formFieldList`)、`merge_urls` 纯函数(5 模式 × `MergeInNewURLs` 提升语义)、`AddLogin`/`UpdateLogin` 调度(插件 `ArgumentException` 镜像校验 + `-32001`/`-32002` 错误)、特性 `KPRPC_FEATURE_ENTRY_URL_REPLACEMENT`(Kee 将发 urlMergeMode=5);`vault.rs` `RpcHost` 写路径(字段映射 `setPwEntryFromEntry`:首 FFTpassword→Password、FFTusername→UserName 后者胜、余字段→displayName 自定义字符串、URL 空格拼接;父分组解析(空/无效/回收站→根回退);更新走 `edit_tracking` 历史快照 = 插件 `CreateBackup`;回收站条目拒绝 = 防御纵深),共 162 后端测试通过(新增 10)。已记录偏差:附加字段存 KDBX 字符串而非插件私有 KPRPC JSON 配置;更新覆盖字段但不清除陈旧自定义字段。(BROWSER_SETTINGS_SYNC 结论:仅客户端特性标志,桌面 loopback 无设置同步通道,不实现不宣告;Kee Vault 事件会话与 v2 方法族超出桌面范围,不实现)
