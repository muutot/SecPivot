# SecPivot 未完成项 / 待决策项

> 本文件只记录**尚未完成**或**尚未决策**的工作。已交付项不在此列。
> 状态：`[ ]` 待做 · `[~]` 部分完成，剩余部分明确 · `[?]` 待 Muu 决策。
> KeePass 官方设置全量对照见 `docs/KEEPASS-CONFIG-INVENTORY.md` 与 `docs/KEEPASS-OPTIONS-GAP.md`；本文件只收尚未关闭的条目。

## 一、明确待实现

- [ ] 主密码过期提醒/强制（`MasterKeyExpiryRec` / `MasterKeyExpiryForce`）：前置 `MasterKeyChanged` 建库/改密钥写入与 `MasterKeyChangeForceOnce` 未设置/False/True 保真已修复；成功改密钥在同次权威保存中消费 True，失败/只读保留请求，真实临时 KDBX/离线 remote、仅密钥文件与并发回归通过；到期规则、提醒/强制执行仍待实现
- [ ] 自动搜索密钥文件（`SearchKeyFiles` / `SearchKeyFilesOnRemovableMedia`）
- [ ] 本地保存备份：KeePass backup on save / count / template；当前仅 remote profiles 有 `backupCount`/`backupTemplate`，本地保存未实现
- [ ] 自动重载已变更文件（`AutoReloadChangedFiles`）
- [ ] 配置日志开关（`AceLogging.Enabled`）
- [ ] 打印系统（KeePass 应急表打印约 20 项：Layout/Include*/Color*/Fonts/Sort）——当前只有离线 HTML 应急表导出

## 二、部分完成（剩余边界明确）

- [~] 主密码重试次数（`AceSecurity.MasterKeyTries`）：默认 3 次、正 int32 配置及旧配置重载已接入；本地/最近/远程打开与锁屏手动/Hello 共用每次提示预算，耗尽后清空字段并结束本次提示，新打开意图重置。提交失败（含 I/O）计次，Hello 查询失败不计次；重复/过期提交有守卫。实际提交函数与 ConfigStore 回归已覆盖，真实 WebView/provider 仍待验；这是提示流程限制，不是永久锁库或离线防破解

- [~] Android TLS 信任链修复（SP-S125）：`webpki-root-certs` 1.0.10（移除 emSign ECC G3）与 `rustls-webpki` 0.103.16（拒绝不信任 SPKI）及最低安全版本约束已接入，其他锁依赖边保留。实际 Android builder 源码在 host 上的 async/blocking TLS、未知 CA 拒绝、八项不信任 SPKI 与根包回归通过；Android 目标编译仍缺 NDK clang，APK/真机/provider 待验

- [~] 弱 KDF 警告（`AceSecurity.KeyTransformWeakWarning`）：默认开启的配置、改 KDF/主密钥提醒已接入；打开旧库时展示当前/推荐成本，同意后按同一算法升级并保留未保存状态，普通保存才写入文件/远程。原生寿命/参数校验、只读、旧保存排队、多标签及临时 KDBX/离线 remote 与实际前端接线回归通过。真实 WebView/provider 与官方客户端互操作仍待验证

- [~] 关闭库时自动保存（`AceCloseDb.AutoSave`）：默认关闭的原生保存/放弃/取消流程已接入关闭标签页、锁定、关窗/托盘退出；临时 KDBX、离线 remote、并发会话及配置/前端回归通过。真实 WebView 关窗/托盘原生三按钮及 Android provider/后台运行仍待验证

