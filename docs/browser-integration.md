# 浏览器集成调研与设计提案

状态:`Phase 1 已交付`(KeePassHttp 兼容协议实现完毕,离线协议级测试通过;真机浏览器扩展验证不可行,保留 `~` 证据缺口)。立项与交付证据见 `TODO.md`。本文是决策依据与实现说明。

## 目标

让 SecPivot 作为 KDBX 客户端,能被 **KeePass 生态**的浏览器扩展当作凭据来源,复用现有 `VaultSession`(解锁校验、条目匹配、回收站跳过、密码不落地 IPC 的安全模型),不外发主密钥。

## 现状(已实现)

- 后台 loopback HTTP 服务:`bridge_server.rs` 监听 `127.0.0.1:19455`,每连接一次 JSON POST。
- 协议核心:`bridge.rs` 实现 `associate`/`test-associate`/`get-logins`/`get-logins-count`/`set-login`/`generate-password`;AES-256-CBC 逐字段加密 + PKCS7;请求 `Verifier` 与响应 `Hmac`(HMAC-SHA256)按 KeePassHttp 语义校验。`generate-password` 返回 20 位随机口令(大小写/数字/符号各至少一位,与应用默认生成器一致)。
- 匹配复用 `VaultSession::autotype_match` 同款 URL 评分逻辑(回收站跳过),`db_hash` = SHA1(根分组UUID ‖ 回收站分组UUID)。
- associate 密钥存会话内(`bridge_keys`),锁定即销毁;首次关联由桌面端审批(设置「集成」面板 + 全局审批提示组件)。

## 生态:候选协议

| 协议                                     | 服务端                                                         | 传输                                        | 加密/认证                                          | 典型客户端                                                        | 与 SecPivot 兼容方式                                                                                                                                        |
| ---------------------------------------- | -------------------------------------------------------------- | ------------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **KeePassHttp**(官方推荐之一)            | KeePass 插件监听 `localhost:19455`                             | HTTP POST JSON,无扩展消息框架               | AES-256-CBC (PKCS7),每请求 Nonce 作 IV+HMAC-SHA256 | ChromeIPass/PassIFox、KeePassHelper、KeePassXC(legacy mode)、Dash | 由一个 Tauri 后台常驻 loopback HTTP 服务实现同样 API(`associate`/`test-associate`/`get-logins`/`set-login`/`get-logins-count`/`generate-password`)          |
| **KeePassRPC**(Kee 官方)                 | KeePass 插件监听 WebSocket `localhost:12546`                   | WebSocket,JSON-RPC                          | SRP(首连)+ 每消息 AES 加密 + HMAC                  | Kee(Firefox/Chrome 官方扩展)                                      | 需要实现 SRP + AES-JSON-RPC,浏览器扩展 Origin 白名单(`moz-extension://`、`chrome-extension://`)                                                             |
| **KeePassXC-Browser / native messaging** | `keepassxc-proxy`(独立进程)+ 主进程 `QLocalSocket`/Unix socket | 浏览器原生消息(stdin/stdout 4 字节长度前缀) | TweetNaCl box (XSalsa20-Poly1305 + curve25519)     | keepassxc-browser 扩展(Ke数据库 XC 专用)                          | 需注册原生消息宿主 manifest(Windows 为注册表)+ 实现协议,并能与 **keepassxc-proxy 同名冲突**——扩展按固定宿主名 `keepassxc-proxy` 调用,除非替换它否则无法共存 |

**结论一**:KeePassXC-Browser 扩展绑定 `keepassxc-proxy` 固定宿主名与 XC 专用协议,不与经典 KeePass 生态兼容;SecPivot 若接它需要伪装成该宿主名,与真实 KeePassXC 冲突,不推荐。

**结论二**:经典 KeePass 官方浏览器集成实际上是两条路径——KeePassHttp(长势最成熟、客户端最多)与 KeePassRPC(官方 Kee 扩展)。二者面向的都不是原生消息而是 loopback socket 监听,与 Tauri 后台架构天然融合。

## 推荐路线(分两步立项)

### Phase 1:KeePassHttp 兼容协议 → 「初代」凭据提供

