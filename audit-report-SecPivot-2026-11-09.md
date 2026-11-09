# Fuck My Shit Mountain Audit Report

**Project:** SecPivot 1.6.5
**Audit mode:** full
**Date:** 2026-11-09
**Reviewer:** Codex
**Baseline commit:** b7c5d9f
**范围:** 整个 SecPivot 第一方仓库；中文 Markdown；用户授权审计、修复及界面优化。

---

## 1. Executive Summary

本次为修复前审计快照。项目已有严肃的秘密按需读取、冲突检查、原子本地写入和测试体系，但错误路径仍存在跨层状态问题。15 项发现中 14 项经代码控制流或浏览器合成场景确认，1 项资源并发风险待压力复现；Confirmed 不等于每个目标平台均完成运行验证。

库存覆盖 252 个可审查第一方文件、82,147 行，所有库存路径进行了风险信号扫描，高风险链路作深读；并非所有文件逐行语义审查。排除 .git、node_modules、target/dist、自动生成 schema、二进制图标和无关 .trae。没有读取用户密码库。Windows 前端 206、Rust 457 测试通过，check 零错误警告；基线 verify 因两份文件格式失败而停止；build/clippy/完整门将在修复后重新运行。

评分为修复前判断，密码管理器的数据完整性、安全权重最高；稳定性、测试和发布其次；性能与设计以已有机制和复现证据判断。没有发现不等于没有问题，不承诺零 bug。

### Score Dashboard

| Dimension       | Score / 10 | Grade | 理由                                                                           |
| --------------- | ---------- | ----- | ------------------------------------------------------------------------------ |
| Security        | 6.5        | B     | 秘密读取与 masked IPC 有保护，SP-02/04/11/12 仍有边界风险，跨平台覆盖 Medium。 |
| Stability       | 5.5        | C     | SP-01 的部分提交和 SP-05 的空 catch 影响用户状态。                             |
| Performance     | 7.0        | B     | 树索引/虚拟列表合理，URI/列表读取与连接上限缺证，覆盖 Medium。                 |
| Testing         | 6.0        | B     | 663 个基线测试通过，但官方 XML 与真实交互缺口已经复现。                        |
| Maintainability | 6.5        | B     | 模块分层存在，SP-14 陈旧维护引用和大型状态组件增加成本。                       |
| Design          | 6.0        | B     | 紧凑视觉可保持，提交结果与 Escape 所有权须纠正。                               |
| Release         | 5.5        | C     | 格式总门失败，签名 runner 隔离和 action 锁定不足；无实际发布验证。             |
| Overall         | 6.1        | B     | 以数据完整性/安全为主的审计判断，非机械平均或质量保证。                        |

分值越高越好，10 为干净可发布；评分不代替证据与覆盖边界。

### Finding Statistics

当前详细卡片统计包含原 15 张卡片与新增 SP-S05；SP-15 本轮由 Suspected 复核为 Confirmed。其他历史补充跟踪记录不计入本表。下方单独保留 `b7c5d9f` 原始 15 项统计与 6.1 分判断，不将复核进展静默改写为基线结果。

| Severity  | Count  | Confirmed | Suspected |
| --------- | ------ | --------- | --------- |
| Critical  | 0      | 0         | 0         |
| High      | 4      | 4         | 0         |
| Medium    | 9      | 9         | 0         |
| Low       | 3      | 3         | 0         |
| Info      | 0      | 0         | 0         |
| **Total** | **16** | **16**    | **0**     |

#### 原始基线统计（保持不变）

| Severity  | Count  | Confirmed | Suspected |
| --------- | ------ | --------- | --------- |
| Critical  | 0      | 0         | 0         |
| High      | 4      | 4         | 0         |
| Medium    | 8      | 7         | 1         |
| Low       | 3      | 3         | 0         |
| Info      | 0      | 0         | 0         |
| **Total** | **15** | **14**    | **1**     |

## 2. Project Map

Svelte 5 主页面/独立窗口 → services/vault 与 invoke → commands → VaultSessions/Session → keepass Database → 本地原子持久化或远程 S3/WebDAV 条件提交 → 可选本地镜像。敏感字段在 DTO 中用存在性标记，按需提取；clipboard、临时附件与锁定生命周期属于隐私边界。配置由 Rust 规范化持久化，renderer 收到 masked 凭据；平台层负责 DPAPI/keyring/SAF/自动输入。Bridge 与 KeePassRPC 是本地网络认证边界；GitHub Actions 是工件与签名边界。

### Coverage Matrix

| Dimension                      | Coverage     | Evidence inspected                                                  | Exclusions / limits                            |
| ------------------------------ | ------------ | ------------------------------------------------------------------- | ---------------------------------------------- |
| Architecture                   | Medium       | vault/session/persist、services/vault、主页面、commands             | 没有逐行语义审阅全部 82,147 行                 |
| Security                       | Medium       | security、bridge/RPC crypto、平台 secret、release                   | 未使用真实库；未运行 macOS/Linux/Android       |
| Stability                      | Medium       | persist、file_io、session、import、UI 错误路径                      | 未注入真实网络/提供商故障                      |
| Performance                    | Medium       | tree 索引、virtual-list、remote 响应、KDF                           | 未做大型真实库基准                             |
| Testing                        | High         | 206 个前端测试、457 个 Rust 测试、headless Edge fixture             | Windows 本地测试不能证明其他平台               |
| Maintainability                | Medium       | 主页面、EntryEditor、settings、技能导航                             | 长测试文件按主题搜索，未逐行覆盖               |
| Design / Principles            | Medium       | 持久化结果、Select/ModalShell、模板效果                             | 保留紧凑视觉，不进行无证据重设计               |
| Release                        | Medium       | ci.yml、release.yml、Cargo profiles、打包脚本                       | 未执行发布、签名或远端工作流                   |
| Documentation                  | High         | SKILL、六份维护引用、TODO、历史审计                                 | 用户手册行为未在全部目标平台验证               |
| Observability / Operability    | Medium       | 错误字符串、远程故障、保存完成、UI feedback                         | 桌面工具无需照搬服务器 metrics 体系            |
| Configuration Safety           | High         | TS/Rust defaults、normalize、config/store、CSP                      | 原生 keyring 未在目标系统执行                  |
| Data Integrity                 | Medium       | XML/CSV、保存、镜像、备份、SAF、冲突合并                            | 未验证每种第三方格式及真实 SAF provider        |
| Privacy / Data Governance      | Medium       | clipboard、vault DTO、secret store、导出、临时附件                  | 已有记住凭据功能涉及产品安全政策，未读取其数据 |
| Accessibility / UX Correctness | Medium       | 1100×720 合成 UI、Select、tree、editor                              | 还需 dark/light/custom 和 720/420 宽度回归     |
| Supply Chain / Reproducibility | Medium       | lockfiles、npm audit production、action refs、signing               | 无 cargo-audit 全依赖公告证据，未执行签名      |
| Cost / Resource Economics      | Medium       | 本地连接、S3 分页、SAF read、编译 profiles                          | 未执行本地并发压力和内存测量                   |
| AI / LLM Safety                | Not assessed | 依赖与网络入口搜索：没有模型调用、RAG 或代理工具 API                | 不适用                                         |
| Fallback / Defensive Code      | High         | catch、.ok()、默认值与解析路径                                      | 外部系统故障未全部动态执行                     |
| Testing Authenticity           | High         | XML 测试、组件契约测试、browser probe                               | 源码正则只能补充，不能代替交互验证             |
| Type Safety                    | Medium       | DTO、invoke、serde、unsafe 平台适配                                 | 平台 FFI 未全运行                              |
| Frontend State                 | High         | GroupTree、EntryEditor、services/vault、session-state               | 大型主页面未逐行语义阅读                       |
| Backend API                    | Medium       | commands、remote trait、S3/WebDAV、bridge/RPC dispatch              | 未执行所有 IPC 原生调用                        |
| Dependency Weight              | Medium       | package.json、Cargo.toml、lockfile、profiles                        | 未量化全部传递依赖体积                         |
| Code Consistency               | High         | 导入/未接入辅助函数、format、模块路由                               | 不将主观命名风格当缺陷                         |
| Comment Coverage               | Medium       | XML 注释、security-model、模块 doc comments                         | 不按注释比例机械扣分                           |
| Concurrency                    | Medium       | session registry、persist completion、模板、clipboard、accept loops | 没有并发洪泛和全部平台竞态实测                 |

