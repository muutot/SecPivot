# KeyVault Roadmap

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
- [x] Favorite/pin entries with `--warning-color` accent (`toggle_favorite` + `KeyVault.Favorite` field)

## Stage 5 — Packaging & release

- [x] App icons (committed `src-tauri/icons/*`), bundle branding metadata (`publisher`/`copyright`/descriptions), custom NSIS template (`src-tauri/windows/installer.nsi`) — verified: `tauri build` produced `KeyVault_0.1.0_x64-setup.exe`
- [ ] GitHub Actions CI mirroring `npm run verify` (`.github/workflows/ci.yml` added; unverified in this environment — no `origin` remote to run it)
- [ ] Release workflow via version-release skill (`.opencode/skills/version-release` + `scripts/*.mjs` added; `release.mjs --dry-run` verified through step 3, tag/push unverified — no remote)

## Stage 6 — S3 remote vaults

- [x] S3 settings panel (`RemoteSettingsPanel.svelte`; endpoint/region/bucket/accessKey/secretKey/prefix/localDir/backupCount, plaintext keys per approved design)
- [x] Remote transport: `RemoteStorage` trait + `S3Storage` (rust-s3 0.34, path-style for MinIO) + `MemoryStorage` fake
- [x] `open_remote_vault` / `create_remote_vault` / `s3_list_objects` commands; `save()` uploads back to S3 for remote sessions
- [x] Save modes: `memory` (upload back only) / `local` (mirror to `Storage/remote/<localDir>` with timestamped `.bak` rotation, `backupCount`)
- [x] Welcome-screen remote browser: list S3 objects, open (password + keyfile) and create remote vaults
- [~] Live S3 end-to-end verification (no docker/minio/aws in this environment; transport now covered by a local mock HTTP S3 server test: ListObjectsV2 XML parsing, path-style signing, get/put, bounded-timeout behavior — `remote::tests::*`; real-provider behavior still unverified)

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
- [x] 便携版打包:`scripts/package-portable.ps1`(tauri build + 复制 exe + README,输出 `dist/KeyVault-<version>-portable.zip`,已验证 zip 内容)

## 后续候选(差距清单已清空)

- 账户绑定 (Hardware-bound, TPM) —— 用户暂缓
- 其余见 roadmap

## 浏览器集成(调研完成,提案见 `docs/browser-integration.md`)

- [~] Phase 1 — KeePassHttp 兼容协议:后台 loopback HTTP 服务(`associate`/`test-associate`/`get-logins`/`get-logins-count`/`set-login`/`generate-password`),AES-256-CBC 逐字段加密 + HMAC-SHA256;匹配复用 `VaultSession::autotype_match` 评分,skips 回收站;associate 密钥存会话内、锁定即销毁。已交付:`bridge.rs`(协议核心,19 测试含 NIST SP 800-38A/RFC 4231 向量)、`bridge_server.rs`(127.0.0.1:19455 服务 + 审批板 + 生命周期,7 测试)、`VaultSession` 桥接(5 测试)、设置「集成」面板(开关/状态/已授权客户端管理)+ 全局关联审批提示,共 132 后端测试通过 (🚧 真机浏览器扩展验证不可行,离线仅协议级测试)
- [~] Phase 2 — KeePassRPC 兼容(SRP-6a + WebSocket :12546 + AES JSON-RPC)——**已立项**:协议规格已从 Kee 4.0.7 官方扩展源码提取(`docs/browser-integration.md` Phase 2 细则)。实施顺序:rpc.rs 协议核心(SRP-6a 1024-bit 群 + 密钥认证 + AES-256-CBC/HMAC-SHA1 帧 + v1 方法/DTO)→ rpc_server.rs(WS loopback + 生命周期 + 会话密钥驻留)→ 前端(设置分区 + SRP 旁路密码对话框)→ 文档与验证。已交付后端:`rpc.rs`(协议核心,8 测试含 JS 镜像客户端 SRP 往返)、`rpc_server.rs`(127.0.0.1:12546 WSS + 握手状态机 + 旁路密码事件 `rpc-side-channel-request` + 原始 socket WS 冒烟测试,6 测试)、`VaultSession` RpcHost(2 测试)、`rpc` 配置开关 + `rpc_status` 命令,共 152 后端测试通过;已交付前端:`RpcSettingsPanel`(集成页 KeePassRPC 开关/状态,`rpc` 设置类型/归一化/`updateRpc`)、`RpcSideChannelPrompt`(旁路密码弹窗,倒计时/复制,挂载于 `+layout.svelte`,TCATO 覆盖层跳过) (🚧 真机 Kee 扩展验证不可行,离线仅协议级自洽测试 + 手动 UI 验证)
- [~] Phase 2b (post) — KeePassRPC 写路径(AddLogin/UpdateLogin)、BROWSER_SETTINGS_SYNC 评估——**已交付**:规格已从 KeePassRPC 1.12 插件源码(`KeePassRPCService.JSONRPC.cs`/`KeePassRPCService.cs` `MergeEntries`/`MergeInNewURLs`/`KeePassRPCService.DTOV1.cs` `setPwEntryFromEntry`)与 Kee 4.0.7 扩展源码提取(`docs/browser-integration.md` Phase 2b 细则)。已交付:`rpc.rs` 写路径 DTO(`RpcLoginWrite`/`RpcFieldWrite`,serde 精确还原 `uRLs`/`hTTPRealm`/`formFieldList`)、`merge_urls` 纯函数(5 模式 × `MergeInNewURLs` 提升语义)、`AddLogin`/`UpdateLogin` 调度(插件 `ArgumentException` 镜像校验 + `-32001`/`-32002` 错误)、特性 `KPRPC_FEATURE_ENTRY_URL_REPLACEMENT`(Kee 将发 urlMergeMode=5);`vault.rs` `RpcHost` 写路径(字段映射 `setPwEntryFromEntry`:首 FFTpassword→Password、FFTusername→UserName 后者胜、余字段→displayName 自定义字符串、URL 空格拼接;父分组解析(空/无效/回收站→根回退);更新走 `edit_tracking` 历史快照 = 插件 `CreateBackup`;回收站条目拒绝 = 防御纵深),共 162 后端测试通过(新增 10)。已记录偏差:附加字段存 KDBX 字符串而非插件私有 KPRPC JSON 配置;更新覆盖字段但不清除陈旧自定义字段。(BROWSER_SETTINGS_SYNC 结论:仅客户端特性标志,桌面 loopback 无设置同步通道,不实现不宣告;Kee Vault 事件会话与 v2 方法族超出桌面范围,不实现)
