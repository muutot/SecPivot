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

### Phase 2(已立项):KeePassRPC 兼容(SRP-6a + WebSocket + AES JSON-RPC)

服务对象:**Kee 官方扩展**(Chrome/Edge/Firefox),协议规格以 Kee 4.0.7 扩展源码(AMO `keefox-4.0.7.xpi`)为权威参考。架构沿用 Phase 1 的双模块切分:`rpc.rs`(纯协议,无 socket,可离线单测)+ `rpc_server.rs`(WS loopback 服务 + 生命周期)。

- **传输**:WebSocket `ws://127.0.0.1:12546`(tungstenite,thread-per-connection,与 `bridge_server.rs` 同模式)。扩展先以 `fetch` 探测 `http://127.0.0.1:12546/pingAvailabilityTest`,**期望 HTTP 404** 才发起 WS——普通 HTTP 请求一律回 404,仅 Upgrade 握手由 WS 处理。
- **信封**(所有消息):`{protocol: "setup"|"jsonrpc"|"error", srp?, key?, jsonrpc?, error?, version: int, features?, clientTypeId?, clientDisplayName?, clientDisplayDescription?}`。`version` 是 24 位整数(大端三字节 = major.minor.patch;Kee 2.0.0 → 131072)。
- **SRP-6a(KeePassRPC 变体)**:
  - 群参数:`N` = 512-bit 常量 `d4c7f8a2b32c11b8fba9581ec4ba4f1b04215642ef7355e37c0fc0443ef756ea2c6b8eeb755a1c723027663caa265ef785b8ff6a9b35227a52d86633dbdfca43`(hex),`g = 2`,`k = b7867f1299da8cc24ab93e08986ebc4d6a478ad0`(固定 SHA-1 常量)。
  - `H()` = SHA-256,作用于 **hex 字符串拼接**;`x = H(s‖p)`,`v = g^x mod N`,`u = H(A‖B)`,`B = (kv + g^b) mod N`,`S = (A·v^u)^b mod N`;客户端证明 `M = H(A‖B‖S)`(全大写 hex 拼接,输出小写 hex),服务端证明 `M2 = H(A大写‖M‖S大写)`;会话密钥 `K = H(S大写hex)` 小写 hex(= 32 字节,即 secretKey)。
  - 握手(服务端视角):`identifyToServer{I, A, securityLevel}` → 服务端生成旁路密码 p(桌面端显示,用户抄入扩展对话框)+ 盐 s + b,发 `identifyToClient{s, B, securityLevel, features}` → `proofToServer{M}` → 校验 M 后发 `proofToClient{M2}`。