## 3. Top Risks

| ID    | Severity | 风险                                                     |
| ----- | -------- | -------------------------------------------------------- |
| SP-01 | High     | 远程提交成功后本地镜像失败，活动会话保留旧密钥和同步基线 |
| SP-02 | High     | Android 文档读取失败仍允许截断写入，回滚无法恢复原库     |
| SP-03 | High     | XML 导出把 KDBX 内部保护格式误作明文交换格式             |
| SP-04 | High     | 跨传输同名配置共用系统钥匙串地址                         |
| SP-05 | Medium   | 模板密文读取失败被吞掉，空密码草稿仍可保存               |
| SP-06 | Medium   | 持久化折叠分组首次渲染自动展开                           |
| SP-07 | Low      | 回收站置底辅助函数未接入树渲染                           |
| SP-08 | Medium   | 下拉框 Escape 冒泡关闭整个编辑器                         |
| SP-09 | Medium   | XML 交换丢失 OTP 类型、字段保护及到期信息                |
| SP-10 | Medium   | S3 列表只读第一页                                        |
| SP-11 | Medium   | 剪贴板所有权字符串在失去所有权或部分锁定路径后仍保留     |
| SP-12 | Medium   | Android 构建与签名共享 runner，发布 checkout 未锁定      |

## 4. Detailed Findings

原 15 张卡片的问题描述与源码位置保留修复前证据；SP-15 的 Status 已明确标出本轮复核变化。新增 SP-S05 与最新验证记录放在补充证据部分。

### Finding: SP-01 远程提交成功后本地镜像失败，活动会话保留旧密钥和同步基线

- Severity: High
- Confidence: High
- Category: Data Integrity
- Status: Confirmed
- Affected area: persist_snapshot / persist_change
- Evidence:
  - File: `src-tauri/src/vault/persist.rs:308`（相对仓库根，审计基线）
  - Function / Module: `persist_snapshot / persist_change`
  - Relevant behavior: 上传成功后镜像写入通过 ? 返回错误，调用方只在 Ok 时接纳新密钥及 base_hash。
- Problem: 远程提交成功后本地镜像失败，活动会话保留旧密钥和同步基线。
- Why it matters: 更改主密钥时远程已使用新密钥，界面却报失败并保留旧密钥；后续保存误报冲突。
- Realistic failure scenario: SaveLocal 模式，本地镜像目录不可写；修改主密钥，远程 PUT 成功而本地写入失败。
- Minimal fix: 把远程已提交与镜像告警明确分开；提交后必须接纳密钥和基线，再呈现告警。
- Better long-term fix: 建立明确的持久化提交结果契约，所有保存、合并、重加密调用共享它。
- Regression test suggestion: MemoryStorage 加不可写镜像路径；断言新密钥能打开远程，会话基线等于提交字节，镜像告警可见。
- Estimated effort: 4–8 小时

### Finding: SP-02 Android 文档读取失败仍允许截断写入，回滚无法恢复原库

- Severity: High
- Confidence: High
- Category: Data Integrity
- Status: Confirmed
- Affected area: write_uri / rollback_uri_bytes
- Evidence:
  - File: `src-tauri/src/vault/file_io.rs:157`（相对仓库根，审计基线）
  - Function / Module: `write_uri / rollback_uri_bytes`
  - Relevant behavior: read_uri_with(...).ok() 把读错误转成没有旧内容，然后继续截断；rollback 无旧内容时给出此前为空的错误说明。
- Problem: Android 文档读取失败仍允许截断写入，回滚无法恢复原库。
- Why it matters: 旧数据库可能在提供商读失败、部分写失败后丢失。Windows 代码追踪确认；未在 Android 提供商上执行。
- Realistic failure scenario: 已有 SAF 文档临时不可读，但可截断写；写到一半失败。
- Minimal fix: 读取旧内容失败时终止写入；成功读取的空内容才视为空文档；回滚后读回核验。
- Better long-term fix: 提取可注入读写的事务助手并限制 URI 流读取量。
- Regression test suggestion: 模拟读失败、部分写失败、回滚写成功但内容不符；读失败应零次调用 writer。
- Estimated effort: 3–6 小时

### Finding: SP-03 XML 导出把 KDBX 内部保护格式误作明文交换格式

- Severity: High
- Confidence: High
- Category: Data Integrity
- Status: Confirmed
- Affected area: buildKeePassXml / export_xml
- Evidence:
  - File: `src/lib/utils/kdbx-xml.ts:197; src-tauri/src/vault/security.rs:1117`（相对仓库根，审计基线）
  - Function / Module: `buildKeePassXml / export_xml`
  - Relevant behavior: 密码写 Protected=True 加未加密 Base64；官方 PlainXml 写 ProtectInMemory=True 加 XML 转义原文。
- Problem: XML 导出把 KDBX 内部保护格式误作明文交换格式。
- Why it matters: 导入其他 KeePass 客户端时密码被误解，迁移后无法登录；现有前后端测试复制了错误假设。
- Realistic failure scenario: 导出含 Unicode 密码的 XML，交给遵循 KeePass PlainXml 的客户端导入。
- Minimal fix: 前后端写官方明文格式；识别旧 SecPivot XML 时明确兼容范围，拒绝其他加密内部 XML。
- Better long-term fix: 使用公开官方样本校准交换格式，避免仅自导自入。
- Regression test suggestion: 使用官方 PlainXml 样本及含 &、<、Unicode 的合成密码检查解析和跨格式保护标记。
- Estimated effort: 2–4 小时

### Finding: SP-04 跨传输同名配置共用系统钥匙串地址

- Severity: High
- Confidence: High
- Category: Configuration
- Status: Confirmed
- Affected area: encrypt_profile_creds / decrypt_profile_creds
- Evidence:
  - File: `src-tauri/src/config/store.rs:108`（相对仓库根，审计基线）
  - Function / Module: `encrypt_profile_creds / decrypt_profile_creds`
  - Relevant behavior: 钥匙串地址只有 name/accessKey、name/secretKey；S3 与 WebDAV 可同时使用 config_1。
- Problem: 跨传输同名配置共用系统钥匙串地址。
- Why it matters: Linux/macOS 保存第二个配置覆盖第一个凭据，重启后连接使用错误账号。Windows DPAPI 内联存储不受此地址碰撞影响。
- Realistic failure scenario: 创建 s3/config_1 与 webdav/config_1，各设不同凭据，保存并重载。
- Minimal fix: 钥匙串地址包含规范传输路径及数据根隔离信息；通过旧引用读取兼容历史配置。
- Better long-term fix: 对 secret-store 身份和迁移制定契约。
- Regression test suggestion: 验证两种传输、两个数据根产生不同地址，并验证旧引用可加载；原生钥匙串需目标平台验证。
- Estimated effort: 2–4 小时

### Finding: SP-05 模板密文读取失败被吞掉，空密码草稿仍可保存

- Severity: Medium
- Confidence: High
- Category: Frontend State
- Status: Confirmed
- Affected area: applyTemplate
- Evidence:
  - File: `src/lib/components/EntryEditorDialog.svelte:607`（相对仓库根，审计基线）
  - Function / Module: `applyTemplate`
  - Relevant behavior: catch 为空，随后仍将部分模板应用；缺少卸载、切换会话和异步操作失效守卫；编辑模式还显示无效选择器。
