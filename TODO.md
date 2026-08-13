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
- [~] GitHub Actions CI：远端运行 [31619349103](https://github.com/muutot/SecPivot/actions/runs/31619349103) 已在 `13fb228` 通过 Frontend/Rust jobs；当前工作流已覆盖 `npm run verify` 的格式、Svelte/Vite、前端行为测试、Rust 测试与 clippy 门禁，但当前 HEAD 尚无远端运行证据
- [~] Release workflow：远端运行 [31619359721](https://github.com/muutot/SecPivot/actions/runs/31619359721) 的 Verify 与 Windows x64 build 已通过，`v1.2.0` draft Release 已包含 NSIS 与便携 ZIP；Android build 因 NDK `llvm-ranlib` 未传给 `openssl-src` 而失败。当前工作流已修正 NDK 工具、四 ABI universal APK、签名与产物硬门禁，仍待当前 HEAD 的远端运行及 APK 资产证据

## Stage 6 — Remote vaults (S3 / WebDAV)

- [x] Remote settings panel (`RemoteSettingsPanel.svelte`; S3/WebDAV 二级页签与各自 endpoint/credentials/prefix/backup 配置，凭据在 Windows 上经 DPAPI 加密落盘)
- [x] Remote transport: `RemoteStorage` trait + `S3Storage` (rust-s3 0.34, path-style for MinIO) + `WebDavStorage` + `MemoryStorage` fake
- [x] `open_remote_vault` / `create_remote_vault` / `s3_list_objects` commands; `save()` uploads back through the selected S3/WebDAV transport for remote sessions
- [x] Save modes: `memory` (upload back only) / `local` (mirror to `Storage/remote/<kind>/<config>` with timestamped `.bak` rotation, `backupCount`)
- [x] Welcome-screen remote browser: list S3/WebDAV files, open (password + keyfile) and create remote vaults
- [x] Live S3 end-to-end verification: `s3_transport_round_trips_against_live_s3_server` 对真实本地 S3 服务器(MinIO `B:\Program\s3\minio` :9000,path-style,`rustfsadmin`/`rustfsadmin`)跑通 建桶→put→list→get→删对象→删桶 全链路(测试运行时观察到 MinIO 磁盘上 `secpivot-live-<pid>` 桶目录出现并随清理消失,测试带 `--nocapture` 无 skip);原 rustfs 1.0.0-beta.12 服务端在 Windows 上存储层无法就绪(format 文件加载命中 Windows 共享冲突 `os error 32`,重试 10 次后仍 `storage_quorum` 未就绪,见 `docs/PITFALLS.md`),故以 MinIO 为本地真实服务器验证
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
- [~] 统一弹窗基础设施：通用业务对话框已迁移到 `ModalShell`（header/body/actions、size/tone）+ `modal-shared.css`（`.text-input`/`.modal-actions`/`.modal-button` primary/danger），包括 EntryEditor/GroupMeta/附件预览/相似密码/HIBP/过期条目等；`VaultWelcome` 的凭据弹窗仍保留独立表面与同名局部样式，受「VaultWelcome/LockScreen 不动」约束暂缓统一
- [ ] 统一凭据表单：`ViewportMenuShell` 已供右键菜单/列配置复用（ContextMenu、ColumnConfigMenu）；欢迎页与锁屏共用的 `StandaloneVaultShell`/`VaultCredentialFields` 抽取受「VaultWelcome/LockScreen 不动」约束，暂缓
- [x] 低风险性能批次：复用单例 `Intl.Collator` 并预计算当前排序列 key；`GroupPicker` 改用一次性 entry-count map；导入路径建立 group-path 索引；favicon 每次命令复用一个 `reqwest::Client`/连接池
- [x] mutation 自定义图标拆分缓存：后端 mutation 快照省略 `customIcons` 图像负载（`Option::None`），前端 `vault.ts` 维护图标缓存并在权威快照/轻量结果间合并；favorite/展开/CRUD 不再跨 IPC 重传全部 favicon，附 `light_mutation_snapshots_omit_custom_icons` 测试
- [x] `VaultState.revision`：后端在每次 mutation 递增并随快照（含轻量 mutation 结果）返回，前端类型/浏览器回退同步，测试断言 `mutated.revision > full.revision`
- [x] mutation result/delta：`toggle_favorite`/`set_group_expanded`/`set_groups_expanded` 改为返回 `MutationDelta`（`kind` + `revision` + 受影响 id），前端 `applyBackendDelta` 在本地 `VaultState` 上应用，收藏/展开不再重建、编码并跨 IPC 传输完整树；附 favorite/group-expand delta 测试

### P1 — 数据安全、核心 KeePass 工作流与高价值原生能力

- [x] 官方同步·远程变更检测：打开/创建远程库记录内容 SHA-256（base hash），保存/改密/存储重加密前 `get` 比对，远程已变更返回 `REMOTE_CHANGED` 冲突错误（附远程/本地大小），成功后推进 base hash；冲突不计入只读降级
- [x] 官方同步·冲突解析 UI：保存冲突弹窗「覆盖远程（force）/ 下载远程（refresh，确认丢弃本地未保存修改）/ 取消（保留本地）」，`save_vault(force)` 与 `refresh_remote_vault` 后端命令 + `vault.save(force)`/`vault.refreshRemote()` 前端方法
- [x] 官方同步·条目级合并：按条目 UUID + 字段 last-modified 合并本地与远程（历史保留、回收站排除），冲突测试覆盖同改/单改/删除；`merge_databases` 纯函数 + `VaultSession::merge_remote` + `merge_remote_vault` 命令，冲突弹窗新增「合并本地与远程」入口
- [x] Auto-Type 后端读写与窗口关联：`VaultEntry.autoType`/`VaultGroup.autoType` 暴露、`update_entry_autotype`/`update_group_autotype` 命令、`resolve_autotype_sequence_for_window`（关联优先 + `*` 通配），全局热键改用窗口关联解析；round-trip/继承/关联选择测试通过
- [x] Auto-Type 编辑器 UI（条目）：条目编辑器新增「自动填充」页签（enable 开关、默认序列、窗口关联增删改），保存后调用 `update_entry_autotype` 持久化
- [x] Auto-Type 编辑器 UI（分组）：分组右键「自动填充设置」对话框（继承/启用/禁用 + 默认序列），调用 `update_group_autotype` 持久化
- [x] Auto-Type 全局热键多命中选择：多条目命中时 `autotype-pick-request` 事件 + 候选列表 + `autotype_pick` 命令 + 前端选择对话框，回收站条目排除
- [x] 当前数据库设置（读取）：`get_database_settings` 返回 KDF/cipher/compression/historyMaxItems/recycleBinEnabled，覆盖 Aes/Argon2/Argon2id 与 Aes256/Twofish/ChaCha20/None/Gzip 映射及关闭会话 `None`
- [x] 当前数据库设置（修改·部分）：`update_database_settings` 支持 `historyMaxItems`/`recycleBinEnabled` 部分写入与 `null` 重置，保存/重开 round-trip 测试通过
- [x] 当前数据库设置（修改·KDF/cipher/compression）：`update_database_settings` 对克隆库应用新存储配置并同密钥重加密，成功后采纳；Aes→Argon2/ChaCha20/Gzip 保存重开 round-trip 测试通过
- [x] 当前数据库设置（UI）：「数据库设置」对话框（空白区右键 + More 菜单入口）展示并编辑 KDF/cipher/compression/history 上限/回收站开关，按差异调用 `update_database_settings`
- [x] 当前数据库设置（修改·history size/模板组）：`historyMaxSize`/`entryTemplatesGroup` 读取、写入与 `null` 重置，UUID 校验，保存/重开 round-trip 测试通过；UI 已含对应输入
- [ ] 当前数据库设置（KDF benchmark）：尚未实现目标耗时基准测试、参数建议/应用流程及对应验证；不得以固定 Argon2/AES-KDF 参数代替 benchmark 完成证据
- [x] 高级搜索过滤引擎：`matchesAdvancedSearch` 支持字段范围（含自定义字段）、正则、排除取反、过期/收藏/标签/质量条件；`tests/entry-search.test.mjs` 覆盖字段隔离、大小写、非法正则与组合条件
- [x] 高级搜索 UI：搜索框旁「高级搜索」入口 + 过滤对话框（字段范围/正则/排除/过期/收藏/标签/质量），应用于当前视图，快速搜索保持轻量
- [x] 保存搜索：命名搜索配置持久化（设置契约 + 列表加载/删除）
- [x] 密码生成器规则引擎：`customCharset`/`excludeChars`/`requiredChars`/`pattern`（u/l/d/s/a + 字面量）已入设置契约（TS+Rust serde 均保留，round-trip 测试）并在 `generatePassword` 中执行；`tests/password.test.mjs` 覆盖类别开关/符号保证、自定义池/必含/排除、pattern 槽约束与不可能策略显式失败
- [x] 密码生成器配置档（存储）：`DatabaseDefaults.generatorProfiles` 命名 profiles（TS+Rust serde 均保留，长度归一化，round-trip 测试）
- [x] 密码生成器配置档（UI）：数据库设置面板「密码配置档」支持新建/编辑/删除/设为默认（含自定义字符集、排除、必含、pattern）
- [x] 密码生成器规则引擎（Rust）：`generate_password_with(&PasswordGeneratorSettings)` 镜像 TS 规则（类别开关/符号保证、自定义字符集/排除/必含/pattern），两端均以拒绝采样从 OS RNG 取得无偏索引；空池、容量不足和不兼容 pattern 均显式失败，Rust 行为测试覆盖
- [x] 密码生成器配置接线：`BridgeState`/`RpcState` 在配置同步时保存生成器设置，KeePassHttp/RPC `GeneratePassword` 经 `handle_request_with_generator`/`handle_jsonrpc_with_generator` 使用用户规则；无效配置返回协议错误而不静默回退默认策略，Bridge/RPC 测试覆盖成功与失败路径
- [x] 多数据库标签页·会话注册表：`VaultSessions` 托管状态停放非活动会话；`open_vault`/`create_vault`/远端 open/create 返回 `sessionId` 并切换 active；`close_vault`/`get_vault_state` 支持按 sessionId（缺省 active，关闭 active 后自动提升最后停放会话）；round-trip 测试覆盖两库并存
- [~] 多数据库标签页·会话切换：核心隔离已实现并有直接测试，但本轮审查发现拓扑操作与待处理切换仍可能交错，且若干前端已提交状态/切换清理路径尚需修正；以下子项全部完成后再恢复 `[x]`
  - [x] renderer 会话内命令捕获并传递稳定 `sessionId`，后端统一按 id 路由；相同 KDBX 副本（相同 UUID）回归证明 mutation 只影响指定会话
  - [x] 异步结果按 session + revision/替换 epoch 拒绝旧回写；远程整库替换提升 revision，前端行为测试覆盖旧 revision/旧 epoch
  - [x] 标签切换把“未缓存快照验证 → backend active 交换 → frontend 发布”作为一个排队单元；快速 A→B→A 与快照失败不执行交换均有前端行为测试
  - [x] 长时 save/save-as/change-key/favicon、复合编辑链、TOTP、TCATO/全局候选与 owner 校验的附件导入保持发起 session 绑定
  - [x] open/create/openRemote/createRemote/close/closeTab/closeAll 与标签切换共用一个拓扑队列，后端注册表变更及对应前端发布按调用顺序完整执行
  - [x] `refreshTabs` 清理已不存在的 active id；refresh/save/save-as/change-key/setActiveSession 返回 revision/epoch 门禁实际采纳的状态，而不是被拒绝的晚到结果
  - [x] 标签切换清理页面共享 `busy`、分组创建/图标保存状态；旧 session 的 finally 不会让新标签持续禁用工具栏或表单
  - [x] 条目编辑器等待父级复合保存 Promise 并在保存期阻止重复提交/取消；父级完成回调绑定视图代次，A→B→A 的旧保存不再关闭新 A 编辑器或改写其选择/提示
  - [x] CSV/XML/Bitwarden/1Password 文件选择与异步解析绑定发起视图代次；切换标签（含 A→B→A）后旧结果不启动导入，多级分组创建的每次写入均固定到原 session
  - [x] “另存为”在打开原生保存对话框前捕获可见视图代次；A→B→A 后第一轮 A 的旧路径选择不会触发 `save_vault_as`，完成提示/错误也不写入新视图
  - [x] CSV/HTML 明文导出在原生保存对话框前捕获可见视图代次；旧路径选择不启动导出，应急表同时固化 `includePasswords`，不会读取切换后新弹窗的勾选值
  - [x] 详情密码/受保护自定义字段仅在对应秘密读取成功后切换 reveal；加载中重复点击或 session/UUID 失效不再显示空值
  - [x] `EntryDetail` 密码/受保护字段/历史/存储请求绑定 session+UUID 视图代次；同库快速 u1→u2→u1 的第一轮晚到响应不再写入第二轮 u1 视图
  - [x] 条目列表/右键密码复制在读取后、写入剪贴板前再次校验原可见视图代次；A→B→A 的第一轮 A 晚到密码不会进入第二轮 A 的剪贴板，前端行为测试覆盖 stale/current consumer
  - [x] `EntryDetail` 密码/受保护字段复制让秘密读取与剪贴板 consumer 共用 session+UUID 视图代次，并在详情卸载时失效 guard；旧详情的晚到秘密不会由已分离组件写入剪贴板
  - [x] 附件预览弹窗在按钮关闭、Escape、父级标签切换卸载及重新外部打开时清理旧 token；导入失败保留 token 供重试，不丢失受控临时文件引用
  - [ ] 其余常驻详情/弹窗的异步 loading 状态在 session/UUID 变化时完整重置，并补相称行为验证
- [x] 多数据库标签页·前端标签状态：vault.ts 增加 `tabs`/`activeId` store 与 `setActiveSession`/`closeTab`，后端 `list_sessions` 返回标签列表（含 dirty），`VaultTabs` 标签栏（文件名/dirty 标记/关闭/切换，多于一个标签时显示）
- [x] 多数据库标签页·锁定与可见性：`close_all_vaults` 锁定全部标签（工具栏锁/空闲锁/锁后操作），bridge/RPC 与全局热键仅服务 active 会话，`remembered` 随切换/关闭/锁定联动（锁屏 quick-reopen 保留）

### P2 — KDBX 属性完整度与数据交换

- [x] 条目属性·OverrideURL/QualityCheck 可写：`EntryFlags`/`update_entry_flags` 独立写入（字段 absent 保留；OverrideURL 空字符串清除；QualityCheck 显式布尔设置），`VaultEntry.overrideUrl` 暴露并由编辑器保存回调接线；保存重开 round-trip 测试
- [x] 条目属性·前景色可写：`update_entry_flags` 增加 `foregroundColor`（`#RRGGBB`，空清除/absent 保留），`VaultEntry.foregroundColor` 暴露，编辑对话框前景色选择；round-trip 测试
- [x] 分组属性可写：`update_group_meta` 支持 notes/tags/`enableSearching` + 分组右键「属性」对话框（`GroupMetaDialog`）；round-trip 测试（group Auto-Type 已有）
- [x] 跨客户端保真：`CustomData` 仍只读 + 新增 `foreign_attributes_survive_edits_and_flags_round_trip` 综合测试（外部客户端属性 + 字段编辑 + flags/分组 meta 编辑后保存/重开全保真）
- [x] 附件内存预览：`preview_attachment` 返回文本/图片 data URL（2 MiB 截断上限，其余为二进制提示），EntryDetail 附件预览对话框；不落盘
- [x] 附件临时打开：`open_attachment_temp`（受控随机临时目录 + token）+ `cleanup_attachment_temp`（丢弃/锁定清理，`close_all_vaults` 联动）+ 预览对话框两步确认后 `openPath`
- [x] 附件导入修改：`import_attachment_from_temp` 仅允许 token 注册的临时文件写回附件（64 MiB 上限，成功后清理），预览对话框「导入修改/丢弃修改」
- [x] 导入·Bitwarden JSON：后端严格解析（login/secure note、folder→分组、URI/自定义字段/TOTP），`read_text_file` 白名单加 `.json`，右键菜单「导入 Bitwarden」
- [x] 导入·1Password/LastPass：`parse_1pif`（`***Key:value` 块、续行、Folder/Field 映射，跳过 folder 定义）+ LastPass CSV 表头别名（`name`/`extra`/`grouping`）；1PUX（ZIP/加密导出）暂缓，待引入 zip 依赖后再做
- [x] 导出·HTML 应急表/打印：`export_emergency_sheet`（离线可打印 HTML、HTML 转义、含密码需勾选并带警告横幅），CSV 导出增加明文安全确认
- [x] 数据库维护·相似密码检查：`similar_passwords` 服务端分析（编辑距离 ≤ 2 聚类、回收站排除、2000 条上限、密码不外传），`SimilarPasswordsDialog` 报告并可定位条目
- [x] 数据库维护·历史清理：`clear_all_history` 全库清理（返回清理数量 + 刷新状态，当前条目保留，保存/重开验证），菜单入口带确认
- [x] 数据库维护·过期维护：`expired_entries` 集中清单（回收站排除、按过期时间排序、无敏感字段），`ExpiredEntriesDialog` 支持单条/全部「延期 30 天」与「删除」（复用 updateEntries/deleteEntries）
- [x] 数据库维护·损坏库诊断：`probe_vault` 头部分类（KDBX/KDB/未知 + 大小，不解密），`open_vault` 对非 KDBX 文件快速失败并给出明确提示
- [x] 数据库维护·尽力恢复提示：解析失败错误附带「导出 XML 后导入」兜底提示（密钥错误保持原提示）；失败写入保持会话完整已有测试覆盖（save-as 失败/保存中并发编辑）
- [x] 数据库维护·只读降级：连续 3 次保存失败后 `readOnly` 生效，保存/改密/存储重加密拒绝并提示「另存为」，另存为成功后复位；前端禁用保存按钮并显示「只读」标记（CRUD 仍为内存态，待保存路径恢复）

### P3 — 受约束自动化与可选安全增强

- [ ] ~~原生事件规则：提供有限的事件—条件—动作（打开/保存/锁定/定时、备份/同步/显示筛选）~~（用户决定不做，2026-08）
- [x] HIBP 泄露检查：`check_hibp` 严格 opt-in（隐私说明 + 显式开始），仅发送 SHA-1 前 5 位十六进制前缀（k-anonymity，mock 断言 wire 上只有前缀），密码/完整散列绝不出本机；`HibpCheckDialog` 按条目展示泄露次数并可定位
- [ ] ~~密钥文件生成/纸质备份与安全主密钥输入；Windows 用户账户密钥、YubiKey challenge-response 等~~（用户决定不做，2026-08）

## 后续候选(差距清单已清空)

- 账户绑定 (Hardware-bound, TPM) —— 用户暂缓
- 其余见 roadmap

## 浏览器集成(调研完成,提案见 `docs/browser-integration.md`)

- [~] Phase 1 — KeePassHttp 兼容协议:后台 loopback HTTP 服务(`associate`/`test-associate`/`get-logins`/`get-logins-count`/`set-login`/`generate-password`),AES-256-CBC 逐字段加密 + HMAC-SHA256;匹配复用 `VaultSession::autotype_match` 评分,skips 回收站;associate 密钥存会话内、锁定即销毁。已交付:`bridge.rs`(协议核心,19 测试含 NIST SP 800-38A/RFC 4231 向量)、`bridge_server.rs`(127.0.0.1:19455 服务 + 审批板 + 生命周期,7 测试)、`VaultSession` 桥接(5 测试)、设置「集成」面板(开关/状态/已授权客户端管理)+ 全局关联审批提示,共 132 后端测试通过 (🚧 真机浏览器扩展验证不可行,离线仅协议级测试)
- [~] Phase 2 — KeePassRPC 兼容(SRP-6a + WebSocket :12546 + AES JSON-RPC)——**已立项**:协议规格已从 Kee 4.0.7 官方扩展源码提取(`docs/browser-integration.md` Phase 2 细则)。实施顺序:rpc.rs 协议核心(SRP-6a 1024-bit 群 + 密钥认证 + AES-256-CBC/HMAC-SHA1 帧 + v1 方法/DTO)→ rpc_server.rs(WS loopback + 生命周期 + 会话密钥驻留)→ 前端(设置分区 + SRP 旁路密码对话框)→ 文档与验证。已交付后端:`rpc.rs`(协议核心,8 测试含 JS 镜像客户端 SRP 往返)、`rpc_server.rs`(127.0.0.1:12546 WSS + 握手状态机 + 旁路密码事件 `rpc-side-channel-request` + 原始 socket WS 冒烟测试,6 测试)、`VaultSession` RpcHost(2 测试)、`rpc` 配置开关 + `rpc_status` 命令,共 152 后端测试通过;已交付前端:`RpcSettingsPanel`(集成页 KeePassRPC 开关/状态,`rpc` 设置类型/归一化/`updateRpc`)、`RpcSideChannelPrompt`(旁路密码弹窗,倒计时/复制,挂载于 `+layout.svelte`,TCATO 覆盖层跳过) (🚧 真机 Kee 扩展验证不可行,离线仅协议级自洽测试 + 手动 UI 验证)
- [~] Phase 2b (post) — KeePassRPC 写路径(AddLogin/UpdateLogin)、BROWSER_SETTINGS_SYNC 评估——**已交付**:规格已从 KeePassRPC 1.12 插件源码(`KeePassRPCService.JSONRPC.cs`/`KeePassRPCService.cs` `MergeEntries`/`MergeInNewURLs`/`KeePassRPCService.DTOV1.cs` `setPwEntryFromEntry`)与 Kee 4.0.7 扩展源码提取(`docs/browser-integration.md` Phase 2b 细则)。已交付:`rpc.rs` 写路径 DTO(`RpcLoginWrite`/`RpcFieldWrite`,serde 精确还原 `uRLs`/`hTTPRealm`/`formFieldList`)、`merge_urls` 纯函数(5 模式 × `MergeInNewURLs` 提升语义)、`AddLogin`/`UpdateLogin` 调度(插件 `ArgumentException` 镜像校验 + `-32001`/`-32002` 错误)、特性 `KPRPC_FEATURE_ENTRY_URL_REPLACEMENT`(Kee 将发 urlMergeMode=5);`vault.rs` `RpcHost` 写路径(字段映射 `setPwEntryFromEntry`:首 FFTpassword→Password、FFTusername→UserName 后者胜、余字段→displayName 自定义字符串、URL 空格拼接;父分组解析(空/无效/回收站→根回退);更新走 `edit_tracking` 历史快照 = 插件 `CreateBackup`;回收站条目拒绝 = 防御纵深),共 162 后端测试通过(新增 10)。已记录偏差:附加字段存 KDBX 字符串而非插件私有 KPRPC JSON 配置;更新覆盖字段但不清除陈旧自定义字段。(BROWSER_SETTINGS_SYNC 结论:仅客户端特性标志,桌面 loopback 无设置同步通道,不实现不宣告;Kee Vault 事件会话与 v2 方法族超出桌面范围,不实现)
