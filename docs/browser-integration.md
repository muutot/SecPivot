# 浏览器集成调研与设计提案

状态:`Phase 1 已交付`(KeePassHttp 兼容协议实现完毕,离线协议级测试通过;真机浏览器扩展验证不可行,保留 `~` 证据缺口)。立项与交付证据见 `TODO.md`。本文是决策依据与实现说明。

## 目标

让 KeyVault 作为 KDBX 客户端,能被 **KeePass 生态**的浏览器扩展当作凭据来源,复用现有 `VaultSession`(解锁校验、条目匹配、回收站跳过、密码不落地 IPC 的安全模型),不外发主密钥。

## 现状(已实现)

- 后台 loopback HTTP 服务:`bridge_server.rs` 监听 `127.0.0.1:19455`,每连接一次 JSON POST。
- 协议核心:`bridge.rs` 实现 `associate`/`test-associate`/`get-logins`/`get-logins-count`/`set-login`/`generate-password`;AES-256-CBC 逐字段加密 + PKCS7;请求 `Verifier` 与响应 `Hmac`(HMAC-SHA256)按 KeePassHttp 语义校验。`generate-password` 返回 20 位随机口令(大小写/数字/符号各至少一位,与应用默认生成器一致)。
- 匹配复用 `VaultSession::autotype_match` 同款 URL 评分逻辑(回收站跳过),`db_hash` = SHA1(根分组UUID ‖ 回收站分组UUID)。
- associate 密钥存会话内(`bridge_keys`),锁定即销毁;首次关联由桌面端审批(设置「集成」面板 + 全局审批提示组件)。

## 生态:候选协议

| 协议                                     | 服务端                                                         | 传输                                        | 加密/认证                                          | 典型客户端                                                        | 与 KeyVault 兼容方式                                                                                                                                        |
| ---------------------------------------- | -------------------------------------------------------------- | ------------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **KeePassHttp**(官方推荐之一)            | KeePass 插件监听 `localhost:19455`                             | HTTP POST JSON,无扩展消息框架               | AES-256-CBC (PKCS7),每请求 Nonce 作 IV+HMAC-SHA256 | ChromeIPass/PassIFox、KeePassHelper、KeePassXC(legacy mode)、Dash | 由一个 Tauri 后台常驻 loopback HTTP 服务实现同样 API(`associate`/`test-associate`/`get-logins`/`set-login`/`get-logins-count`/`generate-password`)          |
| **KeePassRPC**(Kee 官方)                 | KeePass 插件监听 WebSocket `localhost:12546`                   | WebSocket,JSON-RPC                          | SRP(首连)+ 每消息 AES 加密 + HMAC                  | Kee(Firefox/Chrome 官方扩展)                                      | 需要实现 SRP + AES-JSON-RPC,浏览器扩展 Origin 白名单(`moz-extension://`、`chrome-extension://`)                                                             |
| **KeePassXC-Browser / native messaging** | `keepassxc-proxy`(独立进程)+ 主进程 `QLocalSocket`/Unix socket | 浏览器原生消息(stdin/stdout 4 字节长度前缀) | TweetNaCl box (XSalsa20-Poly1305 + curve25519)     | keepassxc-browser 扩展(Ke数据库 XC 专用)                          | 需注册原生消息宿主 manifest(Windows 为注册表)+ 实现协议,并能与 **keepassxc-proxy 同名冲突**——扩展按固定宿主名 `keepassxc-proxy` 调用,除非替换它否则无法共存 |

**结论一**:KeePassXC-Browser 扩展绑定 `keepassxc-proxy` 固定宿主名与 XC 专用协议,不与经典 KeePass 生态兼容;KeyVault 若接它需要伪装成该宿主名,与真实 KeePassXC 冲突,不推荐。

**结论二**:经典 KeePass 官方浏览器集成实际上是两条路径——KeePassHttp(长势最成熟、客户端最多)与 KeePassRPC(官方 Kee 扩展)。二者面向的都不是原生消息而是 loopback socket 监听,与 Tauri 后台架构天然融合。

## 推荐路线(分两步立项)

### Phase 1:KeePassHttp 兼容协议 → 「初代」凭据提供

理由:协议最简单(纯 HTTP 单向、无握手/心跳),浏览器端扩展成熟多 (Chrome/Edge/Firefox),与现有 `VaultSession` 对接成本最低,可先于复杂协议验证 Tauri 后台监听架构。

技术选型(已按此实现):

- 服务:标准库 `std::net` 单线程 accept + 每连接独立线程(无 async 运行时负担),loopback 绑定 `127.0.0.1:19455`;请求头 16 KiB / 请求体 1 MiB 上限,读超时 10 s。
- 密钥杂凑:`aes` + `cbc`(AES-256-CBC, PKCS7)+ `hmac`/`sha2`(HMAC-SHA256),`getrandom` 生成 nonce/客户端 id。
- 字段按 KeePassHttp 规范逐字段加密(`Url`/`SubmitUrl`/`Login`/`Password`/`Uuid`/`Realm`/`Names[]`),每响应独立 `Nonce`(作 IV)。
- `associate` 密钥存储:KeyVault 侧密钥落在 **会话内**(与 keyfile 同理),不落 `config.json`,锁定即销毁——`security-model.md` 已增补不变式。
- 匹配复用 `VaultSession::autotype_match` 同款 URL 评分逻辑(回收站跳过),避免两套匹配规则。

安全约束(已落实):

- 仅监听 loopback;绑定 `127.0.0.1` 固定地址。
- 未解锁库时请求返回错误信封,不触发自动解锁。
- HTTP 本体是明文 JSON,但敏感字段密文 + HMAC;错误日志不打印解密明文。
- 首次 `associate` 需要用户在桌面端手动批准:后端发 `bridge-associate-request` 事件(载荷 `{ token, id }`,不含密钥),前端审批提示组件调用 `bridge_approve`;120 s 未答复自动拒绝。

### Phase 2(可选、后置):KeePassRPC 或 KeePassXC 协议

- KeePassRPC 需要 SRP(rust `srp` crate)+ WebSocket(`tokio-tungstenite`)+ AES JSON-RPC,单次 SRP 实现与测试成本明显高于 Phase 1;仅在 Phase 1 架构验证后再立项。
- KeePassXC-Browser 需在 Phase 完成的「后台 loopback 服务 + 密钥杂凑」基础上加原生宿主(manifest + Windows 注册表桥)。天然冲突问题需先解决以确认是否接入。

## 对仓库的影响(已落实)

- 新后端模块 `bridge.rs`(协议核心,无 socket,可单测)+ `bridge_server.rs`(loopback 服务 + 生命周期 + 审批板);`lib.rs` 挂 managed state 与命令(`bridge_status`/`bridge_clients`/`bridge_remove_client`/`bridge_approve`),`set_config` 按 `bridge.enabled` 启停。
- 设置面板新增「集成」分区(`BridgeSettingsPanel.svelte`:开关 + 服务状态 + 已授权客户端列表/移除)。
- 全局关联审批提示 `BridgeApprovalPrompt.svelte`(挂 `+layout.svelte`,TCATO 窗口除外)。
- `security-model.md` / `data-contracts.md` / `project-structure.md` 已增补对应章节。

## 验证边界

- 可离线验证(已交付):响应信封字段、AES 往返、HMAC/Verifier、URL 匹配、associate 审批流、HTTP 帧解析与超限(共 131 后端测试通过)。
- 无法离线验证:浏览器扩展真实上屏自动填充、真实扩展握手 —— 标 `~` 保留到有运行环境。