- Problem: 模板密文读取失败被吞掉，空密码草稿仍可保存。
- Why it matters: 用户以为复制完整模板，实际生成缺密码条目；后返回请求还可能覆盖新草稿。
- Realistic failure scenario: 合成模板 hasPassword=true，但浏览器 fallback 的 `getActiveSessionId()` 为 null，秘密读取被跳过，仍产生缺密码的部分草稿；浏览器复现 error=false、save=true。源码空 catch 另行确认原生秘密读取失败也会继续应用。最初脚本虽注入读取异常，实际未进入读取分支，不能将它作为抛错路径的运行证据。
- Minimal fix: 原子解析全部字段；失败保留原草稿且显示错误；加载期间禁止保存，失效操作不应用，只在创建模式显示。
- Better long-term fix: 模板解析从组件剥离为有明确生命周期的助手。
- Regression test suggestion: 浏览器覆盖失败保留草稿、成功应用、锁定/切库、快速重选、Escape 关闭和编辑模式不显示。
- Estimated effort: 2–4 小时

### Finding: SP-06 持久化折叠分组首次渲染自动展开

- Severity: Medium
- Confidence: High
- Category: Frontend State
- Status: Confirmed
- Affected area: knownUuids initialization
- Evidence:
  - File: `src/lib/components/GroupTree.svelte:95`（相对仓库根，审计基线）
  - Function / Module: `knownUuids initialization`
  - Relevant behavior: knownUuids 只包含已展开分组；effect 将原有折叠分组当新增分组展开。
- Problem: 持久化折叠分组首次渲染自动展开。
- Why it matters: 每次重开库丢失用户树布局；浏览器合成 fixture 复现 nested 可见。
- Realistic failure scenario: 库中 normal 折叠、nested 为子分组，重新打开。
- Minimal fix: knownUuids 从完整初始树建立，expanded 单独遵循持久化标记。
- Better long-term fix: 统一树状态初始化和新增/删除协调逻辑。
- Regression test suggestion: 渲染后子节点隐藏；新增子分组可展开；内容更新不能改变原有折叠状态。
- Estimated effort: 30–60 分钟

### Finding: SP-07 回收站置底辅助函数未接入树渲染

- Severity: Low
- Confidence: High
- Category: Accessibility
- Status: Confirmed
- Affected area: root and recursive each blocks
- Evidence:
  - File: `src/lib/components/GroupTree.svelte:263; src/lib/components/GroupNode.svelte:351`（相对仓库根，审计基线）
  - Function / Module: `root and recursive each blocks`
  - Relevant behavior: 两个组件导入 orderedChildren，却都直接遍历 children。
- Problem: 回收站置底辅助函数未接入树渲染。
- Why it matters: 已声明的回收站置底功能在实际界面不生效，浏览器显示回收站第一。
- Realistic failure scenario: 原始 KDBX 树中回收站在普通组之前。
- Minimal fix: 根层和递归层使用 orderedChildren，避免修改持久化顺序。
- Better long-term fix: 保持一个只读显示排序入口。
- Regression test suggestion: 两层树用不同顺序 fixture，断言显示置底且原 children 数组不变。
- Estimated effort: 30–60 分钟

### Finding: SP-08 下拉框 Escape 冒泡关闭整个编辑器

- Severity: Medium
- Confidence: High
- Category: Accessibility
- Status: Confirmed
- Affected area: onListKeydown / ModalShell window handler
- Evidence:
  - File: `src/lib/components/Select.svelte:84`（相对仓库根，审计基线）
  - Function / Module: `onListKeydown / ModalShell window handler`
  - Relevant behavior: 下拉框阻止默认但不阻止传播，外层全局 Escape 同时关闭模态。
- Problem: 下拉框 Escape 冒泡关闭整个编辑器。
- Why it matters: 用户取消选择时未保存草稿被丢弃；浏览器复现 dialog 消失。
- Realistic failure scenario: 打开新条目模板选择器，按 Escape。
- Minimal fix: 消费下拉层 Escape，保持外层对话框和焦点；每个 listbox 使用唯一 ID。
- Better long-term fix: 嵌套交互统一消费规则，只有最顶层处理 Escape/Tab。
- Regression test suggestion: 实际浏览器确认第一次 Escape 关闭列表并回焦触发器，第二次才关闭编辑器。
- Estimated effort: 30–90 分钟

### Finding: SP-09 XML 交换丢失 OTP 类型、字段保护及到期信息

- Severity: Medium
- Confidence: High
- Category: Data Integrity
- Status: Confirmed
- Affected area: parseEntry / export_xml
- Evidence:
  - File: `src/lib/utils/kdbx-xml.ts:82; src-tauri/src/vault/security.rs:1105`（相对仓库根，审计基线）
  - Function / Module: `parseEntry / export_xml`
  - Relevant behavior: HmacOtp、SteamOtp 与 otp 都归入 totp；customFields 不保留 ProtectInMemory；导出没有 Times。
- Problem: XML 交换丢失 OTP 类型、字段保护及到期信息。
- Why it matters: HOTP 计数/Steam 类型退化，导入后受保护字段变成普通字段，到期约束丢失。
- Realistic failure scenario: 导出/导入带 HOTP counter、自定义保护字段和到期时间的条目。
- Minimal fix: 保留 OTP 原生键和 protected 属性，并在导出写 Times；避免重复写 otp 与原生键。
- Better long-term fix: 定义完整交换字段矩阵，分别标记 CSV 有损与 XML 可保存项。
- Regression test suggestion: 官方 XML fixture 导入至 Rust KDBX 后重新打开，断言字段保护、HOTP counter、Steam 类型和到期值。
- Estimated effort: 3–6 小时

### Finding: SP-10 S3 列表只读第一页

- Severity: Medium
- Confidence: High
- Category: Backend API
- Status: Confirmed
- Affected area: S3Storage::list / parse_list_v2
- Evidence:
  - File: `src-tauri/src/remote/s3.rs:231`（相对仓库根，审计基线）
  - Function / Module: `S3Storage::list / parse_list_v2`
  - Relevant behavior: 只请求一次 ListObjectsV2，忽略 IsTruncated 和 NextContinuationToken。
- Problem: S3 列表只读第一页。
- Why it matters: 超过第一页的远程库不出现在列表，用户无法选择。
- Realistic failure scenario: 前缀下超过 1000 对象，响应 IsTruncated=true。
- Minimal fix: 遍历 continuation-token，检测重复/缺失 token，使用整体时间和响应大小限制。
- Better long-term fix: S3/WebDAV 共享有界列表响应读取。
- Regression test suggestion: 本地 HTTP mock 两页，第二页 query 含正确编码 token；重复 token 应失败而非循环。
- Estimated effort: 2–4 小时

### Finding: SP-11 剪贴板所有权字符串在失去所有权或部分锁定路径后仍保留

- Severity: Medium
- Confidence: High
- Category: Privacy
- Status: Confirmed
- Affected area: clearClipboardIfUnchanged / lock lifecycle
- Evidence:
  - File: `src/lib/utils/clipboard.ts:47`（相对仓库根，审计基线）
  - Function / Module: `clearClipboardIfUnchanged / lock lifecycle`
  - Relevant behavior: 外部剪贴板变化时 return 不释放 lastCopiedText；clearOnLock=false 路径不释放引用；浏览器 fallback 忽略 execCommand 返回值。
- Problem: 剪贴板所有权字符串在失去所有权或部分锁定路径后仍保留。
- Why it matters: 已锁定后渲染器仍额外保留密码文本，失败复制还可能被报告成功。
- Realistic failure scenario: 复制密码后在另一应用复制内容，等待清除计时器；或关闭 clearOnLock 后锁库。
- Minimal fix: 释放所有权与清空系统剪贴板分离；锁定始终释放 JS 引用；失效清除不能清掉新复制内容；复制失败必须报告。
- Better long-term fix: 统一前后端有代次的所有权管理和计时器生命周期。
- Regression test suggestion: mock Clipboard 延迟读、外部变化、clearOnLock=false、execCommand=false，验证无越权清除且失败可见。
- Estimated effort: 2–4 小时

