# 浏览器集成调研与设计提案

状态:`proposal`(调研完成,未实现)。立项见 `TODO.md`。本文是决策依据,不是交付证据。

## 目标

让 KeyVault 作为 KDBX 客户端,能被 **KeePass 生态**的浏览器扩展当作凭据来源,复用现有 `VaultSession`(解锁校验、条目匹配、回收站跳过、密码不落地 IPC 的安全模型),不外发主密钥。

## 现状

- KeyVault 前端不暴露任何后台服务;Rust 后端没有监听任何端口,也没有浏览器集成协议。
- 现有能力:自动填充(`auto_type` + `{REF:...}`)、TCATO、全局热键(见 `security-model.md`)——这些是**桌面到前台窗口**的通道,浏览器扩展完全不经过它们。
- 因此当前**不兼容**任何 KeePass 浏览器扩展。

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

技术选型:

- 服务:`tokio` + `axum`(项目已有一个自带 `Runtime` 的 tokio 依赖模式,见 `remote.rs`),loopback 绑定 `127.0.0.1:19455`。
- 密钥杂凑:`aes` + `block-modes`(AES-CBC, PKCS7)+ `hmac`/`sha2`(HMAC-SHA256)。
- 字段按 KeePassHttp 规范逐字段加密(`Url`/`SubmitUrl`/`Login`/`Password`/`Uuid`/`Realm`/`Names[]`),每响应独立 `Nonce`。
- `associate` 密钥存储:沿用 KeePassHttp「AES 密钥存于库根分组 `KeePassHttp Settings`,客户端记住 `Id`」;KeyVault 侧密钥落在 **会话内**(与 keyfile 同理),不落 `config.json`,锁定即销毁——需在 `security-model.md` 增补一条不变式。
- 匹配复用 `VaultSession::autotype_match` 同款 URL 评分逻辑(回收站跳过),避免两套匹配规则。

安全约束(必须):

- 仅监听 loopback;绑定非 loopback 一律拒绝。
- 未解锁库时 `get-logins` 返回错误,不触发自动解锁(或仅触发前端解锁提示,由用户确认)。
- HTTP 本体是明文 JSON,但敏感字段密文 + HMAC;DP012 内机器不会经不住中间人。`PITFALLS.md` 增补:凡请求带密文的错误日志不得打印明文。
- 首次 `associate` 需要用户在桌面端手动批准(复用设置面板,"允许此浏览器客户端")。

### Phase 2(可选、后置):KeePassRPC 或 KeePassXC 协议

- KeePassRPC 需要 SRP(rust `srp` crate)+ WebSocket(`tokio-tungstenite`)+ AES JSON-RPC,单次 SRP 实现与测试成本明显高于 Phase 1;仅在 Phase 1 架构验证后再立项。
- KeePassXC-Browser 需在 Phase 完成的「后台 loopback 服务 + 密钥杂凑」基础上加原生宿主(manifest + Windows 注册表桥)。天然冲突问题需先解决以确认是否接入。

## 对仓库的影响(shorthand)

- 新后端模块 `browser_http.rs`(或 `bridge.rs`)装载协议实现,`lib.rs` 挂常驻任务。
- 设置面板新增「浏览器集成」小节(开关 + 查看已授权客户端 + 移除授权)。
- `security-model.md` / `data-contracts.md` / `project-structure.md` 增补对应章节。
- 测试:协议级单元测试(associate/verifier/get-logins 匹配)下发内存 Session;真实扩展可用性需真机浏览器验证,环境无法完成则标 `~`。

## 验证边界

- 可离线验证:响应信封字段、AES 往返、HMAC、URL 匹配(借用 `autotype_match` 单测)。
- 无法离线验证:浏览器扩展真实上屏自动填充、原生注册表宿主、多实例 shortcut —— 标 `~` 保留到有运行环境。