理由:协议最简单(纯 HTTP 单向、无握手/心跳),浏览器端扩展成熟多 (Chrome/Edge/Firefox),与现有 `VaultSession` 对接成本最低,可先于复杂协议验证 Tauri 后台监听架构。

技术选型(已按此实现):

- 服务:标准库 `std::net` 单线程 accept + 每连接独立线程(无 async 运行时负担),loopback 绑定 `127.0.0.1:19455`;请求头 16 KiB / 请求体 1 MiB 上限,读超时 10 s。
- 密钥杂凑:`aes` + `cbc`(AES-256-CBC, PKCS7)+ `hmac`/`sha2`(HMAC-SHA256),`getrandom` 生成 nonce/客户端 id。
- 字段按 KeePassHttp 规范逐字段加密(`Url`/`SubmitUrl`/`Login`/`Password`/`Uuid`/`Realm`/`Names[]`),每响应独立 `Nonce`(作 IV)。
- `associate` 密钥存储:SecPivot 侧密钥落在 **会话内**(与 keyfile 同理),不落 `config.json`,锁定即销毁——`security-model.md` 已增补不变式。
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

### Phase 2b(已立项):KeePassRPC 写路径(AddLogin/UpdateLogin)

服务对象与权威来源不变(Kee 4.0.7 扩展源码 + KeePassRPC 1.12 插件源码)。服务端语义以 KeePassRPC `KeePassRPCService.JSONRPC.cs`(`AddLogin`/`UpdateLogin`)、`KeePassRPCService.cs`(`MergeEntries`/`MergeInNewURLs`)与 `KeePassRPCService.DTOV1.cs`(`setPwEntryFromEntry`)为基准。

- **AddLogin**(v1 名,扩展实际调用):`[EntryDTO login, String parentUUID, String dbFileName]` → `EntryDTO + db`(DatabaseSummaryDTO)。
  - 服务端新建条目:条目 UUID 由服务端生成(Kee 4.0.7 的 `Entry.toKPRPCEntryDTO` 不含 `uniqueID`);字段映射按插件 `setPwEntryFromEntry`:
    - 第一个 `FFTpassword` 字段 → `Password`;所有 `FFTusername` 字段 → `UserName`;
    - 其余字段 → 按 `displayName`(回退 `name`)存为附加字段。插件写入「KPRPC JSON」自定义配置;SecPivot 决策:写入 KDBX 自定义字符串字段(与 Phase 1 `set-login` 一致,不引入 KPRPC JSON 格式);
    - `uRLs[0]` → `URL`,`uRLs[1..]` → 备用 URL;`title` → `Title`;`hTTPRealm` → 条目配置。
  - `parentUUID` 空 → 根分组;非空按 UUID 查找分组,未找到回退根分组。`dbFileName` 空 → 当前库;SecPivot 单库会话:路径不匹配回退当前库(插件 `SelectDatabase` 同语义)。
  - 成功返回新条目的 `EntryDTO`(含 `db` 摘要);库锁定 → 错误信封。
- **UpdateLogin**(v1 名):`[EntryDTO login, String oldLoginUUID, int urlMergeMode, String dbFileName]` → `EntryDTO + db`。
  - 前置校验:`login` 缺失 / `oldLoginUUID` 空 / `dbFileName` 空 / `oldLoginUUID` 无法解析到条目 → JSON-RPC error;目标条目位于回收站 → 拒绝。
  - 合并语义(插件 `MergeEntries`):Title/UserName/Password/附加字段/HTTPRealm/图标从 DTO 覆盖;更新前创建历史快照(`CreateBackup`,对应 SecPivot 条目历史机制);URL 按 `urlMergeMode` 合并(`MergeInNewURLs` 语义:源 URL 逆序插入、去重、原备用 URL 提升为主 URL):
    - `1` = 合并源 URL(保留旧 URL,新 URL 置顶,旧 URL 仍可匹配)
    - `2` = 删除旧主 URL 后合并源 URL
    - `3` = 保留旧 URL,仅追加源中不存在的 URL
    - `4` = URL 不变
    - `5` = 删除全部旧 URL,整体替换为源 URL 列表
  - Kee 4.0.7 发送 `urlMergeMode`:`features` 含 `KPRPC_FEATURE_ENTRY_URL_REPLACEMENT` → `5`,否则 → `2`。决策:服务端提供该特性(语义最简、可预期),同时实现全部 5 种模式(纯函数,可离线单测)。