### Finding: SP-12 Android 构建与签名共享 runner，发布 checkout 未锁定

- Severity: Medium
- Confidence: High
- Category: Supply Chain
- Status: Confirmed
- Affected area: android / publish jobs
- Evidence:
  - File: `.github/workflows/release.yml:252`（相对仓库根，审计基线）
  - Function / Module: `android / publish jobs`
  - Relevant behavior: 构建依赖执行与注入签名凭据位于同一 job；publish 使用 actions/checkout@v4，而其他 job 固定 SHA。
- Problem: Android 构建与签名共享 runner，发布 checkout 未锁定。
- Why it matters: 被攻陷构建进程可存活到签名步骤读取凭据；浮动 action 扩大供应链变更面。仅证明结构暴露，未观察到入侵。
- Realistic failure scenario: Android 构建依赖启动后台进程，下一步骤注入 keystore 密码。
- Minimal fix: 签名用独立干净 job，仅下载受验证的未签名 APK；锁定 checkout SHA；密码不用明文 argv，清理使用 always/trap。
- Better long-term fix: 限定可签名工件与来源，保留 checksum/provenance。
- Regression test suggestion: 工作流静态校验和 GitHub 实际构建/签名工件验证；本地 YAML 不作为 CI 成功证据。
- Estimated effort: 2–4 小时

### Finding: SP-13 开发 CSP 连接端口与 Vite 不一致

- Severity: Low
- Confidence: High
- Category: Configuration
- Status: Confirmed
- Affected area: devCsp
- Evidence:
  - File: `src-tauri/tauri.conf.json:31`（相对仓库根，审计基线）
  - Function / Module: `devCsp`
  - Relevant behavior: devUrl/Vite 为 14200，connect-src 放行 1420。
- Problem: 开发 CSP 连接端口与 Vite 不一致。
- Why it matters: 同源 HMR 可由 `self` 放行；当 localhost 页面回退连接 Vite 公告的 `127.0.0.1:14200` 地址时，旧允许列表会拦截该 WebSocket。复核已收窄原先“热更新必然被拦截”的描述。
- Realistic failure scenario: 运行 dev，主 WebSocket 不可用后客户端尝试数字地址回退；原生 Tauri 上是否进入此分支尚未实测。
- Minimal fix: 开发 CSP 精确允许实际 Vite HTTP/WS 端口。
- Better long-term fix: 配置端口集中校验。
- Regression test suggestion: 真实 Vite 服务下分别验证同源 HMR 和数字地址连接，观察 CSP violation；原生 Tauri dev 热更新仍需桌面运行证据。
- Estimated effort: 15–30 分钟

### Finding: SP-14 基线格式门失败与维护导航陈旧

- Severity: Low
- Confidence: High
- Category: Documentation
- Status: Confirmed
- Affected area: maintenance references / format gate
- Evidence:
  - File: `skills/secpivot-dev/references/project-structure.md; src/lib/components/GroupTree.svelte`（相对仓库根，审计基线）
  - Function / Module: `maintenance references / format gate`
  - Relevant behavior: 基线 verify 在 Prettier 失败；引用不存在 entry-copy.ts、modal-shared.css，传输/凭据说明与当前源码不符。
- Problem: 基线格式门失败与维护导航陈旧。
- Why it matters: 维护者沿错误导航修改旧架构；总门无法起跑。
- Realistic failure scenario: 按 skill 导航查找 retired 文件或运行 npm run verify。
- Minimal fix: 只修正有源码证据的引用和格式，不因子项全勾选就勾父 TODO。
- Better long-term fix: 文档导航链接和符号采用自动检查。
- Regression test suggestion: Prettier/cargo fmt 通过；校核文档文件存在且职责与源码一致。
- Estimated effort: 1–2 小时

### Finding: SP-15 本地 bridge/RPC 连接的并发工作缺少统一上限

- Severity: Medium
- Confidence: Medium
- Category: Concurrency
- Status: Confirmed（基线为 Suspected；`c97a894` 隔离压测复核确认并修复线程放大，详见补充证据）
- Affected area: accept loops / connection handling
- Evidence:
  - File: `src-tauri/src/bridge/server.rs; src-tauri/src/rpc/server/mod.rs`（相对仓库根，审计基线）
  - Function / Module: `accept loops / connection handling`
  - Relevant behavior: 连接后创建处理线程；配对限流晚于连接，RPC SRP 等待批准没有已证统一连接预算。
- Problem: 本地 bridge/RPC 连接的并发工作缺少统一上限。
- Why it matters: 同用户本地程序可能造成线程/内存放大。代码显示资源形态；未执行洪泛，不能声称已形成拒绝服务。
- Realistic failure scenario: 本地进程并发建立大量连接并保持未完成握手。
- Minimal fix: 先测量连接/线程峰值；若复现，accept 边界加并发上限及握手超时。
- Better long-term fix: 有限线程池和按生命周期释放的连接许可。
- Regression test suggestion: 隔离的合成握手压力测试，断言并发资源受限并可恢复。
- Estimated effort: 3–6 小时

## 5. Architecture Analysis

- Coverage: Medium
- Inspected evidence: vault/session/persist、services/vault、主页面、commands
- Exclusions / limits: 没有逐行语义审阅全部 82,147 行

SP-01、SP-05：提交边界与异步草稿所有权需要明确；不建议重写。

## 6. Security Analysis

- Coverage: Medium
- Inspected evidence: security、bridge/RPC crypto、平台 secret、release
- Exclusions / limits: 未使用真实库；未运行 macOS/Linux/Android

SP-02、SP-04、SP-11、SP-12；现有按需秘密读取和 ETag 保护有价值。

## 7. Stability Analysis

- Coverage: Medium
- Inspected evidence: persist、file_io、session、import、UI 错误路径
- Exclusions / limits: 未注入真实网络/提供商故障

SP-01、SP-02、SP-05、SP-10 的异常路径比常规路径风险更高。

## 8. Performance Analysis

- Coverage: Medium
- Inspected evidence: tree 索引、virtual-list、remote 响应、KDF
- Exclusions / limits: 未做大型真实库基准

树已有 WeakMap 索引与虚拟列表；URI 无界读和列表累积需要有界。

## 9. Testing Analysis

- Coverage: High
- Inspected evidence: 206 个前端测试、457 个 Rust 测试、headless Edge fixture
- Exclusions / limits: Windows 本地测试不能证明其他平台

绿测漏过 SP-03、SP-05–08；应增加交互和官方互操作 fixture。

## 10. Maintainability Analysis

- Coverage: Medium
- Inspected evidence: 主页面、EntryEditor、settings、技能导航
- Exclusions / limits: 长测试文件按主题搜索，未逐行覆盖

SP-14：陈旧导航有实证；大组件是变更风险信号，不单独判 bug。

## 11. Design / Principles Analysis

- Coverage: Medium
- Inspected evidence: 持久化结果、Select/ModalShell、模板效果
- Exclusions / limits: 保留紧凑视觉，不进行无证据重设计

SP-01、SP-05、SP-08 违反显式失败与单一交互所有权。

## 12. Release Analysis

- Coverage: Medium
- Inspected evidence: ci.yml、release.yml、Cargo profiles、打包脚本
- Exclusions / limits: 未执行发布、签名或远端工作流

SP-12、SP-14；本地快编译与 CI 发布优化隔离保持合理。

## 13. Documentation Analysis

- Coverage: High
- Inspected evidence: SKILL、六份维护引用、TODO、历史审计
- Exclusions / limits: 用户手册行为未在全部目标平台验证

SP-14；历史修复不能直接当成当前问题，TODO 全措辞需要证据。

## 14. Observability / Operability Analysis