- [~] 复制后锁定归属（SP-S11）：renderer 侧拓扑与视图代次有回归；原生多窗口 / OS ownership 尚未验证
- [~] 剪贴板并发生命周期（SP-11）：同 renderer 写入/条件清理/锁定清空已串行；跨窗口、原生二进制复制顺序、外部相同内容复制的 OS ownership 待验证
- [~] 条目历史版本：其他修改入口与跨客户端完整语义仍未覆盖；同秒内容冲突与跨客户端边界待复核
- [~] 官方同步条目级合并：其余同秒内容冲突与跨客户端边界待复核，不按标题字段回归认定完整历史/附件保真
- [~] 多数据库标签页拓扑：核心隔离与视图代次已实现并测试；拓扑操作与待处理切换交错、部分前端状态/清理路径仍需复核（子项已全部 `[x]`，最终整体复核未关闭）
- [~] 逐网址匹配策略：分组/附件/历史完整 XML 迁移、1Password 来源规则编码、Bitwarden 全部模式映射、真实 WebView/Android provider 未完成
- [~] 导入 Bitwarden JSON：动态链接、完整逐 URI 匹配、组织 collections、原生历史/附件、真实客户端导出待实现/验证
- [~] 导入 1Password/LastPass：真实 1Password 导出运行验证缺失，完整兼容不标完成
- [~] 导入 1PUX：额外网址逐网址范围、复杂类型与源历史仍是 protected JSON；真实导出、原生选择器、Android provider / 新 APK 未验证
- [~] 导出 KeePass XML：分组/附件/历史与完整原生互操作继续验证，明文确认对话框不变
- [~] 原生 XML 导入完整保真/原生互操作（SP-S30 之后）
- [~] CI：工作流已覆盖 `npm run verify`；当前 HEAD 尚无远端运行证据
- [~] Release workflow：工作流已修正 NDK 工具、四 ABI universal APK、签名与产物硬门禁；当前 HEAD 远端运行及 APK 资产证据待补
- [~] KeePassHttp（Phase 1）：协议级测试通过；真机浏览器扩展验证不可行
- [~] KeePassRPC（Phase 2）：协议级自洽 + 手动 UI 验证；真机 Kee 扩展验证不可行
- [~] KeePassRPC 写路径（Phase 2b）：已交付；附加字段存 KDBX 字符串而非插件私有 KPRPC JSON 配置，更新覆盖字段但不清除陈旧自定义字段
- [~] 远程密码空白保真（SP-S75）：真实 provider、移动密钥存储、历史已被 trim 值的恢复不在此完成范围
- [~] WebDAV namespace/propstat/完整 XML 与真实 provider（SP-S71～S74 各自剩余边界）
- [~] 原生剪贴板写入（SP-S113）：未调用共享 OS 剪贴板，实际复制/客户端、外部相同内容代次、跨 renderer 顺序与手机 provider 未验证
- [~] 原生另存为（SP-S114/S115）：实际文件写入、手机选择器/provider、跨客户端行为不由合成测试关闭
- [~] 持久化状态回复排序（SP-S116）：真实 IPC 时序、手机 APK/选择器/provider 和其他平台不属于完成证明
- [~] 内部条目交换（SP-S87～S110 各自边界）：OS 剪贴板、KeePass 容器/保护方言/时间戳/历史、真实跨客户端交换、手机 provider、Android 新 APK 未验证
- [~] 原生 KDBX 日期异常处理（SP-S106）：损坏加密库实际 IPC/真机/provider 及完整日期互操作待验证
- [~] 临时主密码清理（SP-S107～S111）：框架/renderer/全进程内存擦除、keyfile read 内部缓冲、手机 provider 独立待审
- [~] 大字号窄屏选项显示（SP-S112）：Android 选择器、provider、任意配色对比度不据 browser 证据关闭
- [~] 空闲锁（SP-S68）：Android 原生 onPause/onStop 后台锁定、系统挂起、renderer 无法运行期间的后端定时约束未完成
- [~] 配置诊断（SP-S69）：移动端明文凭据存储、原生日志、Keystore/后台锁定验证未覆盖
- [~] 欢迎页仅密钥文件（SP-S67）：Android 选择器/持久授权、新 APK/真机 provider、真实云端流程未标记完成
- [~] 原生 KeePass 互操作：真实 KeePass 客户端打开/保存本库、真实扩展与原生 WebView 尚未验证

## 三、待 Muu 决策

- [?] 推送重写后的本地 git 历史到 origin/main：当前本地已重写（audit 提交清理，272→161 + 后续提交），未推；`origin/main` 仍指旧历史，普通 pull 会分叉。备份 `B:\Software\AI\DSH\backups\secpivot-pre-rewrite-20261125.bundle`。此前决定"暂不推，只保留本地"
- [?] Policy 强制策略 / UIFlags / KeyCreationFlags / KeyPromptFlags：个人库强加会锁死用户；建议只做只读展示或完全不做，不造"改了也不生效"的 UI 壳
- [?] Secure desktop 主密钥输入 / ProtectProcessWithDacl：Windows 安全桌面成本高、官方自己不推荐；是否接受平台风险与维护成本
- [?] 通用插件系统 / 触发器系统：产品约束暂不接入；优秀插件能力仅在其安全边界、维护成本和产品价值明确时作为原生功能实现
- [?] TAN 视图、Windows 收藏夹导出、打印系统：架构不适用或价值不明确
- [?] 账户绑定（Hardware-bound, TPM）：此前用户暂缓
- [?] 原始 KeePass 原生事件规则（打开/保存/锁定/定时 → 备份/同步/筛选）：2026-08 已决定不做，若产品方向变化需重新确认
- [?] 密钥文件生成/纸质备份、Windows 用户账户密钥、YubiKey challenge-response：2026-08 已决定不做，若需支持要重新评估

## 四、明确不实现（仅存档，勿当待办）

以下为已决策的排除项，仅在此留痕，避免被误读为遗漏：

- 管理强制策略的实际 enforcement（个人库不适用）
- 打印系统、TAN 视图、插件缓存内部状态、Rect/DPI 等纯内部状态缓存
- `{CMD:...}` 占位符执行与确认、通用宏系统（安全与维护成本）
- Auto-Type 按键间隔（enigo 0.6 仅 Linux X11 支持，Windows 做即假开关）
- MinimizeAfterAutoType（当前最小化是焦点交接机制，改为可选有打错目标窗口风险）
- 全局空闲锁的 WebView 内检测（不可靠，需要系统级事件）