- **BROWSER_SETTINGS_SYNC**:仅 Kee 扩展 `FeatureFlags.offered` 中宣告的客户端能力(浏览器多端设置同步,面向 Kee Vault 网页端托管会话);Kee 4.0.7 扩展无对应消息处理器,桌面 loopback 服务器无设置同步通道 —— 不实现、不宣告。Kee Vault 事件会话(`AckInit`/`sessionId` 浏览器托管传输)超出桌面客户端范围,不实现。
- **v2 方法族**(`AddEntry`/`UpdateEntry`/`AllDatabases`/`AllDatabasesAndIcons`/DTO_V2)依赖 `KPRPC_FEATURE_DTO_V2` 且面向 Kee Vault,桌面 Kee 4.0.7 不调用 —— 不实现。
- **测试策略**:`rpc.rs` 增加 URL 合并模式单测(5 模式 × 多 URL 场景)+ `AddLogin`/`UpdateLogin` 调度单测;`vault.rs` 增加写路径测试(新建于根/指定分组、字段映射、URL 合并、历史快照、回收站拒绝、锁定拒绝)。真机 Kee 扩展 E2E 仍不可行(离线),保留 `~` 证据缺口。
- **Phase 2b 交付状态(已交付)**:`rpc.rs` 新增写路径 DTO(`RpcLoginWrite`/`RpcFieldWrite`,serde 精确还原 `uRLs`/`hTTPRealm`/`formFieldList` 键名)、`merge_urls` 纯函数(5 模式 + `MergeInNewURLs` 源主 URL 提升语义)、`handle_jsonrpc` 的 `AddLogin`/`UpdateLogin` 分支(镜像插件的 `ArgumentException` 预校验与 `-32001`/`-32002` 错误码)与特性 `KPRPC_FEATURE_ENTRY_URL_REPLACEMENT`(Kee 将发 `urlMergeMode=5`);`vault.rs` 的 `VaultSession` RpcHost 写路径:`setPwEntryFromEntry` 字段映射(首 `FFTpassword`→Password 受保护、`FFTusername`→UserName 后者胜、余字段→`displayName` 回退 `name` 的自定义字符串、URL 空格拼接与读路径一致)、父分组解析(空/无效/回收站内 → 根回退)、更新走 `edit_tracking` 历史快照(插件 `CreateBackup` 等价)、回收站条目拒绝(插件允许,但 Kee 读路径永不可见,属防御纵深)。**已记录偏差**:附加字段存 KDBX 字符串而非插件私有「KPRPC JSON」配置;更新覆盖字段但不删除陈旧自定义字段(避免误删应用管理字段)。

## 对仓库的影响(已落实)

- 新后端模块 `bridge.rs`(协议核心,无 socket,可单测)+ `bridge_server.rs`(loopback 服务 + 生命周期 + 审批板);`lib.rs` 挂 managed state 与命令(`bridge_status`/`bridge_clients`/`bridge_remove_client`/`bridge_approve`),`set_config` 按 `bridge.enabled` 启停。
- 设置面板新增「集成」分区(`BridgeSettingsPanel.svelte`:开关 + 服务状态 + 已授权客户端列表/移除)。
- 全局关联审批提示 `BridgeApprovalPrompt.svelte`(挂 `+layout.svelte`,TCATO 窗口除外)。
- `security-model.md` / `data-contracts.md` / `project-structure.md` 已增补对应章节。
- Phase 2 计划:`rpc.rs` + `rpc_server.rs`、设置面板 KeePassRPC 分区、SRP 旁路密码对话框(桌面端展示随机密码,用户抄入 Kee 扩展)。

## 验证边界

- 可离线验证(已交付):响应信封字段、AES 往返、HMAC/Verifier、URL 匹配、associate 审批流、HTTP 帧解析与超限(共 131 后端测试通过)。
- 无法离线验证:浏览器扩展真实上屏自动填充、真实扩展握手 —— 标 `~` 保留到有运行环境。