- Coverage: Medium
- Inspected evidence: 错误字符串、远程故障、保存完成、UI feedback
- Exclusions / limits: 桌面工具无需照搬服务器 metrics 体系

SP-01、SP-05 的成功/失败信号不准确；新增诊断禁止打印秘密。

## 15. Configuration Safety Analysis

- Coverage: High
- Inspected evidence: TS/Rust defaults、normalize、config/store、CSP
- Exclusions / limits: 原生 keyring 未在目标系统执行

SP-04、SP-13；masked IPC 和未知字段默认已有测试。

## 16. Data Integrity Analysis

- Coverage: Medium
- Inspected evidence: XML/CSV、保存、镜像、备份、SAF、冲突合并
- Exclusions / limits: 未验证每种第三方格式及真实 SAF provider

SP-01–03、SP-09；当前 ETag/CAS 冲突处理不作为缺陷重复报告。

## 17. Privacy / Data Governance Analysis

- Coverage: Medium
- Inspected evidence: clipboard、vault DTO、secret store、导出、临时附件
- Exclusions / limits: 已有记住凭据功能涉及产品安全政策，未读取其数据

SP-11；匿名合成 fixture，不收集或打开用户数据库。

## 18. Accessibility / UX Correctness Analysis

- Coverage: Medium
- Inspected evidence: 1100×720 合成 UI、Select、tree、editor
- Exclusions / limits: 还需 dark/light/custom 和 720/420 宽度回归

SP-05–08；错误、焦点和状态正确性优先于装饰性改版。

## 19. Supply Chain / Reproducibility Analysis

- Coverage: Medium
- Inspected evidence: lockfiles、npm audit production、action refs、signing
- Exclusions / limits: 无 cargo-audit 全依赖公告证据，未执行签名

SP-12；生产 npm audit 未报告漏洞，不等于供应链安全证明。

## 20. Cost / Resource Economics Analysis

- Coverage: Medium
- Inspected evidence: 本地连接、S3 分页、SAF read、编译 profiles
- Exclusions / limits: 未执行本地并发压力和内存测量

SP-10、SP-15 和 URI 读取需要资源预算；无计费模型调用面。

## 21. AI / LLM Safety Analysis

- Coverage: Not assessed
- Inspected evidence: 依赖与网络入口搜索：没有模型调用、RAG 或代理工具 API
- Exclusions / limits: 不适用

产品无 AI 执行面；仓库技能文本不等于产品 AI 面。

## 22. Fallback / Defensive Code Analysis

- Coverage: High
- Inspected evidence: catch、.ok()、默认值与解析路径
- Exclusions / limits: 外部系统故障未全部动态执行

SP-02、SP-05、SP-11；兼容旧导出要明确标记，不能把未知加密猜成明文。

## 23. Testing Authenticity Analysis

- Coverage: High
- Inspected evidence: XML 测试、组件契约测试、browser probe
- Exclusions / limits: 源码正则只能补充，不能代替交互验证

SP-03 的自定义格式断言复制了实现；SP-07 有函数却没接入渲染。

## 24. Type Safety Analysis

- Coverage: Medium
- Inspected evidence: DTO、invoke、serde、unsafe 平台适配
- Exclusions / limits: 平台 FFI 未全运行

SP-01 结果类型不足以表达部分提交；不是所有 unsafe 都构成 bug。

## 25. Frontend State Analysis

- Coverage: High
- Inspected evidence: GroupTree、EntryEditor、services/vault、session-state
- Exclusions / limits: 大型主页面未逐行语义阅读

SP-05、SP-06；异步操作必须同时校验会话和组件生命周期。

## 26. Backend API Analysis

- Coverage: Medium
- Inspected evidence: commands、remote trait、S3/WebDAV、bridge/RPC dispatch
- Exclusions / limits: 未执行所有 IPC 原生调用

SP-01、SP-10；避免返回上传失败来描述已经提交的远程状态。

## 27. Dependency Weight Analysis

- Coverage: Medium
- Inspected evidence: package.json、Cargo.toml、lockfile、profiles
- Exclusions / limits: 未量化全部传递依赖体积

reqwest、keepass、平台插件有实际职责；无证据支持大规模换依赖。

## 28. Code Consistency Analysis

- Coverage: High
- Inspected evidence: 导入/未接入辅助函数、format、模块路由
- Exclusions / limits: 不将主观命名风格当缺陷

SP-07、SP-14；仅修复实际遗漏和门禁失败。

## 29. Comment Coverage Analysis

- Coverage: Medium
- Inspected evidence: XML 注释、security-model、模块 doc comments
- Exclusions / limits: 不按注释比例机械扣分

SP-03、SP-14：错误格式注释可持续误导测试，应同步纠正。

## 30. Concurrency Analysis

- Coverage: Medium
- Inspected evidence: session registry、persist completion、模板、clipboard、accept loops
- Exclusions / limits: 没有并发洪泛和全部平台竞态实测

SP-01、SP-05、SP-11 为已追踪风险；SP-15 仍为 Suspected。

## 31. Principles Compliance

### Principles Violated

| Principle                   | Violations | Severity      | Affected Areas                            |
| --------------------------- | ---------- | ------------- | ----------------------------------------- |
| Explicit commit boundary    | 1          | High          | SP-01：远程提交不能由镜像失败伪装成未提交 |
| Fail-Fast                   | 2          | High / Medium | SP-02、SP-05：读失败不能假装空内容        |
| Interaction/state ownership | 3          | Medium        | SP-05、SP-06、SP-08                       |
| Boundary contract           | 2          | High / Medium | SP-03、SP-09：XML 官方格式及字段保真      |

### Principles Respected

持久化与会话锁已分离；远程 ETag 防止条件覆盖；树索引避免逐节点递归计数；DTO 按需获取秘密；配置 masked 与 Rust 默认兼容均有测试。保留这些机制，修复边界即可。

## 32. Recommended Fix Order

### Fix Immediately

SP-01–04：远程提交状态、SAF 原库保护、XML 互操作和凭据身份隔离。先补可失败的回归再实施。

### Fix Before Stable Release

SP-05–13：原子模板、树状态、Escape、完整 XML 字段、S3 分页、剪贴板释放和签名隔离。全门通过后才进入发布流程。

### Schedule Later

SP-15 先做隔离压测；KDF 耗时校准和 1PUX 迁移按能力证据安排。不要自动勾选 Android/CI 待办。

### Ignore for Now

纯主观装饰改版、无测量依据的大拆分、不适用的服务器指标和 AI 面；没有证据时不把历史结论复报为新 bug。

## 33. Quick Wins

SP-06、SP-07、SP-08、SP-13、SP-14 都有低成本局部修复；其中树排序、焦点回归和错误提示直接改善紧凑界面体验。

## 34. Long-term Refactor Plan

仅建议将持久化提交结果与模板异步解析变成明确的局部契约；收益是可验证部分提交和草稿所有权，风险是漏接调用方，测试须覆盖保存、改钥、合并、配置重加密与切会话。不建议仓库整体重写。

## 35. Competitor Evidence / 竞品能力取舍

资料访问日期：2026-11-09。官方能力存在不代表 SecPivot 尚未具备，以下基于当前源码比较。