- **密钥认证(1b)**:`key{username, securityLevel}` → 服务端随机 `sc`,发 `key{sc, securityLevel, features}` → `key{cc, cr}`(cr = SHA256("1"‖secret‖sc‖cc) 小写 hex)→ 校验后发 `key{sr}`(sr = SHA256("0"‖secret‖sc‖cc))。密钥按 username 存会话内(与 bridge_keys 同理,锁定即销毁;扩展侧自动回退 SRP 重新授权)。
- **JSON-RPC 帧**:AES-256-CBC + PKCS7(iv 16 随机字节),`hmac = base64(SHA1(SHA1(key字节)‖密文‖iv))`;帧 `{message, iv, hmac}`。每条消息独立 IV。
- **方法(v1 名,Kee 4.0.7 实际调用)**:`GetAllDatabases(null)` → `[DatabaseDTO]`;`FindLogins([urls, null, httpRealm, "LSTnoForms", false, uuid, dbFileName, freeText, username])` → `[EntryDTO + {db: DatabaseSummaryDTO}]`;`GetPasswordProfiles(null)` → `[名称]`;`GeneratePassword([profileName, url])` → 口令字符串;`AddLogin([EntryDTO, parentUUID, dbFileName])` / `UpdateLogin([EntryDTO, oldLoginUUID, urlMergeMode, dbFileName])` → EntryDTO + db(Phase 2b);`OpenAndFocusDatabase`/`ChangeDatabase`/`LaunchGroupEditor`/`LaunchLoginEditor` 返回空或错误。服务端可主动调用 `KPRPCListener`/`callBackToKeeFoxJS` 通知扩展。
- **DTO 字段**(camelCase):`DatabaseDTO{name, fileName, iconImageData, root: GroupDTO, active}`;`GroupDTO{title, uniqueID, iconImageData, path, childLightEntries: [EntrySummaryDTO], childGroups: [GroupDTO]}`;`EntrySummaryDTO{iconImageData, usernameValue, usernameName, title, uRLs, uniqueID}`;`EntryDTO{uRLs, neverAutoFill, alwaysAutoFill, neverAutoSubmit, alwaysAutoSubmit, iconImageData, parent: GroupSummaryDTO, matchAccuracy, hTTPRealm, uniqueID, title, formFieldList: [FieldDTO]}`;`FieldDTO{displayName, id, name, type: "FFTusername"|"FFTpassword"|"FFTtext"|"FFTradio"|"FFTcheckbox"|"FFTselect", value, page}`。用户名=首个 text 字段,口令=首个 password 字段。
- **特性标志**:服务端必须包含客户端必需的 `["KPRPC_FEATURE_VERSION_1_6", "KPRPC_GENERAL_CLIENTS", "KPRPC_SECURITY_FIX_20200729"]`(在 identifyToClient 与 key.sc 响应中随 `features` 数组下发);securityLevel 双向检查,服务端发 3。
- **错误码**:`AUTH_FAILED`/`AUTH_RESTART`/`AUTH_EXPIRED`/`AUTH_INVALID_PARAM`/`AUTH_MISSING_PARAM`/`AUTH_CLIENT_SECURITY_LEVEL_TOO_LOW`/`VERSION_CLIENT_TOO_LOW`/`UNRECOGNISED_PROTOCOL`/`INVALID_MESSAGE`,错误信封 `error{code, messageParams}`;认证失败后扩展自动清存根密钥并回退 SRP。
- **安全约束(与 Phase 1 一致)**:仅 loopback;锁定时直接错误信封、不自动解锁;旁路密码只在桌面端展示、不入日志;SRP 中间态(K、密钥)存会话内,锁定即销毁;超时 120 s。
- **依赖新增**:`num-bigint`(SRP 大数)、`tungstenite`(default-features 关闭,仅 handshake;std TcpStream 线程模式,不引入 async 运行时负担)。

## 对仓库的影响(已落实)

- 新后端模块 `bridge.rs`(协议核心,无 socket,可单测)+ `bridge_server.rs`(loopback 服务 + 生命周期 + 审批板);`lib.rs` 挂 managed state 与命令(`bridge_status`/`bridge_clients`/`bridge_remove_client`/`bridge_approve`),`set_config` 按 `bridge.enabled` 启停。
- 设置面板新增「集成」分区(`BridgeSettingsPanel.svelte`:开关 + 服务状态 + 已授权客户端列表/移除)。
- 全局关联审批提示 `BridgeApprovalPrompt.svelte`(挂 `+layout.svelte`,TCATO 窗口除外)。
- `security-model.md` / `data-contracts.md` / `project-structure.md` 已增补对应章节。
- Phase 2 计划:`rpc.rs` + `rpc_server.rs`、设置面板 KeePassRPC 分区、SRP 旁路密码对话框(桌面端展示随机密码,用户抄入 Kee 扩展)。

## 验证边界

- 可离线验证(已交付):响应信封字段、AES 往返、HMAC/Verifier、URL 匹配、associate 审批流、HTTP 帧解析与超限(共 131 后端测试通过)。
- 无法离线验证:浏览器扩展真实上屏自动填充、真实扩展握手 —— 标 `~` 保留到有运行环境。