| 产品与来源                                                                                                                      | 官方能力                                                           | SecPivot 对比与取舍                                                      |
| ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| [KeePass 官方 XML writer](https://raw.githubusercontent.com/dlech/KeePass2.x/master/KeePassLib/Serialization/KdbxFile.Write.cs) | PlainXml 原文加 ProtectInMemory；Default 内部格式 XOR 后 Base64    | 直接用于 SP-03 格式修复；不能沿用未加密 Base64 假设                      |
| [KeePassXC 用户指南](https://keepassxc.org/docs/KeePassXC_UserGuide)                                                            | KDF 解密耗时校准、数据库报告、模板克隆、窗口自动输入、现代迁移格式 | 已有安全报告/模板/自动输入，优先补模板可靠性；KDF 校准是有价值未完成能力 |
| [Bitwarden 导出](https://bitwarden.com/help/export-your-data/)                                                                  | JSON 明文/加密、CSV 有损、附件 ZIP                                 | 明确 XML/CSV 保真边界；附件迁移需独立完整性与安全设计                    |
| [1Password 当前导出](https://support.1password.com/export/)                                                                     | 1Password 8 使用 1PUX 或 CSV                                       | 已有 legacy 1PIF 不能代表当前迁移完整；1PUX 列后续独立能力               |
| [KeeWeb](https://keeweb.info/)                                                                                                  | 紧凑离线界面、附件拖入、受保护字段                                 | 保留现有密集视觉，改善错误/焦点/排序，避免无关信息挤占主界面             |

不引入用户已排除的任意插件体系、事件自动化或硬件密钥；不为“竞品有”而直接扩大秘密外传面。

## 36. Reverification / 真实性复核与边界

- SP-05–08 已用隔离浏览器合成库复现：模板缺少会话时部分应用且保存可用、折叠子组出现、回收站第一、Escape 关闭 editor。模板最初注入的读取异常未执行，已按实际路径修正证据；修复后脚本显式提供合成会话，验证真实读取异常保留草稿并提示失败，以及成功应用、清空模板和恢复焦点。脚本与截图保存在本任务 visualization 目录，未使用真实库。
- SP-03 与 KeePass 官方 writer 两个格式分支核对，现有测试确实重复错误假设。
- SP-01/02/04/09/10/11/12/13/14 是源码控制流或配置证据；平台/故障实测限制逐项记录。SP-15 单列 Suspected，没有洪泛证据。
- 旧审计中 ETag、会话保存锁、附件临时文件清理、配置 masked 等问题已有当前实现，不重复报告；大文件、unsafe、空 catch 本身不是自动缺陷。
- npm production audit 零公告、663 个测试通过不能证明零 bug。browser fallback 只能证明交互；Android、macOS/Linux keyring、真实签名工件和第三方原生导入仍需目标运行。

## 37. Remediation Evidence / 修复记录

评分、统计和问题位置保留 `b7c5d9f` 修复前快照；下表单独记录已落实的改动。没有将原生设备未测行为标记为验证完成。

| 发现          | 修复提交                                              | 直接验证                                                                                                                                                                                    | 剩余边界                                                             |
| ------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| SP-01         | `1d3eef7`                                             | 远端上传后镜像失败仍采纳新密钥/hash；使用新密钥重开上传字节，后续保存不误报冲突；健康镜像清除提示                                                                                           | 真云端及原生 UI 需复测                                               |
| SP-02         | `00f0779`                                             | 不可读文档零次写入；部分写入回滚；回滚读回必须相等；流读取最多分配 limit+1                                                                                                                  | Android provider I/O、非普通文件描述符同步行为尚需真机验证           |
| SP-03         | `3e18ad5`                                             | 前后端使用官方 PlainXml 原文与 ProtectInMemory；六项真实 DOMParser 回归涵盖兼容旧 SecPivot、拒绝外部加密格式及无效编码                                                                      | 原生 KeePass 导入未完成，TODO 保留部分完成                           |
| SP-04         | `1f085b8`、`fcee069`                                  | 48 个 config 测试通过，涵盖传输/数据根地址隔离；clippy 测试模块位置已修正                                                                                                                   | macOS/Linux 原生 keyring 迁移未实测                                  |
| SP-05         | `a562570`、`e53e5ef`                                  | 原子解析助手及浏览器失败/成功/清空与过期守卫通过；演示库首个状态回调已获得会话标识                                                                                                          | 桌面 IPC 未真机验证                                                  |
| SP-06         | `140fd7c`                                             | 初始 UUID 库存独立于展开集；浏览器确认折叠子组隐藏                                                                                                                                          | 仅使用合成库                                                         |
| SP-07         | `a74fc03`                                             | 根层和递归渲染均用排序后子组；浏览器确认回收站位于最后                                                                                                                                      | 仅使用合成库                                                         |
| SP-08         | `de8d0fa`                                             | 浏览器确认快速 Escape 仅收起下拉、编辑器保留、焦点回到触发器；顶层模态拥有键盘处理                                                                                                          | 原生 WebView 需复测                                                  |
| SP-09         | `42d2f1a`                                             | 保留原生 OTP 字段名、受保护自定义字段和过期时间；第七项浏览器往返回归及真实 KDBX 保存重开通过                                                                                               | 标准字段的全部属性及第三方原生导入仍需独立验证                       |
| SP-10         | `a5cded1`、`7ebe497`                                  | 两页真实 HTTP 请求先红后绿；保留 token/键名的空格、中文、实体和 CDATA；异常后页、整体超时、对象/页数/响应大小上限均通过                                                                     | 当前无 MinIO 服务，真实服务端验签未验证                              |
| SP-11（部分） | `39697a0`、`3e3066f`、`9638487`、`d8398f6`、`c7e7333` | 原生条件清理在同一锁内；两端只持有摘要；零秒无 renderer 基线并取消旧任务；相同内容的新复制保留独立身份；1000 次替换释放旧载荷；任务准备前预留代次，取消和首次线程创建交错不再重新安装旧任务 | 并发写入/跨进程 IPC 顺序与外部相同内容复制仍须处理；原生剪贴板未实测 |
| SP-12         | `95accb2`                                             | 独立 signer 无 checkout/构建代码；全部动作固定 SHA；真实内联 Bash 的合成测试验证密码 env 引用、失败清理和 ABI 检查                                                                          | 未执行真实 CI 签名任务或 APK 安装，仍需工件验证                      |
| SP-13         | `7c53ef8`                                             | 浏览器与真实 Vite 服务：旧配置同源 HMR 可用，数字地址回退被拒；修复后两者均可连接，零 connect-src violation                                                                                 | 未运行原生 Tauri dev；探针隔离了自动处理的 inline nonce/hash         |
| SP-14（部分） | `fa2bab1`                                             | 纠正项目地图与远端凭据说明；格式门禁恢复通过                                                                                                                                                | 其余过期文档及主题 token 仍需逐项核实                                |

此前 `1d3eef7` 的完整 `npm run verify` 通过（前端 208、Rust 463）。本轮完整门禁首次因凭据测试模块位置触发 clippy 失败，`fcee069` 调整后已重新运行完整 `npm run verify` 并通过：格式、Svelte check 零错误警告、150 键设置契约、生产构建、前端 208、Rust 单元测试 464 + ACL 集成测试 2、clippy `-D warnings` 均通过。验证日志位于本任务 visualization 目录的 `verify-after-android-acl.log`。审计 metadata 及两份历史 metadata 仅移除 JSON 尾逗号，全部通过严格 JSON 解析，Prettier override 防止后续重新生成无效 JSON。

新增真机反馈：Android 云端数据库点击“另存为”未弹出选择器，用户提供的提示文字为 `Command plugin:safe|create_document not allowed by ACL`。当前仓库注册与前端调用的插件名均为 `saf`；反馈文字中 `safe` 的拼写差异尚未通过安装包或原始设备日志核实。源码中存在已确认的 ACL 问题：`src-tauri/capabilities/android.json` 误用 `screens`，Tauri 2 忽略该字段，主窗口未获得 SAF 权限。`e5e3c4e` 改为 `windows: ["main"]`；实际 Tauri ACL 解析器在修复前复现 `plugin:saf|create_document` 拒绝，修复后两项回归证明本地主窗口可打开/新建文档，并且不授权其他窗口、远程页或桌面/iOS。此新增问题不混入修复前 15 项统计。需要新 APK 安装后的系统选择器与提供商写入实测；当前本机缺 Android SDK/NDK，未构建或发布 APK。

新增复核问题：受保护 `HmacOtp` 字段在获取验证码后被无条件写成未保护字段，使后续快照暴露种子。严重性 Medium，状态 Confirmed；测试先在原实现复现失败，`a2b0ddf` 保留原保护标记后通过，并确认保存重开仍隐藏种子、计数器从 42 推进到 43。此补充问题不计入修复前 15 项统计。

XML 两个提交的完整 `npm run verify` 均通过；最新完整门禁日志 `verify-xml-fidelity.log` 包含前端 210、Rust 单元测试 465 + ACL 集成测试 2、格式、Svelte check、150 键契约、构建及 clippy。随后 HOTP 安全修复通过完整后端门禁（Rust 466 + ACL 2、clippy、格式）；没有将该后端结果表述为重新运行了全部前端门禁。原生 KeePass portable 下载返回 HTML 而非 ZIP，未执行任何下载文件；原生互操作验证仍缺失。

新增演示会话问题：`refreshTabs` 只写 tab store 的 `browser`，内部 `activeSessionId` 仍为 null，模板及视图守卫无法开始。严重性 Low，状态 Confirmed；真实服务模块测试先在首次状态回调复现 null，`e53e5ef` 在 refresh/open/create 首次发布前设置标识，close/lock 发布 null 前清除。回归涵盖全部这些生命周期路径；新启动的开发服务上，无 getter 替身的模板失败/成功/清空及焦点回归通过。估计修复工作量 30–60 分钟。此问题不混入修复前统计，且不作为桌面持久化证据。

新增嵌套模板菜单问题：`GroupNode` 递归未传递 `templatesGroup` / `ontemplates`，已设置的深层模板组显示“设为”而不是“取消”，操作也不触发 page 回调。严重性 Medium，状态 Confirmed；浏览器深层合成分组先复现错误标签，`51f4113` 补传两个属性后，设为/取消的实际 page 回调和 patch 均通过。探针对浏览器不支持的数据库设置持久化做显式替身；未将其当成 KDBX 证据。深色/浅色 1100×720、自定义主题 720×720、深色 420×720 的菜单均在视口内，截图已检查，未新增 CSS/token。估计修复工作量 15–30 分钟。此问题不混入修复前统计。

CSP 复核同时区分测试浏览器的本地网络访问授权与产品 CSP：授权前浏览器拒绝本地 WebSocket，授权后同源 HMR 在旧配置就可工作；对数字地址的显式回退连接仍有确定的 CSP 拒绝，修复后放行。报告已收窄 SP-13 的失败场景，未声称所有旧配置的主 HMR 都失败。

`a5cded1` 完整门禁通过（前端 210、Rust 473 + ACL 2）。后续整合门禁出现分页 HTTP 502；此前归因于 mock 的 5 秒寿命提前结束，这一推断撤回。`7ebe497` 延长有限 mock 等待时间并改善诊断，但并未证明原始失败原因。后来记录到请求端点为 `127.0.0.1:59614`、响应实际 peer 为 `127.0.0.1:51400`，且系统代理已启用；本地 mock 只返回 200。`2cb9449` 仅在测试共享 HTTP 客户端禁用代理后，端点与 peer 一致，完整门禁通过。生产代理配置不变。

剪贴板补充复核：原生 UTF-16 读取把终止符后的内存填充视为文本，合成 `[A, 0, 0, X, 0]` 原来得到 `A\0\0X`，修复后得到 `A`。原前端分离读取/清空 IPC 可以删除两次调用之间到达的其他复制内容；真实服务模块的合成竞态先红后绿。`39697a0` 让 Windows 文本与二进制条件清理在同一次剪贴板锁内完成，比较副本零化；renderer 在确认/失去/无法确认所有权后释放记忆值。非 Windows 无原生条件清理时显式回退到可读的 Clipboard API，读失败不盲清。Web Clipboard API 本身仍无原子比较清空能力。触及真实 OS 剪贴板的测试必须显式设置 `SECPIVOT_CLIPBOARD_LIVE=1`，本轮未启用，不能将正常门禁当成原生剪贴板运行证明。

最新完整 `npm run verify` 对应 `39697a0` 的改动，日志 `verify-clipboard-atomic-final.log`：格式、Svelte check 零错误警告、150 键契约、生产构建、前端 212、Rust 475 + ACL 2、clippy `-D warnings` 全部通过。首次重跑曾在 clippy 范围判定风格检查失败，修正后已完整重跑成功。

SP-12 已落实源代码修复：`android` 仅构建并上传未签名 APK；独立 `sign-android` 下载工件并用固定 build-tools 35.0.0 签名，无 checkout 或构建源码。四项秘密仅注入签名 step，keystore 私有临时目录通过 EXIT/INT/TERM 清理，密码参数使用 `env:` 引用。最终 staged APK 校验签名、必需 native 库与独占 ABI 后才生成 SHA256SUMS。Windows `RELEASE.md` 随工件传递，发布任务删除可变 checkout；所有剩余动作均固定 SHA。顺带修正同一构建路径中的 Gradle 文件相对目录并在无法移除签名配置时失败。四项回归使用真实内联 Bash 和显式模拟工具，覆盖成功、秘密缺失、验签失败、缺失/混入 ABI 与 keystore 清理；YAML 图和全部内联 Bash 语法也通过。本机没有真实签名工具链，没有执行 CI、构建 APK 或使用正式签名秘密；不能据此恢复 Android 发布 TODO 的完成状态。

该次完整 `npm run verify` 的日志为 `verify-release-signing-isolation.log`，对应 `95accb2`：格式、Svelte check、150 键契约、构建、前端 216、Rust 475 + ACL 2、clippy `-D warnings` 全部通过。SP-11、SP-14 仍部分修复，SP-15 保持 Suspected。竞品差距、XML 原生互操作、剪贴板生命周期和真实发布工件继续处理；本阶段不承诺完整语义覆盖或零 bug。

追加手机反馈：用户确认安装版本为 **v1.6.5**。本轮 ACL 修复尚未构建进新 APK，仍需安装包含修复的包后复测；版本号本身不能解释 `safe` / `saf` 拼写差异。

`3e3066f` 继续处理 SP-11：后端在启动定时线程前将文本/整条目载荷转换为 32 字节 SHA-256 摘要并零化原输入；零秒退出也走零化。睡眠任务仅保留零化摘要，取消代次会在读取剪贴板前提前退出，条件比较仍在原生锁内完成。回归覆盖文本/二进制内容、长度变化、空字节、空内容与取消代次；没有操作真实剪贴板。renderer 残留、同内容新复制的代次、排队时序和每次复制一个等待线程仍须继续处理，SP-11 保持部分修复。

该后端整合曾两次出现分页连接失败。临时诊断记录 mock 接受连接后在尚未读到字节时返回 Windows `10035 / WouldBlock`，客户端对应 `10053 / ConnectionAborted`。新增延迟发送字节的真实 socket 回归稳定复现：Windows accepted socket 继承监听器非阻塞模式，`read_timeout` 不改变它。`b9ce539` 显式恢复 fixture 的阻塞模式后通过；这是独立于此前代理 502 的第二个已确认测试问题，也不是 mock 总寿命不足。生产 Bridge/RPC 已有该切换，无需重复修改；临时诊断全部删除。

此前完整 `npm run verify` 对应 `3e3066f`，日志 `verify-clipboard-fingerprint-sockets.log`：格式、Svelte check、150 键设置契约、生产构建、前端 **216**、Rust **477 + ACL 2**、clippy `-D warnings` 全部通过。基线评分/15 项统计未改，真实 Android APK、原生剪贴板、第三方 XML 导入及发布签名工件验证仍缺失；目标保持进行中。

`9638487` 补齐 renderer 秘密释放：取消定时清理不再保存任何复制基线；正间隔只保留 SHA-256 摘要，不保存模块全局密码；条件清理 IPC 仅接收固定 32 字节摘要。每次复制拥有独立对象身份，旧异步读取不再误删或释放较新的相同内容，锁后晚到摘要被代次拒绝。非 Windows 的 `clipboard_clear` 显式返回 false，由 renderer 执行清理，避免把 native stub 的 no-op 当成成功。旧式 `execCommand` 返回 false 会报错，抛异常和失败均在 finally 清空并移除临时 textarea。真实服务模块的五条失败路径先红后绿，并补充锁后摘要与读权限拒绝回归，共 8 项剪贴板前端测试；正常流程没有新增复制前的异步等待，保持现有可见视图 guard 的副作用位置。零秒后端调用也取消前一代任务。

`d8398f6` 将后台文本/整条目清理合并到 `commands/clipboard/scheduler.rs` 的一个工作线程、一个待执行槽。旧载荷在替换/取消时立即 Drop；回调执行剪贴板 I/O 不持调度锁，在原生锁内再次验证代次；unsupported 平台不启动无效线程。四项实际工作线程回归覆盖 1000 次替换、立即释放、取消、自己的截止时间、唤醒和处理中替换。尚未完整证明的是：并发写入与 IPC 的请求顺序、首次初始化与取消交错，以及外部程序重新复制相同内容时的原生序号保护。Web Clipboard API 没有原子 CAS，不能将 browser 身份 guard 描述成跨程序原子操作；SP-11 保持部分修复。

原生写入另有待复核候选：`platform/clipboard.rs` 当前写入使用空 HWND，并有原生分配失败后的释放路径需继续检查。本轮尝试在独立 Windows window station + desktop 中用合成内容验证；CreateWindowStation 对三个访问掩码均返回错误 5（Access denied），在任何 OpenClipboard 调用前已停止。没有读取或修改用户共享剪贴板，未取得原生写入成功/失败证据，不将候选算作新增 Confirmed 发现。

最新完整 `npm run verify` 对应 `d8398f6`，日志 `verify-clipboard-deadline-worker.log`：格式、Svelte check、150 键设置契约、生产构建、前端 **223**、Rust **481 + ACL 2**、clippy `-D warnings` 全部通过。此前 renderer 完整门禁日志 `verify-clipboard-renderer-fingerprints.log` 为前端 223、Rust 477 + ACL 2。源码注释与测试恢复默认设置的小修正另经格式/针对性回归确认，没有扩大为新的原生运行结论。Android 新 APK 和手机端选择器仍需目标环境验证，整体审计继续进行。

`c7e7333` 修复后端准备阶段与取消的交错：原实现先准备摘要/创建工作线程，再在安装任务时递增代次；首次初始化尚未发布时，取消通过 `OnceLock.get()` 看不到调度器，旧请求随后可以重新安装任务。针对原调度器执行“旧请求准备中 → 取消 → 旧请求安装”的回归实际失败，提示 `a cancelled request installed a new wipe`。修复将无工作线程的调度状态与线程创建分离；文本/整条目命令在摘要、原生读取和线程创建前预留代次，安装时在同一调度锁内核对代次。取消始终更新调度状态，尚未创建线程也有效。三项新增回归覆盖准备中取消、首次线程启动前取消，以及较旧准备晚完成时拒绝安装并立即释放其摘要。此处证明的是后端代次预留后的顺序，不等于 renderer 并发写入或跨进程 IPC 的调用顺序；SP-11 继续部分修复。

最新完整 `npm run verify` 对应 `c7e7333`，日志 `verify-clipboard-reservations.log`：格式、Svelte check、150 键契约、生产构建、前端 **223**、Rust **484 + ACL 2**、clippy `-D warnings` 全部通过。首次全门禁在 clippy 的单位值映射风格检查失败，修正后完整重跑通过。真实 OS 剪贴板测试仍未启用，Android 新 APK、设备选择器与云端 provider I/O 仍未验证；基线评分与 15 项发现统计不变。

SP-15 当前复核结论提升为 **Confirmed**，`c97a894` 已修复处理线程放大。隔离的 `127.0.0.1:0` socket fixture 调用真实 HTTP 解析器，64 条未完成请求在原线程接入方式下同时保留 64 个处理线程，预算断言实际失败（`bridge-connection-pressure-red.log`）；接入生产预算后同一 fixture 只启动 32 个线程。HTTP/RPC 均使用 `loopback.rs` 的共享接入策略，每服务最多 32 个处理线程（同时启用两者共 64），超额连接在创建线程前释放且不排队。预算保存在 managed 服务状态中，旧线程在监听器重启后继续占用自己的许可；实际共享 accept loop 回归验证了重启时拒绝新连接、旧连接关闭后的接入恢复与真实 socket 回显，另验证了 handler unwind 释放许可及 stop 通道断开退出。该证据证明线程放大和接入预算，未制造系统内存耗尽，也未运行完整 Tauri/浏览器扩展拒绝服务场景。原始 14 Confirmed + 1 Suspected 统计仍保留为审计基线，卡片 Status 与 metadata 明确记录当前复核状态。

`c97a894` 的完整 `npm run verify` 日志 `verify-browser-connection-budget.log`：格式、Svelte check、150 键契约、构建、前端 **223**、Rust **488 + ACL 2**、clippy 全部通过。停止监听器仍不等于撤销既有处理线程或审批；源码注释与维护参考已纠正此前“等待监听线程就已关闭所有连接”的误述，完整停机生命周期继续复核。

### Finding: SP-S05 RPC 重复 peek 导致合法 WebSocket 请求头误分类（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Backend API
- Status: Confirmed（源代码修复已验证）
- Affected area: KeePassRPC WebSocket upgrade detection
- Evidence:
  - File: `src-tauri/src/rpc/server/mod.rs`（`c97a894` 修复前源码）
  - Function / Module: `is_websocket_upgrade` / `serve_connection`
  - Relevant behavior: `peek` 每次从相同未消费位置读取，旧实现却重复追加到 head；仅能观察前 2 KiB，并可因累计长度达到上限提前认定是普通 HTTP。普通请求路径随后发送 404。
  - Exact-source probe: 从源码提取完整分类函数，`rustc -O --test` 在隔离 socket 分段写入合法请求头，返回 `Some(false)` 早于完整头到达，断言实际失败（`rpc-fragmented-head-red.log`）；修复后同一探针通过（`rpc-fragmented-head-green.log`）。最初未优化的短请求头 75 ms 探针未复现，未把该次通过当作缺陷证据。
- Problem: 合法分段握手或 Upgrade 位于前 2 KiB 之外的请求被误认为端口探测；正文中的相同文本也可能误导旧分类。
- Why it matters: 本机扩展收到错误响应后无法建立凭据服务连接，表现为间歇握手失败。
- Realistic failure scenario: 扩展或网络栈分段发送 HTTP 头，第一段没有 Upgrade；或者合法附加头把 Upgrade 推到前 2 KiB 之后。
- Minimal fix: 使用新 peek 快照替换旧观测，等待实际头终止符，仅解析头内 Upgrade 字段；64 KiB 限制只针对请求头，保留全部未消费字节供 tungstenite 读取；不完整头观察设截止时间并短暂让出线程。
- Better long-term fix: 完整停机取消与握手总耗时控制一并约束服务生命周期，必要时采用保留预读字节的流适配器。
- Regression test suggestion: 已加入四项真实 socket 回归：1 KiB 首段延迟补全且字节仍可完整读取、4 KiB 头与大小写/制表符、头外伪 Upgrade/超限头，以及请求头与 65,535 字节合法帧同时到达。既有真实 WS 往返与 404/CORS 探测继续通过。
- Estimated effort: 1–2 小时

`d4e72ce` 已修复 SP-S05。最新完整门禁日志 `verify-rpc-fragmented-head.log`：格式、Svelte check、150 键契约、生产构建、前端 **223**、Rust **492 + ACL 2**、clippy `-D warnings` 全部通过。原生扩展、Android APK/provider、真实剪贴板、XML 第三方导入与竞品能力补齐仍有未完成验证；整体目标保持进行中。补充发现不改变原始 15 项发现和 6.1 分基线。
