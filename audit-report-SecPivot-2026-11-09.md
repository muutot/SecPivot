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

当前详细卡片统计包含原 15 张卡片与新增 SP-S05–SP-S77；SP-15 本轮由 Suspected 复核为 Confirmed。其他历史补充跟踪记录不计入本表。下方单独保留 `b7c5d9f` 原始 15 项统计与 6.1 分判断，不将复核进展静默改写为基线结果。

| Severity  | Count  | Confirmed | Suspected |
| --------- | ------ | --------- | --------- |
| Critical  | 1      | 1         | 0         |
| High      | 19     | 19        | 0         |
| Medium    | 55     | 55        | 0         |
| Low       | 13     | 13        | 0         |
| Info      | 0      | 0         | 0         |
| **Total** | **88** | **88**    | **0**     |

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

原 15 张卡片的问题描述与源码位置保留修复前证据；SP-15 的 Status 已明确标出本轮复核变化。新增 SP-S05–SP-S12 与最新验证记录放在补充证据部分。

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
| [1Password 当前导出](https://support.1password.com/export/)                                                                     | 1Password 8 使用 1PUX 或 CSV                                       | 1PIF JSON-line 基础迁移已修复（SP-S10），完整保真仍部分；1PUX 独立待实现 |
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

### Finding: SP-S06 关闭浏览器服务后旧连接仍能继续处理请求（补充发现）

- Severity: Medium
- Confidence: High
- Category: Security / Concurrency
- Status: Confirmed（源代码修复已验证）
- Affected area: KeePassHttp / KeePassRPC listener shutdown
- Evidence:
  - File: `src-tauri/src/loopback.rs`、`bridge/server.rs`、`rpc/server/mod.rs`（`90bc787` 修复前源码）
  - Function / Module: `accept_loop` / `ServerHandle::stop` / request dispatch
  - Relevant behavior: stop 仅退出并等待监听线程，已创建的 HTTP/WS worker 没有取消代次；连接和审批继续存活，重启也没有使旧请求失效。
  - Real-socket reproduction: 调用真实共享 accept loop，连接建立后停止并 join listener，随后发送 `x`，仍得到 `write=Ok(()), read=Ok(1), byte=[120]`，断言实际失败，见 `browser-stop-existing-connection-red.log`。这是共享传输边界复现，未声称完整 Tauri/真实扩展读取了密码。
- Problem: 用户关闭浏览器集成不能撤销已经存在的连接与待审批；旧请求可在关闭后继续进入协议处理。
- Why it matters: 服务关闭设置与实际授权入口不一致，审批等待和连接许可还会占用资源。
- Realistic failure scenario: 浏览器已连接，用户在设置中关闭集成；扩展继续沿用旧连接发送请求，或者关闭前的关联审批稍后得到允许。
- Minimal fix: 每次启动创建独立 `ListenerLifetime`，stop 使代次失效、关闭注册 socket 并唤醒审批；协议处理在取得 vault mutex 后重查代次，与停止线性化；网络/存储 I/O 不持这些协议守卫。Windows 使用 100 ms native poll 的可取消流，保留较长逻辑读取超时。已获准的持久化 RPC 作业仍完成并采纳已提交结果，许可直到 worker 返回才释放。
- Better long-term fix: 向前端发送按请求身份关联的失效事件，及时清除已显示的审批和 SRP 密码；另复核单连接强制关闭、握手总耗时和写超时。
- Regression test suggestion: 已验证停止后旧 socket 不再回显、32 条空闲连接释放及重启恢复、待审批唤醒/旧代次不弹提示/不得注册旧 key、等 vault lock 的请求重查、事件 panic 不泄漏 token 或毒化 lifetime、不完整 WS 握手取消、跨三个 native poll 的分段读取、逻辑超时与获准作业直到完成仍保留许可。作业许可测试模拟获准工作，不能代替真实存储提交中 stop 的故障试验。
- Estimated effort: 3–5 小时

`9a994d3` 已修复 SP-S06。32 条 socket 的 native `shutdown(Both)` 均返回成功，但 Windows 已阻塞的 recv 在 peer 被释放前仍未退出，见 `browser-shutdown-sockets-probe.log`；因此撤回“仅关闭克隆 socket 就能及时结束 worker”的假设，以短 native poll 加取消检查解决。同一旧连接回归修复后得到 EOF。最新完整 `npm run verify` 日志 `verify-browser-shutdown.log`：格式、Svelte check、150 键契约、构建、前端 **223**、Rust **502 + ACL 2**、clippy `-D warnings` 全部通过。维护引用同步停机、锁顺序、可取消流和提交边界；没有新增可勾选的完整 Android/发布 TODO。前端已显示的提示仍无取消事件，手机 v1.6.5 的新 APK/选择器/云端 provider、真实剪贴板和第三方 XML 互操作仍待目标验证，整体目标保持进行中。

### Finding: SP-S07 旧审批结果覆盖新弹窗的操作状态（补充发现）

- Severity: Medium
- Confidence: High
- Category: Frontend State / Accessibility
- Status: Confirmed（源代码修复已验证）
- Affected area: Browser association prompt
- Evidence:
  - File: `src/lib/components/BridgeApprovalPrompt.svelte`（`b69b972` 修复前源码）
  - Function / Module: `decide` / association event / dismissal timer / mount cleanup
  - Relevant behavior: `bridge_approve` 返回后直接更新全局 outcome/busy 并定时清空 pending，没有核对返回结果属于当前弹窗。新事件能在旧请求处理中替换 pending；销毁时也没有使旧结果失效。
  - Reproduction: `tests/bridge-approval.test.mjs` 编译并执行完整生产组件 script，仅替换 Svelte state、event、IPC 和 timer 环境。旧 A 成功后 B 得到 `outcome=ok`，旧 A 失败后正在提交的 B 得到 `busy=false`，卸载后仍安装一个 timer；四项断言在修复前实际失败，见 `bridge-approval-race-red.log`。首次测试脚手架曾因 TypeScript 的空 export 标记不能在 VM 中执行而失败；该语法失败不是缺陷证据，修正脚手架后才记录状态断言。
- Problem: 新审批显示旧审批的授权/过期结果，操作状态提前释放，或被旧关闭回调删除。
- Why it matters: 可见客户端与反馈不一致，用户无法可靠确认自己正在处理哪个关联请求；这是 renderer 状态错误，未证明后台将 A 的 key 绑定到 B。
- Realistic failure scenario: 本地扩展短时间提出 A、B 两个关联请求，用户处理 A 时 B 到达；A 的异步响应晚到，B 被标为已授权或提前关闭。
- Minimal fix: 在 invoke 前捕获 request 对象，成功、失败、finally 与关闭回调只对同一当前对象生效；卸载清空 pending 和状态，取消 timer，并拒绝晚到事件。后端 token、会话和监听代次校验仍独立保留。
- Better long-term fix: 为等待/结束提示接入按请求身份的后台失效事件，并复核 SRP 提示的过期、锁定和连接结束清理。
- Regression test suggestion: 四条生产脚本回归覆盖旧成功、旧失败遇到新 busy、卸载晚到结果、失效 timer 回调。Edge 实际挂载组件并注入合成 IPC，点击 A→B 后让 A 失败，验证 B 按钮仍 disabled 且不显示 A 错误；B 完成后提出 C，经过旧 1.4 s deadline 后 C 仍可操作；卸载移除 modal，无 pageerror。1100/420 宽度截图已检查，见 `bridge-approval-render.log`。没有运行原生 Tauri/真实扩展或共享 OS clipboard。
- Estimated effort: 1–2 小时

`2303a0d` 已修复 SP-S07，仅调整 script 的请求所有权，未改 markup/CSS；data-contracts 已同步。最新完整 `npm run verify` 日志 `verify-bridge-approval-identity.log`：格式、Svelte check、150 键契约、生产构建、前端 **227**、Rust **502 + ACL 2**、clippy `-D warnings` 全部通过。报告当前详细卡片共 18 项，原始 15 项与 6.1 分基线保持不变。服务停止/锁定/超时的提示失效、SRP 秘密释放、剪贴板并发写入、SAF provider 同步及真实 Android APK/第三方 XML 互操作继续处理；完整语义覆盖仍未完成。

### Finding: SP-S08 RPC 缓存密钥越过会话授权失效边界（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Concurrency
- Status: Confirmed（源代码修复已验证）
- Affected area: KeePassRPC authorization lifetime, SRP proof and write admission
- Evidence:
  - File: `src-tauri/src/rpc/server/handshake.rs`、`rpc/server/mod.rs`、`vault/hosts.rs`（`936c8b0` 修复前源码）
  - Function / Module: `dispatch_setup` / `dispatch_decoded_jsonrpc` / `reply_jsonrpc_write` / `VaultSession::rpc_key`
  - Relevant behavior: JSON-RPC 仅用 `Conn.session_key` 解密并验 MAC，随后只检查 host 是否打开，没有查询会话中当前授权；`rpc_key` 的 lazy expiry 因而不会在既有连接请求时运行。写准备也没有重新验证。SRP proof 没有再次检查 vault open，stored-key proof 只验证此前缓存的 challenge secret，且 stage 1 未记录连接用户名。
  - Red regressions: 在真实合成 KDBX 会话中设密钥已过期，旧连接仍返回 `GetAllDatabases` 响应；真实会话 close 后，已准备好的有效 SRP proof 仍注册 key，回复未带 `AUTH_FAILED`；移除 FakeHost 的当前 key 后，旧 challenge proof 仍成功，未带 `AUTH_RESTART`。三个断言实际失败，见 `rpc-authorization-lifetime-red.log`。该复现调用生产状态机和真实合成 vault，未运行真实浏览器扩展或 Tauri app socket handler。
- Problem: 会话授权过期、撤销、替换或关闭不能约束缓存着旧密钥的连接；等待用户输入的证明也可能在锁库后创建新授权。
- Why it matters: `rpc.sessionTimeoutSecs` 和会话隔离不再是有效访问边界。当前未授权主机仍可能经 `FindLogins` 暴露凭据，或接收 `AddLogin`/`UpdateLogin` 写入。旧 socket 能验 MAC 不等于当前 vault 仍授权它。
- Realistic failure scenario: 曾获准的扩展保持连接，用户使授权过期或关闭原库、打开另一个库；扩展继续发送用旧 key 加密的请求。另一条路径是在用户锁库或切库前开始 SRP，迟到 proof 在新活动会话上完成。
- Minimal fix: 在同一 vault mutex 内，读 dispatch 和写 prepare 核对连接 username/32 字节 key 与当前 host key，触发 lazy expiry；写检查放在持久化许可之后，避免排队越过失效。stored-key proof 重查其 captured identity/key，失效要求 `AUTH_RESTART`；fresh SRP proof 要求 open host 与原 stable session id。临时 key 比较副本/hex 和 challenge 处理副本采用 Zeroizing。
- Better long-term fix: 为授权和提示统一记录明确的请求/会话身份，并补原生扩展的关闭、锁定、超时与重连端到端测试；继续复核已显示的 SRP 密码、半握手和单连接关闭清理。
- Regression test suggestion: 七项新增回归覆盖真实库过期、锁后 proof、撤销 challenge、真实 registry 切库/lock-reopen 的 SRP 身份、新会话正常 SRP、配置保留 key 的锁定错误/解锁重用/同名 rotation/普通 close-reopen、等持久化许可期间到期，以及真实 Add/Update 持久化。过期测试补强为 `FindLogins`：先成功读取合成密码，再将授权置为过期，旧 frame 被拒绝且 key set 清空。已获准写作业在 prepare 后撤销 key，仍完成/adopt 并重开看到更新，证明未破坏持久化提交边界。保留 key 模式在 locked host 上仍可完成 stored-key proof，但数据请求只返回 -32000 锁定错误。
- Estimated effort: 3–5 小时

`b785fef` 已修复 SP-S08。最终完整 `npm run verify` 日志 `verify-rpc-authorization-lifetime-final.log`：格式、Svelte check、150 键契约、生产构建、前端 **227**、Rust **509 + ACL 2**、clippy `-D warnings` 全部通过。此前全门禁也通过，随后补强真实合成凭据请求的测试并完整重跑；未将原先仅数据库元数据的 red 复现描述成真实用户密码泄漏。security-model、data-contracts、project-structure 已同步当前授权和锁定重连语义，并纠正相关旧模块路径、404/CORS 与 setup read timeout 的陈旧说明。当前详细卡片 19 项，原始 15 项/6.1 分基线保持不变；frontend 提示失效和秘密释放、剪贴板并发、SAF provider I/O、新 Android APK及第三方 XML 互操作仍未完成，目标继续进行。

### Finding: SP-S09 RPC 配对密码过期仍显示，旧复制与计时回调污染新提示（补充发现）

- Severity: Medium
- Confidence: High
- Category: Security / Frontend State
- Status: Confirmed（前端局部修复已验证；后端提示取消仍待处理）
- Affected area: RPC side-channel modal expiry and asynchronous ownership
- Evidence:
  - File: `src/lib/components/RpcSideChannelPrompt.svelte`（`a0da9f6` 修复前源码）
  - Function / Module: `startCountdown` / `copyPassword` / `onMount` cleanup
  - Relevant behavior: 倒计时按 interval 回调次数减一，归零只停止 timer，`pending.password` 仍显示；unmount 也只停 timer，未释放 pending。复制 Promise 完成后直接设置全局 `copied`，旧 timer 同样直接修改当前 remaining。
  - Red regressions: 执行完整生产组件 script，以隔离时钟、事件与剪贴板代替宿主环境。到期后 pending 未清、110 秒延迟只减一、timer 恢复前仍能复制、旧复制把 B 标为 copied、复制期间过期、旧 timer 改动 B、unmount 后旧事件/复制恢复状态、无效 TTL 显示、无 resume 检查共 9 项断言失败，见 `rpc-prompt-renderer-red.log`。这是合成密码和环境替身的前端证据，没有使用真实用户密码。
- Problem: 一次性配对密码超过显示期限仍可在屏幕上读取；后台定时器延迟可进一步延长显示，且旧异步操作能覆盖新提示的操作状态。
- Why it matters: 用户看到过期提示后仍暴露配对秘密，且新提示可能被误标记为已经复制，造成连接失败与误操作。后端 proof 的到期检查不能消除屏幕中的残留密码。
- Realistic failure scenario: 用户打开配对提示后切换窗口或使设备休眠，恢复时 timer 尚未执行；密码继续显示。另一条路径是复制 A 后立即接到 B，A 的迟到复制完成让 B 的复制按钮被禁用。
- Minimal fix: 使用墙钟 deadline，interval、focus/visibility resume、复制开始与完成都重新检查；过期关闭弹窗并释放 pending，关闭/unmount 同样清理。timer/copy 回调只在仍持有相同 request object 时更新状态，卸载后的队列事件不再接收。
- Better long-term fix: 为每次后端 SRP attempt 引入关联身份及关闭事件，覆盖 proof、连接退出、锁定、切库与服务停用。当前 TTL 从 renderer 收到事件起算，未处理事件投递延迟；释放 JS 引用不等于证明堆内存被 zeroize。
- Regression test suggestion: 9 项生产 script 回归修复后全部通过。真实 Edge 挂载当前 Svelte 组件，验证 A copy 完成不禁用 B、到期后 DOM 不含密码/弹窗、延迟 visibility resume、手动关闭和复制中 unmount；剪贴板与 IPC 均为替身。深色/浅色/自定义主题的 1100px/420px 截图已检查，弹窗未溢出且保持既有紧凑语义 token。
- Estimated effort: 1–2 小时

`22cd55d` 已修复 SP-S09 的前端范围。完整 `npm run verify` 日志 `verify-rpc-prompt-renderer.log`：格式、Svelte check 零错误警告、150 键契约、构建、前端 **236**、Rust **509 + ACL 2**、clippy 全部通过。渲染日志 `rpc-prompt-renderer-render.log` 与六张截图保存在本任务 artifacts 目录；未触碰真实库或 OS 剪贴板。security-model 和 data-contracts 同步了当前保证与未完成的取消边界，未新增完整生命周期/Android TODO 勾选。当前详细卡片 20 项；原始基线保持不变。后端 SRP 提示结束事件及零化失败路径、剪贴板并发、SAF provider、新 APK/设备及第三方 XML 互操作继续审查，整体目标未完成。

`65e47cd` 进一步补上同一问题的后端结束通知：每次配对拥有进程内递增的字符串 `requestId`，连接持有仅包含身份与事件回调的 `PromptLease`。后续 setup（proof 或替换）释放旧 lease；连接正常退出或 unwind 时同样释放，发送 `rpc-side-channel-ended { requestId }`。前端只关闭匹配提示，同时记录最新非秘密身份，拒绝旧/重复 show 和 end-before-show。旧源码 `22cd55d` 对新增前端验证 **5 失败 / 10 通过**，当前 **15 全通过**，见 `rpc-prompt-end-red.log`；没有把测试夹具的编译错误计为行为复现。三项后端新增回归覆盖连接退出/unwind、真实合成 KDBX proof 成功及 locked proof 失败的通知、替换 setup 在新密码 show 之前结束旧提示。此前停机机制会让 SRP worker 退出并释放 lease，但这轮没有真实 Tauri/扩展停机弹窗端到端证据。

配对临时密码现用 `Zeroizing<String>`，只在 SRP pending 已建立后发出 show；begin 返回错误或 callback unwind 也受作用域析构保护。事件载荷借用密码，避免额外的 owned payload 克隆，并去掉 secret-bearing payload 的 Debug 派生。已核对本地 Tauri 2.11.5 的 `Emitter::emit` → manager `emit` → `EmitArgs::new` 同步序列化路径；这属于代码保证，没有测量回收后的内存。最新完整 `npm run verify` 日志 `verify-rpc-prompt-lifecycle.log`：格式、Svelte check、150 键契约、构建、前端 **242**、Rust **512 + ACL 2**、clippy 全通过。渲染日志 `rpc-prompt-lifecycle-render.log` 验证关联 end、旧 end、新旧 show 乱序、复制、过期/恢复/关闭/unmount，仍为合成 IPC/剪贴板。三份维护引用同步协议与所有权；详细卡片仍为 20 张。**空闲配对提示的锁定/切库即时取消、事件投递延迟、JS 堆零化、真实 Android 新 APK/provider 和第三方 XML 互操作仍未闭环**，目标继续进行。

`8e050d8` 补齐 SP-S09 的空闲提示取消，同时收紧 SP-S08 的 SRP 返回原标签边界。`VaultSessions` 在成功 open/create adoption、关闭 active、close-all lock 和实际 switch 后集中结束 active lifetime；本地/远程四条打开/创建路径均调用该注册表，无需逐个 command 补 hook。`vault/session_end.rs` 只持有弱注册的一次性回调；RPC 在 active mutex 内注册，状态交换后先释放 registry/observer guards，再发送已有的关联 end。失败/idempotent 变化及关闭 parked 标签不影响当前提示，重复结束/连接 drop 只通知一次，未新增线程或秘密存储。

真实合成 KDBX 的修复前回归确认四种拓扑变化均无结束通知；A→B→A 后旧 proof 仍返回成功。干净 baseline 探针运行 `fd8a08e` 的生产模块，只将新测试的 watcher 创建适配为原 `PromptLease::new`，运行后逐字恢复当前文件；两项断言失败见 `rpc-idle-baseline-clean-red.log`。初次 red 夹具在断言持有观测 mutex 时触发析构回调二次 panic；已纠正为先 clone 观测、drop 连接后集中断言，不把夹具 abort 计作产品 bug。首次 baseline 适配遗漏多行调用引发的编译错误也不计作行为证据。

取消信号也限制后端 proof：已结束的 attempt 即使回到同一 stable sessionId 仍失败。复查修复中又用生产状态机确认“取消后先发 stored-key setup，再发旧 proof”会丢掉 lease 标记却保留 SRP 状态，见 `rpc-cancelled-interposed-setup-red.log`；因此任何收到取消的后续 setup 都先清空整份 pending SRP/a/expiry/session，再处理新的握手。六项新增回归覆盖四种真实库转移、原标签回访、插入 setup、失败/parked/idempotent 不误取消、通知在 registry guard 外且 callback panic 不损坏 active mutex、1000 次观察者释放无 owned callback 累积。最终完整 `npm run verify` 日志 `verify-rpc-active-lifetime-final.log`：格式、Svelte check、150 键契约、构建、前端 **242**、Rust **518 + ACL 2**、clippy 全通过；三份维护引用已同步。无 markup/CSS 变化；没有真实 Tauri/扩展的锁定弹窗端到端证据。详细卡片仍为 20 张、原始基线不变；APK/provider、JavaScript 堆/延迟事件、剪贴板并发与原生互操作等剩余范围继续处理。

提供商复核新增来源：[AOSP ContentResolver](https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/master/core/java/android/content/ContentResolver.java) 明确允许 exclusive read/write 返回 pipe/socket，`rw` 才要求可 seek 文件；本地 `tauri-plugin-fs 2.5.2` 的 Android Kotlin 用 `openAssetFileDescriptor`，Rust 直接将 detach 的 FD 转为 `File`，SecPivot 写模式为 `wt` 并无条件 `sync_all()`。这是非普通 FD 同步错误的源码/平台契约线索，尚未提升为 Android 实测结论。WSL 名称列表虽有 Ubuntu，但启动报 `ERROR_PATH_NOT_FOUND`（缺失 ext4.vhdx），Linux pipe/fsync 试验未运行，不将列表当作可用运行环境。官方 [1PUX 格式说明](https://support.1password.com/1pux-format/) 已成功取得，明确是未加密 ZIP，包含 `export.attributes`、`export.data` 与附件目录；TODO 中“ZIP/加密导出”的描述须纠正，并继续复核现有 legacy 1PIF 的真实兼容性。

### Finding: SP-S10 1PIF 导入解析自造块格式，真实 JSON-line 导出返回零条（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Testing Authenticity
- Status: Confirmed（基础 JSON-line 导入已修复；完整迁移仍部分）
- Affected area: 1Password legacy import, format compatibility and completion evidence
- Evidence:
  - File: `src-tauri/src/vault/import.rs:148`、`src-tauri/src/commands/entries.rs:496`、`src/routes/+page.svelte:1184`（修复前 `8e050d8` 源码）
  - Function / Module: `parse_1pif` / `finish_1pif_item` / `handleImportOnePassword`
  - Relevant behavior: parser 忽略不以 `***` 开头的行，只处理 `***Key:value`；其唯一相关测试同样编造此格式。真实 1PIF 使用 JSON-line，每条记录包含 `typeName`、`secureContents` 等。生产 command 把空 Vec 当作解析成功，页面随后显示“没有可导入的条目”。
  - Independent reference: [Bitwarden 1PIF importer](https://github.com/bitwarden/clients/blob/1a8bdbfe625bff90a9c0cabef2002e37f6038fa1/libs/importer/src/importers/onepassword/onepassword-1pif-importer.ts) 按行 `JSON.parse`，支持 standard `secureContents` 与 Windows `hmac/overview/details` 变体；对应公开 spec-data 也为 JSON-line。先前猜测文件 URL 的 404 和 GitHub API 限流不属于兼容性证据，最终以只读 shallow/no-checkout 参考 clone 的确定路径与 commit 核对。
  - Reproduction: 精确抽取修复前生产 parser 和 helper，仅移除 DTO serde 装饰，用独立 Rust probe 输入按上述真实 schema 构造的两个合成登录/笔记。函数返回 **0，预期 2**，断言实际失败，见 `onepassword-1pif-baseline-red.log`。未运行 1Password 原生导出或真实用户文件，没有把公开样本密码输出到报告。
- Problem: 菜单及 TODO 声称的 legacy 1PIF 能力没有真实格式兼容性；用户正常导出后无法迁移，而现有测试可以保持全绿。
- Why it matters: 迁移入口无法完成核心工作流，用户可能误以为空库或导出文件无条目。测试只验证自造格式，不能作为竞品互操作及功能完成证据。
- Realistic failure scenario: 用户从旧版 1Password 导出 JSON-line 1PIF，选择导入；全部记录被忽略，页面报没有条目，登录凭据和笔记未迁入。
- Minimal fix: 解析真实逐行 JSON standard/Windows 变体，严格处理坏记录并保持字段 designation、notes、URL、TOTP、分组与受保护字段语义；错误不能静默归零。更新 IPC 结果/错误、前端映射及真实 schema 合成回归；不要继续用 `***Key:value` 作为真实性测试。
- Better long-term fix: 独立支持现代未加密 1PUX ZIP，做有界解压、账号/库/附件与各类字段迁移，采用官方 schema 和多个客户端样本复核。迁移全过程保留本地秘密边界，不为导入而发送凭据到第三方。
- Regression test suggestion: JSON-line 登录/笔记、多条与 CRLF、损坏后续记录不得部分成功、standard 与 Windows variant、隐藏字段/TOTP/多 URL、分组/未知类型/回收状态；通过生产 import command 和合成 KDBX 保存重开复核最终字段。现代 1PUX 单独验证，不以 1PIF 修复代替。
- Estimated effort: 2–4 小时（1PIF）；1PUX 另行估算

TODO 的原“1Password/LastPass 已完成”已降为 partial，明确 LastPass 别名仍接线、1PIF 真实导出失败；1PUX 单列 pending 并纠正其未加密 ZIP 定义。data-contracts 和 project-structure 同步了真实缺口，没有在源码修复前宣称完成。当前详细卡片 **21**（全部 Confirmed，High 5 / Medium 13 / Low 3）；原始 15 项/6.1 分基线保持不变。最新应用完整门禁仍对应 `8e050d8`（前端242、Rust518、ACL2），它未覆盖真实 1PIF 格式，所以不抵消本项失败证据。报告 lint/严格 JSON/格式门另行验证本次文档，后续继续实现兼容修复及剩余审计。

`8c09399` 修复 SP-S10 的基础 JSON-line 导入。生产实现已替换为
`vault/import/onepassword.rs`，支持 standard `secureContents` 与 Windows
`overview/details` 变体、BOM/CRLF/UUID 分隔符、回收状态排除、登录/笔记、OTP
及去重后保留的所有网址。坏记录或不支持的分组/附件使整份解析失败，错误只含
行号；8 MiB 上限也应用于直接 IPC。隐藏/密码类型/guarded 字段的保护标记通过
统一 DTO 与实际前端 bulk import 保留，隐藏用户名不会降为普通 UserName。
所有自定义字段加来源前缀与唯一后缀，避免 reserved/OTP alias 与同名值覆盖；
复杂字段保留 JSON，未映射 secure contents 与 passwordHistory 保存为受保护的
Additional Data 字段。它们仍不是原生历史、专门身份类型、分组或附件迁移。
命令输入使用 Zeroizing，秘密 DTO 移除 Debug；没有证明 JS 堆可零化。

回归证据：`onepassword_tests.rs` 五项真实 schema 合成测试覆盖标准/Windows/
笔记、损坏后续记录不得部分成功、敏感错误文本不泄漏、隐藏/guarded/复杂字段、
同名与 reserved 字段、隐藏用户名，以及 serde → EntryInput → batch insert →
KDBX 保存重开的凭据、OTP、多 URL 和隐藏快照。`tests/io-import.test.mjs`
运行实际服务模块图与 importEntries，仅替换 Tauri invoke 边界，验证保护标记和
OTP 穿过真实 `import_entries` 调用；其初次失败来自测试误把命令写成 add_entries，
经源码核对修正为真实 import_entries，不作为产品缺陷或 red-to-green 证据。
另直接输入确定 commit 的 Bitwarden TestData、WindowsOpVaultTestData 和
IdentityTestData 三条公开合成样本，验证用户名/密码/隐藏字段/身份数据保留，
见 `onepassword-public-reference-green.log` 与 SHA-256 provenance；临时 probe
测试已 byte-for-byte 恢复，未将竞品测试文本复制进仓库，也未输出样本秘密。
这仍不是原生 1Password 实际导出或运行时互操作验证。

最新完整 `npm run verify` 日志 `verify-onepassword-json-line-final.log`：格式、
Svelte check 零错误警告、150 键契约、生产构建、前端 **243**、Rust **522 + ACL 2**、
clippy `-D warnings` 全通过。维护文档 currency gate 更新 data-contracts、
security-model 与 project-structure；TODO 继续 partial，完整导入未被勾选。
当前详细卡片保持 **21**，原始 15 项与 6.1 分基线不变。手机 v1.6.5 另存为
仍只有 ACL 源码修复证据，无新 APK 或真机选择器验证；SAF provider、剪贴板并发、
原生 XML、现代 1PUX 与其余完整语义/UI 覆盖仍须继续处理，整体目标保持进行中。

`79f48c8` 补齐 SP-11 的单 renderer 写入顺序。修复前生产 clipboard.ts 在
copyRaw 完成后才检查代次；用真实模块图与两个可控 Clipboard API Promise
复现：A 写入未完成 → 锁定清空 → A 完成，会恢复合成秘密且不安装清理 timer；
A 未完成 → B 完成 → A 完成，会让旧 A 覆盖新 B。两项断言真实失败，见
`clipboard-renderer-pending-write-red.log`。仅替换平台 API，没有用户 vault 或
共享 OS 剪贴板操作，结论为受支持异步完成时序的代码缺陷，非真机测量。

修复增加同一 renderer 的文本写入/条件 native clear/显式 clear Promise 队列。
空闲写入仍同步开始，保留 consumer guard 与实际写入之间的调用边界；新操作
同步释放旧 ownership，旧排队复制被代次取消，已开始的写入完成后才显式清空。
Browser read/hash 等待不占队列，其最后 clear 重新验证 ownership；拒绝的写入
不损坏队列。初版队列复核还实际失败于 lockVault 原先 await clear 后才 closeAll
的顺序：等待平台写入会阻塞会话关闭，见 `clipboard-lock-close-pending-red.log`。
这属于修复中发现并纠正的边界，未新增为基线产品卡片。最终 lockVault 同步发起
ownership 失效后，并行启动 session close 与 clipboard clear；即使写入仍未完成，
会话关闭已执行，迟到的写入随后仍清空。lockAfterAction 的保留剪贴板选择保持。

`clipboard-service.test.mjs` 现在 14 项，其中新增六项覆盖待写/锁定、旧写覆盖新写、
取消排队复制与提前会话关闭、拒绝后恢复、旧 native wipe 与新相同文本复制的
顺序，以及空闲复制同步开始。最终完整 `npm run verify` 日志
`verify-clipboard-renderer-write-order-final.log`：格式、Svelte check、150 键契约、
构建、前端 **249**、Rust **522 + ACL 2**、clippy 全通过。维护文档 currency gate
同步 security-model、data-contracts、project-structure 与 partial TODO。
本队列没有协调另一 webview、原生 binary-copy command 或外部程序的 clipboard
sequence；这些并发/ownership 边界仍待验证，SP-11 继续 partial，整体目标进行中。
当前详细卡片与原始基线统计不变；无 markup/CSS 变化，也无新 APK/设备验证。

### Finding: SP-S11 迟到或被取消的密码复制仍锁定后来的数据库（补充发现）

- Severity: Medium
- Confidence: High
- Category: Frontend State / Stability / Data Integrity
- Status: Confirmed（当前 renderer 路径已修复；原生多窗口归属待验证）
- Affected area: sensitive copy completion and lock-after-action topology
- Evidence:
  - File: `src/lib/services/security.ts:42`、`src/lib/utils/clipboard.ts:56`、`src/lib/services/vault.ts:817`（`79f48c8`）
  - Function / Module: `copySensitive` / `copyText` / `VaultStore.closeAll`
  - Relevant behavior: copySensitive 在 await copyText 后仅检查当前 lockAfterAction 设置，然后关闭所有会话；不检查复制是否被代次取消或原来的可见会话是否仍连续有效。closeAll 排队后也没有完成归属检查。
  - Reproduction: `tests/security-copy.test.mjs` 运行实际 security/clipboard/vault 模块图，仅替换 Clipboard API 与 Tauri invoke 边界。四个断言在生产实现上失败：复制未完成时打开 B；A→B→A；排队密码复制被 clear 取消；B open 先进入 topology 队列而旧复制 lock 随后进入。当前会话均被 closeAll 清掉，见 `security-copy-owner-red.log`。正常当前复制的对照测试通过。全部数据为合成，未访问用户库或系统剪贴板。
- Problem: 旧复制完成会被解释为对当前所有会话的锁定授权，即使该复制根本没有写入剪贴板，或用户已经进入另一个数据库。
- Why it matters: 锁定关闭所有后端会话，后来的数据库工作流会突然中断；未保存编辑存在丢失风险。本轮复现验证了前端发送全库关闭及缓存清空，不声称真实用户数据丢失。
- Realistic failure scenario: 开启复制后锁定，复制 A 密码时平台写入延迟；用户先打开 B 或切回原 A，旧复制完成后全部标签被锁定。另一场景是取消排队复制后仍被误锁。
- Minimal fix: 只让真正完成且仍属于原可见会话的复制触发 follow-up；以连续 view epoch 防止 A→B→A，锁定入队后在 topology 执行点再次检查，失败/取消不得触发锁定。
- Better long-term fix: 将所有跨 await 的全局 follow-up 绑定到操作身份，并继续验证多窗口/后端活动会话变化的授权边界。
- Regression test suggestion: 延迟写入后的打开/切换/原标签回访、复制取消、先入队 open、正常当前复制及失败路径；验证真正调用的关闭命令、会话状态与剪贴板保留语义，后续补原生平台运行证据。
- Estimated effort: 1–2 小时

本项单独计入当前卡片统计；不把 SP-11 的单 renderer 写入顺序修复描述为已解决
全部 follow-up 归属。当前卡片 **22**（High 5 / Medium 14 / Low 3，全部 Confirmed）；
原始 15 项与 6.1 分审计基线不变。修复及完整验证继续进行。

`a02c759` 修复 SP-S11 当前 renderer 的完成归属。copyText 的可选 afterCopy
hook 仅在写入成功且原复制代次仍 current 时执行，保留 Promise<void> API；零秒
清理与 renderer digest 不可用仍支持正常 copy-follow-up。copySensitive 用临时
active-id/state 观察者和 KeyedViewGuard 绑定连续 session-id + path 视图，并核对
内部 active id 与当前设置；A→B→A、关闭/重开、browser 固定 id 路径变化都取消
旧归属。观察者持续到 queued close 的执行点，随后 success/failure 均释放。
closeAll 的可选非秘密 predicate 在既有 topology 队列内、调用后台之前再次验证；
browser 分支在附件清理的 await 后也重查。手动/闲置/焦点锁定无 predicate，保持
原来的全库锁定语义，close_all_vaults 的后台 IPC schema 未变。

八项新增 actual-service 回归全部通过：前述四条 baseline 失败路径、当前复制
锁定且保留值的对照、browser 固定 id 的路径变化、拒绝写入不锁且观察者释放、
无 renderer digest 时仍能正常锁定。初版仅按 active id 观察时，browser 新路径
回归实际失败，见 `security-copy-browser-path-intermediate-red.log`；补上 path
生命周期后通过。这属于修复过程中补强边界，不另列基线产品卡片。测试替换
Clipboard API 和 Tauri invoke，未使用真实用户库或共享 OS 剪贴板；不代表原生
Android/macOS/WebView 多窗口端到端验证。

最新完整 npm run verify 日志 `verify-sensitive-copy-owner-final.log`：格式、
Svelte check 零错误警告、150 键契约、构建、前端 **257**、Rust **522 + ACL 2**、
clippy 全通过。此前一次 npm run check 在 Svelte 启动阶段报 Node Zone Allocation
内存分配失败；未改变源码或检查参数的再次检查和最终全门均通过，未将其归因
为应用缺陷，也没有借缩小检查范围掩盖失败。维护文档 currency gate 已同步三份
引用，TODO 保留 native 多窗口 pending。报告/metadata 当前卡片 **22**，原始
15 项/6.1 分基线保持不变；手机另存为新 APK 与设备、SAF provider、跨 renderer
剪贴板、竞品导入与完整 UI/语义覆盖继续推进。

### Finding: SP-S12 文本导入未接 Android 文档读取，过期选择仍读取明文（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Frontend State / Privacy
- Status: Confirmed（主机源码/服务回归通过；Android provider 运行待验证）
- Affected area: native text-import picker and URI read boundary
- Evidence:
  - File: `src/lib/services/io.ts:174`、`src/lib/services/saf.ts:16`、`src-tauri/src/commands/entries.rs:465`（`825d15b` 修复前源码）
  - Function / Module: `pickImportFile` / `openDocument` / `read_text_file`
  - Relevant behavior: 所有 Tauri 平台都调用 dialog open；`read_text_file` 从路径扩展名过滤后直接调用 std::fs，无法处理 Android 的 content URI。已有 vault SAF 服务只暴露 KDBX MIME 默认集，文本导入未调用它。选择返回后立即读文件，outer awaitCurrentView 直到读取结束才拒绝过期结果。
  - Reproduction: `tests/io-picker.test.mjs` 运行生产 io/settings/saf 模块，仅替换 navigator 与 Tauri invoke。修复前六项中三项失败：Android 首次调用为 `plugin:dialog|open`；Android、Windows 的 A→B→A 旧选择均启动 `read_text_file`。取消、桌面正常选择、当前错误提示对照通过，见 `io-picker-red.log`。URI/文件名适配缺口由前后端控制流直接确认，未将主机替身当成 Android 真机故障复现。
- Problem: Android 文本导入缺少可用文档读取链路；已结束的可见会话仍能因迟到选择而把外部明文读入 renderer，尽管后续数据库写入已被守卫阻止。
- Why it matters: 手机用户无法按现有菜单完成 CSV/XML/Bitwarden JSON/1PIF 导入；锁库或切库后不必要的明文读取延长敏感数据生命周期。未观察用户凭据泄漏、原生选择器异常或数据库误写，不提高为 High。
- Realistic failure scenario: 手机选择云端 provider 的导出文件；返回的是无扩展名的 content URI，旧后端按本地路径处理失败。另一场景是桌面/手机打开导入选择器后切换或关闭数据库，返回旧选择仍读取明文；回到同一 tab id 也无法恢复原视图授权。
- Minimal fix: Android 调用 SAF 并传文本格式 MIME，携带 provider DISPLAY_NAME 到读命令；local path 仍按真实扩展名过滤，URI 用显示名过滤。后端走统一有界 reader，严格 UTF-8；原生 picker 返回后先检查原视图，读完成后继续使用既有 completion guard。
- Better long-term fix: 在真实 APK 上验证云端/本地 provider 的显示名、读取、取消和授权撤销；随后补齐附件及明文导出的 SAF 写入。格式过滤不能替代文件访问授权，不为本项新增广泛路径权限。
- Regression test suggestion: 六项实际服务测试覆盖四种格式的 MIME/URI/显示名、取消、两平台 A→B→A 无读取、桌面对话框过滤和当前错误提示。后端验证 URI 名称缺失/不支持、local path 不能由 fileName 覆盖、8 MiB 上限、非法 UTF-8 不回显内容；共享 reader 的真实临时文件在 metadata 成功后增长仍被拒绝。旧 read_text_file 的并发增长未单独复现，不另计 Confirmed 漏洞。
- Estimated effort: 1–2 小时（源码与主机验证）；真机复测另行安排

`288b224` 已提交 SP-S12 的文本导入修正。`saf.openDocument(mimeTypes?)` 默认
仍用于 KDBX，文本导入按格式传 MIME 并兼容 text/plain/octet-stream，XML 同时
允许 application/xml 与 text/xml。`read_text_file({path,fileName?})` 对 URI
使用 provider 显示名，对本地路径只认可真实扩展名；缺名/不支持格式在读前失败。
8 MiB ceiling 由共享 file_io reader 在流式读取中执行，UTF-8 错误只含通用提示。
原生选择结果先检查发起 view epoch，避免 A→B→A 后读取旧选文件；读取完成
仍由既有 guard 拒绝过期结果。导入文件不写入最近数据库记录。未改 Kotlin、
markup 或 CSS，未以本次结果扩大界面/原生平台覆盖声明。

完整 `npm run verify` 通过，日志 `verify-text-import-saf.log`：格式、Svelte
check、150 键契约、构建、前端 **263**、Rust **525 + ACL 2**、clippy 均通过。
提交前格式检查也通过；documentation currency gate 同步 data-contracts、
security-model 与 docs/android.md。当前详细卡片 **23**（High 5 / Medium 15 /
Low 3，全部 Confirmed），原始 15 项/6.1 分不变。用户手机 v1.6.5 的另存为
仍需包含 ACL 修复的新 APK 和真机选择器/provider 验证；本次不标记已解决。
CSV/XML/HTML 导出与附件保存的 std::fs/桌面对话框链路仍待补齐；native XML、
1PUX、跨 renderer 剪贴板、完整语义/界面覆盖继续推进，整体目标保持 active。

### Finding: SP-S13 原子写入共用临时文件，正常并发保存大量失败（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Data Integrity / Concurrency
- Status: Confirmed（Windows 合成文件并发先红后绿；其他文件系统待验证）
- Affected area: shared local atomic writer
- Evidence:
  - File: `src-tauri/src/util.rs:31`（`0de33c6` 修复前源码）
  - Function / Module: `atomic_write` / `temp_sibling`
  - Relevant behavior: 每次写入共用目标的固定 `.tmp`；遇到 AlreadyExists 删除该文件后重新创建，写入后再次按路径打开进行同步。一个操作可删除或替换另一个操作持有的临时文件。
  - Reproduction: `atomic-parallel-probe.rs` 直接编译原 util 模块，对合成目标进行 120 轮、每轮三个 128 KiB 并发写入；360 次尝试中 353 次失败，见 `atomic-parallel-red.log`。未观察到混合/不完整目标内容，没有使用用户文件。
- Problem: 临时文件缺少操作归属；并发写入因删除、共享及重命名冲突而失败，且最终同步的路径未必仍对应原写入句柄。
- Why it matters: 配置等共享持久化用户能遇到正常保存失败；原子替换 helper 本身不能为并发调用提供可靠隔离。没有观察数据丢失或实际泄漏，不将潜在后果写成已发生事故。
- Realistic failure scenario: 两个调用同时写入同一合成配置目标，一个正在写临时文件，另一个遇到临时文件已存在并试图删除它；Windows 上大量返回保存错误。
- Minimal fix: 每次写入生成随机 128 位 nonce 的同目录临时文件，create_new 作为最终归属检查；始终写入并同步最初持有的句柄，仅清理本操作拥有的临时文件，Unix 权限也通过同一句柄设置。
- Better long-term fix: 保持上层保存准入和 revision 顺序；原子文件替换不决定哪次业务修订应最后提交。补其他文件系统的并发、失败与权限验证。
- Regression test suggestion: 同一目标 60 次并发真实写入全部成功，最终是一个完整载荷，目录无临时残留；预置 legacy `.tmp` 内容不得被删除或改写；失败路径保留目标。
- Estimated effort: 1–2 小时（源码与主机复核）；其他系统验证另计

`a890eb1` 已修复 SP-S13。完整门禁日志 `verify-atomic-siblings.log`：前端
**263**、Rust **527 + ACL 2**、格式、Svelte check、150 键契约、构建和 clippy
全通过。security-model 和 PITFALLS 同步临时文件归属与平台限制。一次针对性
Rust 编译因 LLVM 内存不足失败，同源码/同命令重试及完整门禁通过；未归因为
应用内存耗尽，也未终止用户应用。并发证据不证明用户数据损坏。

### Finding: SP-S14 移动端明文导出与附件保存缺少 SAF 选择和 URI 写入（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Backend API / Data Integrity
- Status: Confirmed（生产 handler/服务与主机写入回归通过；手机/provider 待验证）
- Affected area: CSV/XML/emergency HTML exports and both attachment-save entry points
- Evidence:
  - File: `src/routes/+page.svelte:874`、`src/lib/components/EntryDetail.svelte:464`、`src/lib/components/AttachmentPreviewDialog.svelte:105`、`src-tauri/src/vault/persist.rs:496`（`a890eb1` 修复前源码）
  - Function / Module: `handleExportCsv` / `handleExportXml` / `confirmExportEmergency` / `saveAttachment` / `saveToDisk` / export writers
  - Relevant behavior: 五条 Tauri 保存入口均直接调用桌面 dialog save；后端明文/附件 writer 直接 std::fs::write，无法处理 Android content URI。数据库另存为已有独立 SAF adapter，导出未接入。
  - Reproduction: `tests/io-export.test.mjs` 提取并执行五个原生产 handler，使用真实 io/saf/vault 服务，以 navigator、invoke 和 UI globals 为合成边界。修复前 5 项 SAF 入口断言失败，5 项既有过期视图保护通过，见 `mobile-export-picker-red.log`；不把最初 harness 缺少 detailSessionId 的错误算作产品发现。
- Problem: 手机无法通过这些菜单完成原生导出/附件保存；仅修复数据库另存为的 ACL 仍不补齐这些独立保存路径。
- Why it matters: 常用备份、应急单和附件提取工作流有明确平台接线缺口。源码证据和模拟 Android 不能证明特定手机文件提供商的运行结果。
- Realistic failure scenario: 手机从 CSV/XML/应急单导出或两处附件菜单点击保存，旧入口调用桌面对话框；即便提供 content URI，旧 writer 也把它当本地文件路径。
- Minimal fix: `io.pickSaveFile` 在 Android 按导出 MIME 调用 SAF create，desktop 保留 save options；五个 handler 保留原 view guard 和会话身份。统一 named writer 复用本地原子替换或 URI 写后校验/已验证回滚，命令明文与回滚副本在返回时零化。
- Better long-term fix: 真机验证 local/cloud provider 的选择、取消、覆盖、readback、授权撤销和错误恢复；检查 pipe descriptor 的同步语义。当前仍沿用既有 URI 事务，不宣称 SAF 原子覆盖。
- Regression test suggestion: 23 项前端回归覆盖五条真实 handler 的 SAF MIME/URI/session、取消、A→B→A、desktop options，CSV/XML 拒绝明文确认、应急密码 opt-in 快照和 KDBX 默认过滤；三项 Rust 回归覆盖 BOM/CRLF/Unicode/二进制字节、失败目标保留/无临时残留及 unreadable/partial-write URI 故障回滚。
- Estimated effort: 2–3 小时（源码、跨层回归和主机门禁）；真机复测另计

`7a09b17` 已提交 SP-S14，未改 Kotlin、markup 或 CSS。导出 URI 不写最近数据库；
临时 payload、旧内容及 readback buffer 使用 Zeroizing，失败的 capped read 也
零化。完整门禁日志 `verify-mobile-export-saf.log`：格式、Svelte check、150 键
契约、构建、前端 **286**、Rust **530 + ACL 2**、clippy 全通过；最后文档修改
后的格式检查也通过。documentation currency gate 同步 data-contracts、
security-model、project-structure 和 docs/android.md，纠正陈旧 1PIF 格式说明。

当前详细卡片 **25**（High 5 / Medium 17 / Low 3，全部 Confirmed），原始
15 项/6.1 分不变。用户手机安装 **v1.6.5**，报告的
`plugin:safe|create_document not allowed by ACL` 与源码 `plugin:saf` 的拼写
差异仍未独立确认；`e5e3c4e` 的 windows ACL 修复已在主机验证，但本任务没有
构建/安装/发布替代 APK，没有手机选择器或云端 provider 运行证据，手机另存为
不标记已解决。provider pipe/sync、native XML、现代 1PUX、跨 renderer 剪贴板、
完整语义覆盖与渲染 UI 审查仍在推进，整体目标保持 active。

### Finding: SP-S15 短窗口菜单裁切且不随视口缩放重新定位（补充发现）

- Severity: Low
- Confidence: High
- Category: Accessibility / Frontend State / Stability
- Status: Confirmed（实际 Edge 渲染先红后绿；Android WebView 待验证）
- Affected area: shared viewport menu and context-menu cascade positioning
- Evidence:
  - File: `src/lib/components/ViewportMenuShell.svelte:24`、`src/lib/components/ContextMenu.svelte:54`（`2e36ade` 修复前源码）
  - Function / Module: menu positioning effect / `submenuFlip`
  - Relevant behavior: 主菜单只有右/下侧坐标限制，无通用高度或滚动限制；视口变化不是 reactive dependency，打开后缩小窗口不会重新定位。级联菜单过高时把 top 调为负数，仍不能显示全部内容。
  - Reproduction: `menu-viewport-render.cjs` 在 headless Edge 挂载真实 ContextMenu 和生产 buildToolbarMenuItems。420×320 下 12 项菜单高 358px、top 0、bottom 358，overflowY 为 visible，设置 scrollTop 仍为 0；“设置”在截图底部之外。打开时从 1100×720 缩至 320×240，菜单仍留在 x 932/y 354。24 项合成子菜单 top -394、高 706px，开头不可见。见 `menu-viewport-red.log`、`menu-viewport-assert-red.log` 与 before 截图。
- Problem: 手机横屏或短桌面窗口的末端操作无法直接点到，缩放后整个菜单可能停在视口之外；长子菜单也缺少有界滚动。
- Why it matters: 影响“设置”等辅助入口的可达性和界面完整度。范围集中于短/窄视口，未证明秘密泄漏、数据破坏或主流程全面不可用，按 Low 记录。
- Realistic failure scenario: 把次要工具收进更多菜单，在 420×320 手机横屏打开；底部设置项被裁切且滑动不滚动。桌面菜单保持打开时缩小窗口，旧坐标使操作离开可见区域。
- Minimal fix: 主菜单限制视口宽高、保持 8px 边距、内部滚动；ResizeObserver 与窗口 resize 更新位置且卸载清理。级联改用 fixed 逃离父菜单滚动裁切，自身有界滚动并随父滚动/视口变化定位；保持现有主题 token、间距和图标。
- Better long-term fix: 继续验证 Android WebView、软键盘/visual viewport、不同字体缩放与键盘菜单导航；真实手机触摸可达性不能由 Edge 鼠标回归替代。
- Regression test suggestion: 实际挂载生产组件检查 1100/720/420 宽度的 dark/light/custom 九组视口；短菜单滚到末端并点击“设置”，长子菜单滚到 child 23 并派发；同一实例缩放、负 anchor、Escape、卸载后事件。通过真实页面 welcome 解锁合成库，从工具栏更多菜单滚到“设置”并打开 settings shell。后续补 column-config 和真机软键盘路径。
- Estimated effort: 1–2 小时（局部修复及渲染回归）；真机检查另计

`c42ea09` 已修复 SP-S15。`menu-viewport-green.log` 与 after 截图记录实际组件
及主页面点击路径成功，pageerrors 为空；短窗口菜单为 y 8/高 304/bottom 312，
scrollTop 可到 54。九组主题/宽度截图已检查，紧凑表面与按钮样式保持。
夹具初期使用不同 Svelte runtime、错误的本地化按钮名和 settings API，以及
固定 close 次数断言，均在记录最终回归前修正；不计为产品发现。resize 后先等
浏览器帧布局再断言，避免把观察回调尚未交付当作产品失败。

完整 `npm run verify` 日志 `verify-viewport-menus.log`：格式、Svelte check、
150 键契约、构建、前端 **286**、Rust **530 + ACL 2**、clippy 全通过。
documentation currency gate 更新 css-theming 和 project-structure，没有增加
未验证 TODO 勾选。当前详细卡片 **26**（High 5 / Medium 17 / Low 4，全部
Confirmed）；基线统计与 6.1 分保持不变。SAF 真机、新 APK、原生 XML、现代
1PUX、跨窗口剪贴板与其他首方区域语义审查继续推进，整体目标保持 active。

### Finding: SP-S16 Bitwarden 明文导入接受账号加密 JSON（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data Integrity / Backend API / Testing Authenticity
- Status: Confirmed
- Affected area: Bitwarden JSON parser command boundary
- Evidence:
  - File: `src-tauri/src/vault/import.rs:41`、`src-tauri/src/commands/entries.rs:513`（`2d5c9aa` 修复前源码）
  - Function / Module: `parse_bitwarden_json`
  - Relevant behavior: 解析 JSON 后直接读取 items 和 login 字段，未检查 encrypted/passwordProtected；账号加密导出保留相同外层结构，字符串值实际为密文。
  - Reproduction: 两项新增主机回归在旧实现失败，已有 login/note 与 malformed 对照通过，见 `bitwarden-encryption-marker-red.log`。独立探针调用真实 Tauri command，加载 Bitwarden 官方公开夹具（readonly clone `1a8bdbfe625bff90a9c0cabef2002e37f6038fa1`）：empty-account、account v1、account v2 三种加密导出返回成功，v1/v2 各产生两条记录；password-protected 本就因缺少 items 被拒绝，两份明文夹具正常。见 `bitwarden-public-encryption-red.log`。夹具内容未复制进仓库，未读取用户导出文件或凭据。
- Problem: 密文被误认为可直接迁移的凭据，导入流程能显示成功，却无法得到实际账号密码；畸形加密标记也会按明文解释。
- Why it matters: 用户选错导出选项时获得错误的成功反馈，可能误信迁移已完成。未观察用户数据丢失、凭据外传或解密绕过，按 Medium 记录。
- Realistic failure scenario: Bitwarden 导出 JSON 时选择账号限制加密，再在 SecPivot 选择“导入 Bitwarden”；旧 parser 返回包含密文字符串的登录条目，而非要求明文导出。
- Minimal fix: 在条目映射前验证 encrypted/passwordProtected；任一为 true 拒绝，并明确提示选择未加密 JSON；存在但非布尔的标记失败。缺标记的 legacy 明文和 false 明文仍保留兼容。
- Better long-term fix: 若未来支持加密导入，独立设计密码/KDF 解密边界，不能仅放行加密标记；继续审查明文自定义字段、链接字段、多 URL、组织分组与附件的保真。
- Regression test suggestion: 主机测试账号加密、密码加密、不一致 true 标记、非布尔标记、无标记/false 明文、错误不回显密文；独立官方 v1/v2/empty/password-protected 和两份明文夹具调用真实命令验证边界。真客户端导出另行验证。
- Estimated effort: 30–60 分钟（局部修复和针对性复核）；完整迁移保真另计

`0f07b47` 已修复 SP-S16。四项常驻 parser 回归全部通过；六份官方公开夹具
经真实命令独立复核，加密四份全部拒绝、明文两份保持成功。探针测试已移回
本任务 artifacts，生产仓库未带绝对路径或外部夹具依赖。incoming command
text 在返回时 Zeroizing 清理；未实现加密文件解密，也未承诺 JS 堆清除。

最终完整门禁日志 `verify-bitwarden-encryption-boundary-retry.log`：格式、
Svelte check、150 键契约、构建、前端 **286**、Rust **532 + ACL 2**、clippy
全通过。首次门禁在 Rust 编译阶段遇到 LLVM/PowerShell 主机内存不足而终止；
检查进程结束后，同源码、同验证命令重跑通过，未将其记录为应用运行时缺陷。
documentation currency gate 同步三份合同/安全/结构引用；TODO 的 Bitwarden
完整兼容状态降为 partial，保留自定义字段、链接字段、多 URI 与真实客户端
导出的后续验证。报告当前详细卡片 **27**（High 5 / Medium 18 / Low 4，
全部 Confirmed），原始 15 项与 6.1 分基线保持不变。

### Finding: SP-S17 Bitwarden 自定义名称造成迁移值丢失和原生语义误激活（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data Integrity / Privacy / Backend API
- Status: Confirmed
- Affected area: Bitwarden custom-field names at the shared KDBX write boundary
- Evidence:
  - File: `src-tauri/src/vault/import.rs:146`、`src-tauri/src/vault/serialize.rs:652`、`src-tauri/src/vault/helpers.rs:602`（`4aac415` 修复前源码）
  - Function / Module: `parse_bitwarden_json` / `sync_custom_fields` / OTP and KPRPC URL resolution
  - Relevant behavior: 外部 custom name 原样传给 EntryInput；写入 trim 后丢弃 reserved/empty names，重名进入 HashMap 后仅保留末值。HmacOtp 等被后端当成原生 OTP，KPRPC JSON 被当成浏览器配置读取，而 Bitwarden 自定义字段的名称本来只是标签。
  - Reproduction: 四项实际 parser command → ImportRow serde wire → VaultSession bulk mutation → KDBX save/reopen 回归均在旧实现失败，见 `bitwarden-field-retention-red-final.log`。Password/空名称等附加值缺失，PIN/空白变体与重名丢值，普通 HmacOtp 字段让主 login.totp 的 Totp 变 Hotp；KPRPC JSON 的 altURLs 扩增匹配网址。OverrideURL 字符串标签对照本就不改变独立 override_url 属性，未把该对照算成故障。所有数据库和字段均为合成数据。
- Problem: 导入成功不代表自定义内容完整迁移；外部标签还可能改变认证器或浏览器匹配语义。
- Why it matters: 用户失去部分附加凭据、得到错误验证码类型或意料之外的网址匹配。标准 Password 由 reserved guard 保留，未观察主密码被覆盖、旧源文件损坏或真实浏览器凭据外传，按 Medium 记录。
- Realistic failure scenario: Bitwarden 登录项的附加隐藏字段叫 Password，或多个字段标签为 PIN；导入后附加值不在目标库中。另一项普通标签为 HmacOtp 或 KPRPC JSON，目标客户端按原生控制数据解释它。
- Minimal fix: 给 Bitwarden 自定义字段统一来源前缀，空/缺名称使用 Field；通过共享唯一名称 allocator 保留每个重名值和保护标记。1PIF 使用同一 allocator 并保持已有 1Password 前缀行为；主账号/密码与 login.totp 映射保持原合同。
- Better long-term fix: 单独验证链接字段、多个 URI、附件、组织集合、typed items 和真实客户端导出；不能以本项命名修复宣称完整 Bitwarden 迁移。
- Regression test suggestion: 四项 KDBX 回归验证 reserved/blank names、带空白与显式后缀的重名、保护 flag、标准密码对照、主 TOTP 语义、KPRPC URL matching 和 OverrideURL 属性对照；补缺名称/null 名称 parser 检查，并运行原 1PIF 的保护/保真回归。
- Estimated effort: 1–2 小时（共享命名边界及 KDBX 复核）；剩余迁移能力另计

`7afcd81` 已修复 SP-S17。`import/fields.rs` 将既有 1PIF allocator 抽为共享
来源命名规则，保持 `1Password` 名称行为；Bitwarden 使用 `Bitwarden` 前缀、
唯一后缀和空/null/缺名称的 `Field` 兜底，保护标记随值保留。主 Password 与
login.totp 不改变；XML 原生 OTP 字段的独立导入路径不使用这个外部命名规则。
四项 KDBX 回归先红后绿，所有 13 项针对性 import 回归通过，含原五项 1PIF
schema/保护/serde/KDBX 验证。最终门禁另覆盖了补充 null/缺名称和 map 过滤。

完整 `npm run verify` 日志 `verify-bitwarden-field-retention.log`：格式、
Svelte check、150 键契约、构建、前端 **286**、Rust **536 + ACL 2**、clippy
全通过。documentation currency gate 同步 data-contracts、security-model、
project-structure 和 partial TODO。没有 CSS/markup 改动，也没有使用真实库、
用户导出或 OS 剪贴板。当前详细卡片 **28**（High 5 / Medium 19 / Low 4，
全部 Confirmed），基线 15 项/6.1 分保持不变。Bitwarden 链接字段、多 URI、
组织集合/附件/typed items 以及真客户端导出仍需独立补齐，Android provider、
原生 XML、现代 1PUX 和其他语义/界面覆盖继续推进，整体目标保持 active。

补报期间新出现的本地 `.openidea/changelists.json` 被 Prettier 纳入全仓检查，
格式门因此失败；这不是应用缺陷，也未读取、改写或删除该元数据内容。
`f3aab3d` 为本地 OpenIdea 目录增加 Git/Prettier 忽略，命令证明 ignored=true，
维护流程同步了保留用户元数据的规则。最终全仓格式门恢复通过；该工具边界
修正不改变上述应用回归结果或发现统计，既有 `.trae/` 仍保留原样。

### Finding: SP-S18 XML 导出改变回车凭据并允许无法解析的字符（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data Integrity / Stability / Testing
- Status: Confirmed
- Affected area: desktop and browser PlainXml export text encoding
- Evidence:
  - File: `src/lib/utils/kdbx-xml.ts:163`、`src-tauri/src/vault/security.rs:1118`（`8c7e7a6` 修复前源码）
  - Function / Module: `escapeXml` / `export_xml_content`
  - Relevant behavior: XML 转义复用 HTML 字符替换，原样写出回车及 XML 1.0 无法表示的控制字符。读取器按规范把 CR/CRLF 变为 LF，或拒绝非法文档。
  - Reproduction: 真实 Chromium DOMParser 读回实际 browser builder 输出时，合成密码和受保护字段的回车丢失，6 种非法 scalar 使整个文档出现 parsererror，2 种孤立代理字符在 DOM 边界被替换。两项 Rust 回归先保存重开合成 KDBX，证明秘密仍完整，然后实际导出并用 quick-xml 解码；回车变 LF，非法秘密导出错误地成功。见 `xml-character-browser-red.log`、`xml-character-backend-red-retry.log`；旧普通 XML 回归仍通过。
- Problem: 成功导出的 XML 可能不含原凭据，或完全无法被标准 XML 读取器打开。
- Why it matters: 迁移后认证失败或无法导入；未观察原 KDBX 被破坏，属于特殊字符触发的导出保真问题，按 Medium 记录。
- Realistic failure scenario: 从其他来源保存含回车或隐藏控制字符的密码/受保护字段，再通过 XML 迁移到另一客户端。
- Minimal fix: XML 专用转义用字符引用保留 CR；导出前拒绝 XML 1.0 无法表示的字符，返回不含字段内容的明确错误，目标文件和原凭据保持完整。不能直接套用会删除字符的兼容过滤到凭据上。
- Better long-term fix: 继续真实 KeePass/Android 导入互操作；不把 XML 导出当成附件、历史、原生分组属性的完整备份。
- Regression test suggestion: KDBX 保存重开 → XML 编码/解码保留 CR/CRLF、TAB、Unicode；非法密码/受保护字段使导出失败并保留旧目标，覆盖名称/分组/普通值与孤立 UTF-16 代理字符；真实 DOMParser 独立验证。
- Estimated effort: 1–2 小时（编码边界与回归）；原生客户端互操作另计

官方 KeePass `KdbxFile.Write.cs` 与 `StrUtil.SafeXmlString` 的公开源码已核对：
PlainXml 使用 `ProtectInMemory`，并过滤无法表示的 XML 字符。本修复采纳
XML 字符边界，对凭据采用明确拒绝以保留原值，不声明与官方的删除行为相同。
首轮 Rust 编译发生主机内存耗尽；原命令与源码未改变的重试执行到两项预期
失败，前次编译错误不算应用复现。

`a90394d` 已修复 SP-S18：前后端 XML 专用转义保留 CR；无法表示的字符
在文件写入前拒绝，错误不带字段内容。浏览器 builder 接收可选语言参数，
页面脚本传入当前 UI 语言；CSS/markup 不变。Rust 中间文档在成功交接前与
验证失败时由 Zeroizing 托管，导出命令保留既有零化写入边界。

两项前端回归与两项 Rust 回归先红后绿，原 XML 回归仍通过；后端绿测额外
分别覆盖非法 password 与单独非法受保护 PIN，在 6 种字符下保存重开后
拒绝写出，证明旧目标和两种原值保持完整。实际 Chromium DOMParser 绿测
保留 CR/CRLF、TAB 和有效 Unicode，8 种非法输入均由 builder 拒绝。
证据为 `xml-character-frontend-green.log`、`xml-character-backend-green.log`、
`xml-character-browser-green.log`；参考源码摘要在
`xml-character-reference-provenance.json`。

完整 `npm run verify`（`verify-xml-character-fidelity.log`）通过：格式、
Svelte check、150 键合同、构建、前端 **288**、Rust **538 + ACL 2** 和 clippy。
documentation currency gate 同步 data-contracts/security-model 与 partial TODO。
当前卡片 **29**（High 5 / Medium 20 / Low 4，全部 Confirmed），原始基线不变。
此验证使用合成库和浏览器夹具，没有真实用户库、OS 剪贴板或原生 KeePass
运行；Android 选择器创建文档的副作用和 provider 行为尚未实测。
手机 v1.6.5 的 `plugin:safe|create_document not allowed by ACL` 仍需新 APK
和设备回归，源码 `saf` 与报错 `safe` 的差异未查明；整体审计目标继续 active。

### Finding: SP-S19 Bitwarden 导入静默丢弃卡片、身份及其他专属类型（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data Integrity / Backend API / Testing
- Status: Confirmed
- Affected area: Bitwarden item type filtering and specialized fields
- Evidence:
  - File: `src-tauri/src/vault/import.rs:85`（`abd9008` 修复前源码）
  - Function / Module: `parse_bitwarden_json`
  - Relevant behavior: `type != 1 && type != 2` 直接 continue，返回成功但没有卡片、身份、SSH、银行账户、驾照或护照记录；未来未知类型与错误的专属内容也被该过滤掩盖。
  - Reproduction: 三项真实 parser command / ImportRow wire / KDBX 回归在旧代码失败：Card 返回零条，混合正常与未知/错误专属类型返回成功，带嵌套扩展内容的 Card 也返回零条。见 `bitwarden-typed-records-red-final.log`。首轮夹具使用了不存在的 get_notes API，已改为真实 Notes getter；编译错误不算应用复现。
  - Independent reference: Bitwarden 固定源码 `1a8bdbfe625bff90a9c0cabef2002e37f6038fa1` 的 cipher-type 和六种 Export 模型明确对应 3–8；[KeePassXC BitwardenReader](https://raw.githubusercontent.com/keepassxreboot/keepassxc/develop/src/format/BitwardenReader.cpp) 会创建身份/卡片条目并映射专属字段。来源摘要见 `bitwarden-typed-reference-provenance.json`；未复制第三方实现代码。
- Problem: 界面报告导入完成，但源导出中的整类记录没有进入目标库。
- Why it matters: 用户可能据此停止保管源导出，随后无法从目标库找到卡片号码、证件号或私钥；未观察源文件或已有 KDBX 损坏，按迁移边界的 Medium 问题记录。
- Realistic failure scenario: 一个 Bitwarden 明文 JSON 同时含登录项与卡片/身份项；导入后只看到登录项，其他项目无错误提示地消失。
- Minimal fix: 对官方 3–8 类型创建普通 KeePass 条目，用唯一来源名称映射专属字段并保护其内容；已知字符串字段校验类型，未知扩展内容以受保护 JSON 保留。未知 item type 或错误专属内容整次返回含记录序号、不含内容的错误，不返回部分成功。
- Better long-term fix: 链接字段、多 URI 的匹配规则、组织集合、原生历史、附件及真正的 SSH/证件专属操作仍需分别迁移验证；不能以字段保留宣称所有竞品运行能力已支持。
- Regression test suggestion: 对六种类型运行真实命令 → wire → bulk mutation → KDBX 保存重开，检查前导零、空白、私钥换行、专属值和保护标记、现有自定义字段；混合文件错误不返回前面的有效行；未知嵌套数据/名称碰撞仍保真。
- Estimated effort: 2–3 小时（类型保留与 KDBX 验证）；完整竞品操作语义另计

`00f8bbd` 已修复 SP-S19。六种专属类型通过真实 parser command → ImportRow
wire → import_entries 共用批量写入 → KDBX 保存重开，保留前导零、空白、
私钥 CRLF、所有专属值及保护标记；未知扩展内容受保护 JSON 保留，名称
碰撞不丢值。类型 2–8 同批关闭密码质量检查，登录保持开启，缺少元数据
保留 KeePass 默认；普通条目全字段编辑不修改该属性。

实施复核另发现新卡片会被报告为空密码，且 renderer 丢弃导入质量标记；
真实后端重开与前端 wire 两条回归先红后绿，见
`bitwarden-quality-rust-red.log`、`bitwarden-quality-frontend-red.log`。
混合 false/true/absent 经过 serde、批量写入及 KDBX 重开保留；无效分组
让整批不写入、不改 revision/dirty，非布尔 metadata 拒绝。真实浏览器
回退服务也保留质量标记。该问题属于本次类型迁移修复的语义边界，不另
增加发现卡片。最终针对性导入测试 17 项通过；完整门禁日志
`verify-bitwarden-typed-records.log` 包含前端 **290**、Rust **542 + ACL 2**、
格式、Svelte check、150 键设置契约、构建和 clippy，全部通过。提交前
补充文档后的完整格式门也通过。

documentation currency gate 同步 data-contracts/security-model/project-structure
及 partial TODO；稳定开发流程和主题变量没有变化。当前详细卡片 **30**
（High 5 / Medium 21 / Low 4，全部 Confirmed）；原始基线统计/6.1 分不变。
官方固定 Export 模型和 KeePassXC 映射用于独立来源复核，真实客户端的
专属类型导出、Android APK/选择器/provider 及剩余字段迁移仍未验证；
整体审计继续进行。

### Finding: SP-S20 Bitwarden 链接字段被导为空值且无法识别失效目标（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data Integrity / Backend API / Privacy
- Status: Confirmed
- Affected area: Bitwarden linked custom-field migration
- Evidence:
  - File: `src-tauri/src/vault/import.rs:141`（`1d3ddb4` 修复前）
  - Function / Module: `parse_bitwarden_json` custom fields mapping
  - Relevant behavior: 只读取 field.value，只有 type 1 才保护；type 3 的 linkedId 不解析。官方 FieldExport 保留 linkedId，CipherView.linkedFieldValue 用类型的 linkedFieldOptions 读取目标属性，而不是静态 field.value。
  - Reproduction: 两项真实 parser command → import wire → KDBX 回归在旧代码失败：登录密码链接成为未保护的空字段；混合有效记录和未知/跨类型/非数字/空链接目标仍返回成功。见 `bitwarden-linked-red.log`，未操作真实用户数据。
  - Independent reference: Bitwarden 固定提交 `1a8bdbfe625bff90a9c0cabef2002e37f6038fa1` 的 FieldType.Linked=3、LinkedIdType、FieldExport 和 Cipher/Login/Card/Identity View。七个来源 blob 身份见 `bitwarden-linked-reference-provenance.json`。
- Problem: 来源客户端可以通过自定义标签读取登录密码、卡片号或身份属性，迁移后同一标签只剩空值且没有错误提示。
- Why it matters: 用户依赖自定义字段填充表单时会复制空值，迁移成功提示掩盖字段语义丢失；来源主密码等标准值仍在原字段中，未观察已有库损坏。
- Realistic failure scenario: 登录项有名为表单密码的 linked field（type 3、linkedId 101、value null），导入后自定义字段不能再取得登录密码。
- Minimal fix: 按官方类型/编号读取目标值，保存为唯一且受保护的导入时快照；未知、跨类型或格式错误的链接在发布任何行前拒绝，并只报告记录序号。
- Better long-term fix: 导入快照不具备来源的动态属性链接；动态字段引用需要独立的 KeePass 引用/编辑合同与运行验证，不能以快照保留宣称动态同步已实现。
- Regression test suggestion: 登录 username/password、卡片所有六个目标、身份各字段及 FullName 的值/空白/空值/保护经 wire 和 KDBX 重开保留；跨类型/未知/缺失目标拒绝整批，不回显秘密；stale literal value 不覆盖实际链接目标。
- Estimated effort: 1–2 小时（静态迁移修复与类型复核）；动态引用另计

`78bb016` 已修复 SP-S20。`import/bitwarden_links.rs` 按官方 IDs 解析登录
username/password、卡片六项、身份十八项及组合 FullName，转为受保护
导入时快照。源码 label 仍经共享唯一名称分配器；已知空/缺失/null
目标合法保留空值，未知/跨类型/格式错误或非字符串目标整次拒绝。
FullName 保留姓名部分内部空白，仅按 JavaScript trim 规则处理组合结果，
合成夹具覆盖 BOM、空白 middleName 和外围空格。field.value 的陈旧字面值
不能覆盖链接目标；迁移后快照不具有动态同步语义。

两个旧代码失败回归转绿，补充卡片/身份全部编号、空目标和陈旧值验证；
实际 command → ImportRow wire → 批量导入 → KDBX 重开确认值与保护保留，
普通快照中受保护值仍为空。针对性导入 **20** 项通过；最后扩充错误目标
用例后完整 `npm run verify`（`verify-bitwarden-linked-fields.log`）通过：
前端 **290**、Rust **545 + ACL 2**、格式、Svelte check、150 键设置契约、
构建及 clippy。未改变 IPC shape、markup 或 CSS，documentation currency gate
更新 data-contracts/security-model/project-structure 与 partial TODO。

当前详细卡片 **31**（High 5 / Medium 22 / Low 4，全部 Confirmed），原始
基线保持不变。独立复核使用固定 Bitwarden 公开模型及合成库；没有真实
客户端导出、共享剪贴板、手机 APK 或 provider 运行证据。多 URI 匹配
模式、组织集合/历史/附件、1PUX、KDF 基准、其余生命周期和完整 UI/文件
语义覆盖仍待继续，整体目标未完成。

### Finding: SP-S21 注册域匹配跨越共享托管和通配公共后缀边界（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Privacy / Backend API / Testing
- Status: Confirmed
- Affected area: opt-in browser bridge/RPC registrable-domain credential matching
- Evidence:
  - File: `src-tauri/src/vault/helpers.rs:208`（`808a0df` 修复前）
  - Function / Module: `MULTI_LABEL_SUFFIX` / `registrable_domain` / `url_matches_accuracy`
  - Relevant behavior: 用固定的多标签后缀表加“最后两标签”回退替代完整 PSL；遗漏 PRIVATE 后缀和 wildcard 规则，把不同租户约化到同一共享域。`kprpc_matches_url` 经 BridgeHost.logins_for 和 RpcHost.find_logins 用于返回真实凭据。
  - Reproduction: 真实合成 KDBX 保存重开，开启 matchByRegistrableDomain，对 github.io、blogspot.com、azurewebsites.net、com.ar、tenant.ck、tenant.kawasaki.jp 六个独立租户边界发起请求，Bridge 和 RPC 每次均返回一条原租户凭据；同站子域正例和 www.ck/city.kawasaki.jp 例外正例仍通过。见 `psl-boundary-red-final.log`。初版夹具用了错误的重开口令，修正后才取得产品行为证据；`psl-boundary-red.log` 不作为产品复现。
  - Independent reference: [Mozilla Public Suffix List](https://publicsuffix.org/list/public_suffix_list.dat)，下载版本 2026-11-08_13-21-08_UTC、提交 7a21dc3491d43db036124fdd6a3a77d1cc44ad8a，明确包含上述三项私人后缀、com.ar、_.ck、_.kawasaki.jp 及例外规则；参考文件 SHA-256 为 dc00ceb7bb9256c41506b12a863db6a4b1c6021771eeccf5e073827750ac4b74。
- Problem: 已授权浏览器客户端查询另一个共享托管租户的网址，会收到原租户凭据候选，违反注册域匹配本应保留的站点边界。
- Why it matters: 恶意站点可使用公共托管服务获取同后缀的独立子域；该选项允许同站子域共享凭据，不应允许不同租户共享。功能默认关闭、需已授权客户端且未运行真实扩展/网络外传，按 High 的目的地验证问题记录。
- Realistic failure scenario: 为 vault-owner.github.io 保存登录信息，开启注册域匹配后访问 other-owner.github.io；后端将原登录名和密码列入可用条目。
- Minimal fix: 用包含 ICANN、PRIVATE、wildcard 和 exception 的维护版静态 PSL 替代手工子集；IP、单标签、无法提取注册域的主机保留完整主机边界，不将两个失败解析结果视为同域。
- Better long-term fix: 定期更新固定 PSL 依赖及安全回归；独立复核 URL authority、端口、IDNA、显式严格规则和原生扩展调用，不能以本项修复宣称整个自动填充授权已完整验证。
- Regression test suggestion: 六类不同租户经实际 Bridge/RPC 和 KDBX 重开都无凭据；同注册域子域、PSL 例外、已有 IP/尾点/多标签后缀和默认关闭规则继续通过，补充无注册域/未知后缀控制。
- Estimated effort: 1–2 小时（边界修复、依赖复核及主机回归）；真实浏览器验证另计

`31206eb` 已修复 SP-S21。固定 `psl` 2.1.249 / `psl-types` 2.0.11 的静态
Mozilla PSL 替代手工后缀子集，包含公共与私人后缀、通配及例外。域名切片
直接借用，IP 与无法提取注册域的主机保留完整身份；只在既有 opt-in
Domain 匹配分支生效。清单随二进制携带，查询不发送网络请求。

六类错误租户在原代码经 Bridge/RPC 都返回凭据，修复后均拒绝；真实
合成 KDBX 保存重开仍保持同站子域匹配。三个针对性回归覆盖私有/通配
边界、例外、裸后缀、单标签、未知后缀及精确本机正例；原有 IP、尾点、
多标签后缀及显式精度测试在完整门中继续通过。PSL 下载记录、库来源与
SHA-256 见 `psl-reference-provenance.json`；两个 crate 包摘要与 Cargo.lock
校验和一致。该依赖不含构建脚本，代码接口为 no_std 并禁止 unsafe。

首次完整 `npm run verify` 已通过功能阶段，但 clippy 报 Windows 1455
（宿主机分页文件不足，`verify-psl-boundaries.log`），不计为应用缺陷。
不改源码的 `psl-clippy-retry.log` 重试通过；随后真正重跑完整门禁
`verify-psl-boundaries-retry.log` 全部通过：前端 **290**、Rust **548 + ACL 2**、
格式、Svelte check、150 键合同、生产构建及 clippy。TODO 在直接验证完成
后才恢复勾选，documentation currency gate 同步数据、安全和模块引用。

当前详细卡片 **32**（High 6 / Medium 22 / Low 4，全部 Confirmed）；原始
基线统计/6.1 分不变。没有真实扩展、OS 剪贴板、用户库或 Android APK
运行验证；未来后缀变化须更新固定清单。URI 匹配模式、authority/端口/
IDNA、剩余竞品迁移、KDF 基准和界面/所有文件语义覆盖仍需继续。

### Finding: SP-S22 菜单缺少键盘导航与关闭后的焦点恢复（补充发现）

- Severity: Low
- Confidence: High
- Category: Accessibility / Design / Frontend State / Testing
- Status: Confirmed
- Affected area: shared viewport menu, column picker and context cascades
- Evidence:
  - File: `src/lib/components/ViewportMenuShell.svelte:43`、`src/lib/components/ContextMenu.svelte:109`（`954637f` 修复前）
  - Function / Module: `handleKeydown` / MenuItem / cascade rendering
  - Relevant behavior: shell 仅处理 bubble Escape，没有初始焦点、方向键/Home/End 导航或焦点恢复；级联只有 pointer/click 展开且未声明子菜单语义。
  - Reproduction: headless Edge 挂载生产 ColumnConfigMenu，打开后焦点仍在合成 invoker，ArrowDown 不移动；手动聚焦首项后 ArrowDown/End 仍停在首项；Escape 关闭后 activeElement 为 BODY。见 `column-menu-keyboard-baseline.json` / `.log`。三主题 × 三尺寸的滚动和末项点击通过，未将这些正常路径记作缺陷；截图已检查。
- Problem: 菜单声明为 ARIA menu，却不能按菜单键盘约定操作；关闭后用户丢失操作起点。
- Why it matters: 仅使用键盘的用户难以定位和连续操作列配置、上下文及级联工具。未复现底层条目误操作或数据改变，按 Low 的交互缺陷记录。
- Realistic failure scenario: 打开列配置后用方向键选择末项，焦点不进入菜单；手动 Tab 进入后按 Escape 退出，焦点落到 BODY。
- Minimal fix: 共享焦点/键盘所有权，按当前菜单层导航并跳过 disabled；Esc/Tab 关闭归还焦点，级联左右键和逐层 Escape；外部点击/新弹窗不被恢复焦点覆盖。
- Better long-term fix: 扩展原生 WebView、屏幕阅读器与手机软键盘验证，保留既有紧凑样式和主题 token。
- Regression test suggestion: 真实组件 initial/Arrow/Home/End、disabled/wrap、Enter/Space、Tab/Shift+Tab、逐层 Escape、子菜单异步卸载、外部点击、新弹窗和卸载 listener；dark/light/custom 与窄窗口检查。
- Estimated effort: 1–2 小时（共享行为与真实浏览器回归）；原生辅助技术验证另计

`15248b5` 已修复 SP-S22。共享 `menu-keyboard.ts` 按当前菜单层处理焦点和
上下/Home/End，跳过 disabled 并随键盘导航滚动。级联声明实例独立的
menu/haspopup/expanded/controls 语义，Right/Enter/Space 进入子项，Left/Esc
返回父项；根 Esc、Tab/Shift+Tab 和卸载按所有权归还焦点，外部点击与新
弹窗保留自己的焦点。新增行为沿用现有全局 focus-visible 和主题 token，
没有新增 CSS 配色/尺寸或 UI 重设计。

完整真实组件回归 `menu-keyboard-green-complete.log` 检查 initial、disabled/
wrap、逐层导航、叶项 Enter/Space、复选项不关闭、原生 Tab/Shift+Tab、
关闭回调仅一次、外部点击/新弹窗、卸载和 tick 前关闭后的晚到子焦点。
列菜单与长级联在 dark/light/custom × 1100×720、720×480、420×320 均在
8px 边距内，End 可滚到末项并派发；18 张截图已通过检查图复核，pageerrors
为空。键盘进入子菜单后鼠标移向相邻行的中间实现曾丢失焦点，见
`menu-pointer-transition-red.log`；最终转移焦点后 Escape 继续可用。

另补主页面早于 document 的 window capture 所有权检查，包含递归分组
菜单。`menu-keyboard-capture-red-valid.log` 中，原 page handler 在合成
group owner 打开时仍调用 Save 替身一次，最终打开时零次、关闭后一次。
该测试不调用真实存储或证明条目误写；前两版夹具缺回调和 modifier 默认
值的失败不作为产品证据。菜单消费按键带来的中间闲置计时回归
`menu-idle-red.log` 也已修正：闲置 watcher 改用 window capture，真实
timer 函数配合 fake clock 验证按键刷新期限、无活动仍锁定与 listener
cleanup。九项持久化到仓库的 frontend 回归通过。

最终完整门禁 `verify-menu-keyboard-complete.log` 通过：前端 **299**、
Rust **548 + ACL 2**、格式、Svelte check、150 键合同、生产构建及 clippy。
documentation currency gate 同步主题/模块/安全引用，TODO 仅勾选已经
取得主机和浏览器证据的这项行为。截图拼图工具的首次模块导入/Edge
路径探测失败属于检查工具问题，不计为应用缺陷；随后在标准 Node
环境成功生成并检查 `menu-keyboard-contact-sheet.png`。

当前详细卡片 **33**（High 6 / Medium 22 / Low 5，全部 Confirmed），
原始基线统计和 6.1 分保持不变。原生 Android WebView/屏幕阅读器、
手机 v1.6.5 另存为的新 APK/真机、URI 模式/authority/端口/IDNA、其余
竞品迁移、KDF 基准及完整文件语义/UI 覆盖仍待继续，整体目标未完成。

### Finding: SP-S23 Bitwarden 导入丢失逐网址匹配限制和后续网址（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Privacy / Data Integrity / Testing
- Status: Confirmed
- Affected area: Bitwarden login URI import and browser credential matching
- Evidence:
  - File: `src-tauri/src/vault/import.rs:143`、`src-tauri/src/vault/helpers.rs:255`（`10f9872` 修复前）
  - Function / Module: `parse_bitwarden_json` / `kprpc_matches_url`
  - Relevant behavior: uris 只取首个字符串 uri，不读取 match；存成普通 URL 后走本机默认匹配，后续 URI 和其规则没有保存。
  - Reproduction: `bitwarden-uri-modes-probe.rs` 使用真实 parser command → ImportRow wire → import_entries → 合成 KDBX 保存重开。Never(5) 同网址、Exact(3) 不同路径、Host(1) 不同端口、StartsWith(2) 不同前缀路径，在 BridgeHost/RpcHost 每次均返回一条合成凭据；探针只输出模式及数量，未输出密码，见 `bitwarden-uri-mode-red.log`。双网址例中保存后只有一条 URL，第一个网址各返回 1，第二个各返回 0，见 `bitwarden-multi-uri-red.log`。临时测试已按字节恢复，证据文件留在审计目录。
  - Independent reference: 固定 Bitwarden commit `1a8bdbfe625bff90a9c0cabef2002e37f6038fa1` 的 LoginUriExport 保留 uri/match，domain-service 定义 Domain=0、Host=1、StartsWith=2、Exact=3、Regex=4、Never=5；Never 明确不提供 autofill，Host 比较指定端口。源码摘要见 `bitwarden-uri-reference-provenance.json`；[官方 URI 匹配文档](https://bitwarden.com/help/uri-match-detection/) 下载 SHA-256 为 566092ea119db35386803d722a420cc92d78b2e965fc079716aa14967b62589c。
- Problem: 导入把用户的禁用/严格目的地策略改为默认宽松匹配，并静默删去其他网址，产生凭据返回边界和迁移完整性问题。
- Why it matters: 用户在源客户端禁止自动填充或限制到精确页面后，迁移不应自动开放。证据限合成库中的实际 host 适配器，没有真实浏览器扩展外传或用户库读取，按 High 的策略丢失记录。
- Realistic failure scenario: 将银行或管理后台条目设为 Never 后导入，浏览器请求该网址又得到条目；Exact 页面限制也不能阻止不同路径查询。
- Minimal fix: 在实现有保真性的逐 URI 策略前，对多个有效网址及显式匹配模式明确拒绝整份导入，错误只包含记录编号/不支持原因；不产生弱化条目或部分结果。
- Better long-term fix: 设计原生逐网址规则，保留所有 URI 和模式，覆盖大小写/端口/IDNA/PSL/正则与编辑生命周期，再开放对应导入；现有全条目的 KPRPC accuracy/regex 不能直接表达混合策略。
- Regression test suggestion: 0–5 显式模式、多网址、混合有效/不支持记录、非法模式/URI 类型均不返回可导入部分行；缺省/null 模式的单网址和无网址继续保存重开，既有类型/字段/链接/TOTP 回归保持通过；后续完整规则实现须运行同一 host 边界探针。
- Estimated effort: 1–2 小时（拒绝边界及回归）；完整逐网址规则与原生互操作另计

`4dbe49c` 关闭了 SP-S23 的不安全导入路径。新增 `default_login_uri`，最多
接受一个非空网址，且 match 必须缺省/null；显式模式 0–5 或多个有效
网址返回明确不支持错误，整份解析不给出部分 rows。非法 URI/mode 类型
和 URI 内部空白也拒绝，避免单个错误字符串在原生空白分隔 URL 字段中
变成多个目的地。无网址、缺省/null/空 URI 集合和单个默认网址继续支持。
没有新 IPC/DTO、匹配配置、UI 或依赖变更，后端常规 matcher 保持原行为。

这项修正是安全的格式拒绝边界；**完整逐 URI 功能尚未实现**，包含多个
网址或显式模式的标准 Bitwarden 文件现在不能导入，TODO 保持部分完成。
既有导入已丢失的网址/策略不能从当前 KDBX 自动恢复，须保留原始导出
以备完整迁移。没有检查或修改用户库，也没有建议通过删改规则绕过检查。

四项新增回归覆盖 0–5 模式、有效/不支持混合文件、多个默认网址、十二种
非法结构、内容不进入错误，以及六种支持的单/无网址实际 command→wire→
KDBX 保存重开。`bitwarden-uri-gate-green.log` 的 **19** 项针对性测试通过，
包括现有类型、链接、自定义标签、OTP/匹配、原子 quality metadata。
正式 `npm run verify` 的 `verify-bitwarden-uri-gate.log` 通过：前端 **299**、
Rust **552 + ACL 2**、格式、Svelte check、150 键合同、生产构建和 clippy。
临时 host 探针已按原字节撤下；该探针保留四种限制被放宽和第二网址丢失
的有效红灯证据，不在最终测试套件中留下预期失败测试。

documentation currency gate 更新数据/模块/安全引用与局部 TODO；报告和
metadata 同步 **34** 张 Confirmed 卡片（High 7 / Medium 22 / Low 5），
原始 15 项/6.1 分基线保持不变。原生 Android APK/选择器/provider、
真实扩展和屏幕阅读器未验证；完整逐 URI 模型、URL authority/端口/IDNA、
其余迁移、KDF 基准及完整语义/UI 覆盖仍待继续，整体目标未完成。

### Finding: SP-S24 手工主机提取把 URL 用户信息误判为凭据目的地（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Privacy / Testing
- Status: Confirmed
- Affected area: shared URL authority extraction and browser credential matching
- Evidence:
  - File: `src-tauri/src/util.rs:15`、`src-tauri/src/vault/helpers.rs:169`（`daff215` 修复前）
  - Function / Module: `url_host` / `kprpc_matches_url` / BridgeHost / RpcHost
  - Relevant behavior: 以冒号和路径切分 authority，未解析 userinfo，`https://bank.example:443@unrelated.test/login` 被提取成 bank.example；非法端口也被静默丢弃。
  - Reproduction: 两项新增回归在原实现失败；`url-authority-unit-red.log` 确认主机错误，`url-authority-host-red.log` 在合成 KDBX 保存重开后，默认与可注册域两种配置分别对 userinfo 伪装网址、非法端口请求各返回 Bridge/RPC 一条凭据，共四组越界。探针只输出模式/数量，没有输出密码，没有调用用户库、共享剪贴板或真实扩展。
  - Independent reference: 标准 Node WHATWG URL 解析输出保存在 `url-authority-reference.json`，该网址的 hostname 为 unrelated.test；正常用户名/密码不属于 host，国际化域名转换成 ASCII，IPv6 被规范化。规范：[WHATWG host parsing](https://url.spec.whatwg.org/#host-parsing)。
- Problem: 不可信网址中的用户信息被当成授权主机，目的地识别与浏览器的实际 URL 解释不同。
- Why it matters: 凭据匹配应以实际目的地主机为边界；当前合成后端实测证明错误返回，真实扩展如何构造请求尚未验证，按 High 的输入验证问题记录。
- Realistic failure scenario: 已授权的浏览器客户端提交带有可信站名 userinfo 的网址，目的地属于另一个站点，后端仍返回可信站条目。
- Minimal fix: 使用已有 url crate 解析 authority、IDNA 和 IP；支持既有无 scheme 主机/端口及协议相对地址，拒绝无主机、非法端口、内部空白和不透明 scheme，不增加依赖或 IPC。
- Better long-term fix: 单独复核端口、Exact 路径、scheme、blocked/regex 规则和真实扩展调用，不以主机解析修复宣称整个自动填充授权已完成。
- Regression test suggestion: 保留实际保存重开的两个 host 适配器负例与合法 userinfo 正例，补 IDNA/IPv6/协议相对地址/无 scheme 和非法 authority；完整验证既有匹配、自动填充、REF 与生成器。
- Estimated effort: 1–2 小时（共享解析边界及回归）

`1a3a680` 已修复 SP-S24。共享主机提取使用已有 url crate，正确排除
userinfo、规范化 ASCII IDNA/IP，并保留无 scheme 主机/端口和协议相对
地址；非法 authority/内部空白或控制字符/不透明地址不再猜测主机。
没有新增依赖、IPC、配置或 UI。三个永久回归覆盖单元边界、两个 host
适配器在合成 KDBX 重开后的越界负例/正常 userinfo，以及 IDNA/IPv6/
无协议等价主机和其他主机隔离。原始两项红灯和独立 WHATWG 结果保留。

`url-authority-unit-green.log` 的四项针对性测试、
`url-authority-host-complete.log` 的两项实际 KDBX host 回归通过。
完整 `verify-url-authority.log` 通过：前端 **299**、Rust **555 + ACL 2**、
格式、Svelte check、150 键契约、生产构建与 clippy。
documentation currency gate 同步数据/安全/结构引用；没有据此勾选
整个自动填充或移动端 TODO。当前卡片 **35**（High 8 / Medium 22 / Low 5），
原始 15 项/6.1 分基线不变。端口和 Exact 策略、regex/blocked 组合、
真实扩展、Android APK/真机、完整迁移和语义/UI 覆盖仍待继续。

### Finding: SP-S25 Exact 匹配忽略协议、路径大小写和查询限制（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Privacy / Testing
- Status: Confirmed
- Affected area: native KeePassRPC Exact URL policy shared by browser adapters
- Evidence:
  - File: `src-tauri/src/vault/helpers.rs:178`、`src/lib/i18n/zh-CN.ts:265`（`b111065` 修复前）
  - Function / Module: `url_matches_accuracy` / `strip_query` / Exact
  - Relevant behavior: 完整网址精确匹配实际上删除 scheme、query/fragment，再统一小写；配置 blockHostnameOnlyMatch=true 仍能命中不同协议/路径大小写/查询/fragment。
  - Reproduction: `exact_matching_preserves_complete_url_identity` 在实际合成 KDBX 保存重开后，用默认和可注册域两个设置通过 BridgeHost/RpcHost 查询。五种变化各返回一条条目，共十组错误命中，见 `exact-url-policy-red.log`；原网址正例通过。日志仅标识变化类型/配置/数量，没有输出密码。
  - Independent reference: 固定 KeePassRPC commit `dc0a59b60b3ac21da9944bc5b98fe96e706af80a`，EntryConfigv1.cs:44 将 BlockHostnameOnlyMatch 映射到 Exact；KeePassRPCService.cs:345–350 在完整字符串相等时返回 Best，Exact 对其他字符串立即 continue，删除 query 只用于较宽松模式。文件存于 `kprpc-v1-accuracy-reference.cs` / `kprpc-url-match-core-reference.cs`，后者 SHA-256 48aa6042c6f058cfb706af7499720bee0dc1721f5ce5050d675de3259e9412a5。
- Problem: 用户选择完整网址限制后，后端仍放宽目的地策略，与界面和源客户端的 Exact 语义不一致。
- Why it matters: 精确 URL 可区分 HTTPS、区分大小写的路径及查询中的租户；不能在缺少用户选择时删去这些限制。证据是合成后端返回，不声称已发生真实扩展外传。
- Realistic failure scenario: 条目仅允许 HTTPS 的某租户登录网址，另一个 tenant 查询或 HTTP 网址仍可取得该条目。
- Minimal fix: 在有效主机验证之后，Exact 仅接受完整原字符串相等；删除该分支的自制 query/scheme/path 规范化，保持其他 accuracy 和显式 regex/blocked 优先级。
- Better long-term fix: 单独复核 Hostname 端口策略、规则交互及扩展输入，后续如要支持规范化应提供明确独立模式，不能偷偷扩大 Exact。
- Regression test suggestion: 保留实际两个 host、两种配置、五种不同网址与完整网址正例的保存重开回归，继续运行既有 Exact/subdomain、PSL、alternate/blocked/regex 和 authority 测试。
- Estimated effort: 1 小时（严格分支及回归）

`49a5be3` 已修复 SP-S25。Exact 只接受完整原网址相等；删除自制
scheme/query/path 规范化，并让 Bridge 保留请求与 submit URL 的原始
大小写。前端 `shortestMatchable` 在 Exact 下返回原始 trimmed 网址，
不再删 query/fragment 或补造协议。没有新配置/IPC/依赖或 markup/CSS。

最初只修共享分支时，`exact-url-policy-green.log` 和
`verify-exact-url-policy.log` 的合法网址正例失败，直接暴露 Bridge
预先 lower-case 的第二层问题。该失败已修复，不作为通过证据。
最终 `exact-url-policy-complete.log` 验证实际合成 KDBX 重开的两个 host、
两种配置、五类变化和合法 request/submit 正例；前端两项建议地址回归
的红/绿日志为 `exact-url-suggestion-{red,green}.log`。
正式 `verify-exact-url-policy-complete.log` 通过：前端 **301**、
Rust **556 + ACL 2**、格式、Svelte check、150 键契约、生产构建与 clippy。
源码引用固定 KeePassRPC commit 及下载 hash；API 限流和部分 clone
lazy-fetch 失败仅为取证工具问题，最终使用固定 raw 源码完成核对。

documentation currency gate 同步数据/安全/结构引用。当前卡片 **36**
（High 9 / Medium 22 / Low 5），原始 15 项/6.1 分基线保持不变。
Hostname 端口、frontend authority/建议地址、regex/blocked 组合、真实
扩展与 Android APK/真机、其余迁移及完整语义/UI 覆盖仍待继续。
整体目标保持进行中，未将手机 v1.6.5 另存为标为真机已解决。

### Finding: SP-S26 Hostname 规则忽略显式端口且建议地址删去端口（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Privacy / Testing
- Status: Confirmed
- Affected area: host-and-port credential matching and frontend rule suggestions
- Evidence:
  - File: `src-tauri/src/vault/helpers.rs:179`、`src/lib/utils/match-url.ts:40`、`src/lib/i18n/zh-CN.ts:263`（`e98fecb` 修复前）
  - Function / Module: `url_matches_accuracy` / `shortestMatchable` / Hostname
  - Relevant behavior: blockDomainOnlyMatch=true 仅比较 url_host，端口已丢弃；“主机名+端口相同才匹配”的选项没有兑现，生成建议也只返回 host。
  - Reproduction: `hostname_matching_preserves_explicit_port_boundaries` 通过实际合成 KDBX 保存重开，非默认端口变化、显式默认端口→省略、省略→显式默认端口、IPv6 端口变化，在默认和可注册域两种设置下通过 Bridge/RPC 各返回 1，共八组错误命中；相同 authority 不同路径的正例通过，见 `hostname-port-red.log`。前端对应回归 `hostname-suggestion-red.log` 显示 :8443 被丢弃。
  - Independent reference: 固定 KeePassRPC commit `dc0a59b60b3ac21da9944bc5b98fe96e706af80a`，KeePassRPCService.cs:368–374 比较 HostnameAndPort，Hostname 不继续放宽；URLSummary 保留输入显式端口，省略端口不会根据 scheme 补入。`kprpc-url-match-core-reference.cs` 与本任务只读引用保存该证据。
- Problem: 使用严格主机模式限制同站不同服务时，端口约束被静默移除；用户采纳的建议规则也不能保留该目的地限制。
- Why it matters: 同一主机可在不同端口运行不同应用/管理界面，不能在严格策略下自动扩大凭据匹配范围。这里是合成适配器实测，不声称已发生真实扩展外传。
- Realistic failure scenario: 凭据仅绑定 bank.example:8443 的管理服务，访问该主机的另一端口仍得到条目。
- Minimal fix: 在已验证 canonical host 之上保留独立显式端口，Hostname 比较两者且不补猜 scheme 默认端口；前端建议保留相同端口并正确输出 IPv6 括号。
- Better long-term fix: 用同一合成 authority 语料验证前后端规则，进一步核对 regex/blocked 组合、真实扩展输入与逐 URI 规则，保持 Domain 的明确宽松语义。
- Regression test suggestion: 保留四类实际重开负例与相同 authority 正例；补 alt URL、默认/省略、userinfo、IPv6、IDNA、非法端口和前后端 authority 语料，继续运行 Exact/PSL 与默认域名匹配回归。
- Estimated effort: 2–3 小时（前后端端口保真及回归）

`8a7e939` 已修复 SP-S26。Hostname 比较 canonical host 与独立显式
数值端口，含默认端口；省略保持 None，不因 scheme 补造端口。Domain
继续采用明确的宽松主机/站点范围。前端建议使用 WHATWG URL 验证，并
保留端口、ASCII IDNA 和 IPv6 括号，不能把严格规则变成无端口目的地。
附 `kprpc-url-summary-reference.cs` 固定来源摘要 SHA-256
c2325c3cce5f9f522ba2b4595797472a3c27a2989ff31fc3ae0d3408c4c0185f。

共享 `tests/fixtures/url-authorities.json` 的 **27** 条语料在 Rust/Node
通过，**19** 条支持地址的建议 alt 规则在实际合成 KDBX 保存重开后
通过 Bridge/RPC、两种配置，并排除其他 authority。端口四类负例及
相同 authority 正例修复通过，Domain 不同端口/协议对照继续通过。
复核也保留了裸地址 path/query 中的 @ 或 ://，避免把它们误当 userinfo
或 scheme；非法 authority/端口/内部控制字符仍拒绝。JSON 语料参与
现有严格格式覆盖，Prettier 不加入 JSONC 尾逗号。

`hostname-rule-integration.log`、`url-authority-corpus-{rust,frontend}.log`
保留针对性证据。初轮完整门禁 `verify-hostname-port.log` 在行为测试
通过后因 clippy 的 manual_strip 失败；改为 strip_prefix 后，正式
`verify-hostname-port-complete.log` 全部通过：前端 **303**、Rust
**560 + ACL 2**、格式、Svelte check、150 键契约、构建与 clippy。
documentation currency gate 同步数据/安全/模块/维护引用，TODO 仅新增
由这些直接证据覆盖的 Hostname 项；没有新 IPC/配置/依赖或 UI markup。

截至 2026-11-10，当前卡片 **37**（High 10 / Medium 22 / Low 5），
原始 15 项/6.1 分基线不变。完整逐 URI 迁移、regex/blocked 交互、
原生扩展、Android APK/真机及剩余语义/UI 覆盖仍待继续；CSV 的公式
防护与 RFC 引号组合进入下一轮独立验证，当前尚不计入正式发现。

### Finding: SP-S27 CSV 公式防护前缀破坏标准单元格引用（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / Stability / Testing
- Status: Confirmed
- Affected area: frontend and native CSV export
- Evidence:
  - File: `src/lib/utils/csv.ts:12`、`src-tauri/src/vault/serialize.rs:475`（`7cfe304` 修复前）
  - Function / Module: `escapeCsv` / `escape_csv` / `export_csv`
  - Relevant behavior: 两个 writer 都先引用原字段，再把公式防护撇号加在 CSV 引号外；标准读取器认为此字段已以普通字符开始，逗号和换行不再受引号保护。
  - Reproduction: 前端 buildCsv 与实际合成 VaultSession.export_csv 各导出 8 条记录，密码全部为空；Python stdlib csv.reader(strict=True) 各读出 12 行（含表头），列数为 8/9/8/2/7/9/9/2/1/7/8/8，字段保真为 false。原生 wire 回归同时失败；日志 `csv-standard-reader-red.log`、`csv-backend-red.log`，仅记录行列数/布尔值。
- Problem: 含危险前缀与逗号/引号/换行的标题、用户名、密码或笔记可能拆成额外字段或记录；内部宽松 parser 会掩盖 wire 格式问题。
- Why it matters: 导出文件常用于迁移或应急读取，字段错位会将内容解释到错误列，造成数据损坏。
- Realistic failure scenario: 笔记以减号开头并包含换行，另一个 CSV 客户端将笔记拆成多条记录；带逗号的公式样式字段把后续凭据列整体移位。
- Minimal fix: 先把防护前缀加入字段值，再统一执行 RFC 4180 引号及双引号转义；保留已有前缀保护策略。
- Better long-term fix: 用独立标准读取器覆盖两个 writer 的合成输出，并独立评估导入严格性及公式防护造成的值变化，避免删除用户原有撇号。
- Regression test suggestion: 六种危险前缀分别组合逗号/引号/CRLF，检查实际原生文件和前端序列化、行列数、已有撇号和普通引用单元格不变。
- Estimated effort: 0.5–1 小时（两个 writer 及导出 wire 回归）

`19b6658` 已修复 SP-S27。前端及 Rust writer 先对字段值添加原有公式
防护，再执行 RFC 引号引用和双引号转义。原有普通撇号保持字面值。
`csv-standard-reader-green.log` 的独立 Python stdlib strict reader 对
两份实际输出均读出 **9 行（含表头）、每行 8 列**，所有防护后的
字段值一致。前端 wire 回归和原生实际文件回归覆盖六种前缀、逗号、
双引号、CRLF、普通引用内容和原有撇号；临时审计输出语句已从测试移除。
独立 reader 脚本 SHA-256
92ba448da46549efce567324639af8ff2a8a325084227a1f8c28e351688c56b8。

本轮首先尝试独立 Windows window station；CreateWindowStation 返回
WinError 5，未启动测试。随后通过任务外部 Cargo runner 明确排除两项
共享剪贴板测试：entry_exchange_copy_then_paste_creates_a_new_entry 和
scheduled_wipe_clears_our_own_text；后者已有环境开关，前者开关缺失将另行
审计。没有接触用户数据库或共享剪贴板，也没有修改测试命令的仓库配置。
第一轮门禁 `verify-csv-formula-quoting.log` 的 CSV 回归通过，但已有
loopback listener-stop 测试发生系统套接字资源不足 10055；单独复跑
`csv-loopback-resource-recheck.log` 通过。保留失败日志后以
RUST_TEST_THREADS=1 串行复跑完整门禁，正式
`verify-csv-formula-quoting-complete.log` 终态 exit 0：前端 **304**、
Rust **559 通过 / 2 明确过滤**（发现 561 项）、ACL **2**，格式、
Svelte、150 键契约、生产构建与 clippy 均通过。未将过滤测试计为执行。

documentation currency gate 更新数据/安全/模块引用与这一有限 TODO。
没有新 IPC/配置/依赖或 UI markup/CSS；防护会改变原字段值，此修复
不声称可无损导回，也不声称真实 Excel/LibreOffice 执行验证。当前
卡片 **38**（High 10 / Medium 23 / Low 5），原始 15 项/6.1 分不变。
整体审计继续进行；CSV 导入严格性、逐 URI 迁移及其余语义/UI 覆盖
继续核查，手机 v1.6.5 另存为仍缺新 APK/真机选择器验证。

### Finding: SP-S28 条目交换测试绕过实际剪贴板显式开关（补充发现）

- Severity: Low
- Confidence: High
- Category: Privacy / Testing / Documentation
- Status: Confirmed
- Affected area: Windows live entry-exchange test in the default verification suite
- Evidence:
  - File: `src-tauri/src/vault/tests.rs:46`、`src-tauri/src/platform/clipboard.rs:337`、`skills/secpivot-dev/references/security-model.md:199`（`caaa454` 修复前）
  - Function / Module: `entry_exchange_copy_then_paste_creates_a_new_entry` / `clipboard_is_usable`
  - Relevant behavior: 该 Windows 测试未经 SECPIVOT_CLIPBOARD_LIVE=1 检查即读取系统剪贴板，再调用会写入探针的 usability helper；同仓库其他实际剪贴板测试已要求该开关。仅共享进程 mutex 不能隔离用户/其他进程。
  - Reproduction: 临时 assert stop point 放在第一处 OS 读取之前，移除环境开关后针对性 cargo test 仍到达停止点并失败；`exchange-live-optin-red.log` 留下安全的控制流证据。没有执行系统剪贴板读取/写入，不将推断的格式损失称为实际复现。
- Problem: 普通 npm verify / cargo test 可意外读取并替换开发者剪贴板内容，违背现有默认验证边界。
- Why it matters: clipboard_is_usable 写入探针；restore_clipboard 只保存/恢复文本，无法完整恢复图片及其他非文本格式。问题限于开发验证路径，未证明生产客户端隐私泄露，故评为 Low。
- Realistic failure scenario: 开发者复制截图后运行普通验证，条目交换测试将系统剪贴板替换为测试数据；最后只恢复文本或清空，原截图无法从该 helper 恢复。
- Minimal fix: 在任何 OS 剪贴板读取、锁/探针操作前应用现有显式开关，缺省及非 1 值返回内容无关的 skip 提示。
- Better long-term fix: 实际剪贴板验证仅在用户明确开启、可丢弃的环境执行；保持内存化领域/调度回归覆盖，避免把返回 skip 的测试称为实际 OS 互操作通过。
- Regression test suggestion: 读取前停止点证明缺省/0/true/空值均提前退出，1 到达停止点但不读取系统剪贴板；移除停止点后完整默认门禁无外部 runner 过滤通过。
- Estimated effort: 0.5 小时（入口开关、控制流验证与文档边界）

`6736799` 已修复 SP-S28。入口使用现有开关，在任何 shared clipboard
读取/探针之前返回。临时停止点验证缺省、0、true、空值不进入；仅 1
到达停止点并预期失败，仍未执行 OS 操作。日志
`exchange-live-optin-unset-green.log`、`exchange-optin-control-flow.log`
及四个值的独立日志保存该证据；临时 assertion 已移除。

正式 `verify-exchange-live-optin.log` 无外部 Cargo runner/过滤器，
RUST_TEST_THREADS=2 运行完成 exit 0：前端 **304**、Rust harness
**561**、ACL **2**，格式、Svelte、150 键契约、构建与 clippy 通过。
四个实际剪贴板测试的 body 因缺省开关提前 skip，这不是实际 OS
互操作通过。维护/安全引用同步这个边界，没有改变产品剪贴板实现
或任何领域断言。当前卡片 **39**（High 10 / Medium 23 / Low 6）；
原始基线不变，完整审计继续。CSV 异常引号的转换/模拟 IPC 与独立
标准 reader 已显示差异，将单独记录修复，不合并进此测试安全单元。

### Finding: SP-S29 CSV 导入静默接受异常引号并提交损坏字段（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / Stability / Frontend state
- Status: Confirmed
- Affected area: CSV parser and main-window CSV import
- Evidence:
  - File: `src/lib/utils/csv.ts:47`、`src/lib/services/io.ts:124`、`src/routes/+page.svelte:1115`（`86bfab8` 修复前）
  - Function / Module: `parseCsv` / `csvToImportEntries` / `handleImportCsv`
  - Relevant behavior: parser 在任何位置见双引号都进入引用，不验证结束引用后的字符，也不拒绝 EOF 时尚在引用中；残缺内容正常返回条目，页面没有解析错误反馈分支。
  - Reproduction: 两个纯合成文件分别缺少结束引号、在结束引号后接 junk；实际 csvToImportEntries 各返回 2/1 条，真实 importEntries 服务均向模拟 Tauri adapter 提交 1 次 bulk IPC。Python stdlib csv.reader(strict=True) 独立拒绝两者，合法含逗号/双引号/CRLF 对照均正常。`csv-malformed-io-red.log`、`csv-malformed-standard.log` 仅记录计数和布尔值，未写用户数据库。
- Problem: 不完整文件会把后续字段/记录拼进引用字段，异常引号也会被删除或拼接；用户看到正常导入，无法发现原文件已损坏。
- Why it matters: 迁移的标题、用户名、密码及笔记边界必须保持；识别到格式错误时整份解析失败才能避免局部正确记录掩盖后续损坏。
- Realistic failure scenario: 导出/传输截断了最后一个多行字段的结束引号，导入器仍保存被截断的记录，且不提示重新取得完整文件。
- Minimal fix: 按字段开始/引用中/已结束引用的状态验证 CSV 语法，未闭合、非引用字段内双引号及结束引用后的非法字符明确拒绝；页面捕获本地化、无字段内容的解析错误并停止导入。
- Better long-term fix: 保留合法 BOM、双引号转义和多行字段语料，并用独立标准读取器验证；对历史不合规范导出明确提示检查原文件，不猜测或删除真实内容以继续导入。
- Regression test suggestion: 完整前缀后接残缺记录应整份失败，错误不含合成标记；实际服务/页面 handler 不提交批量写入，合法多行保真且切换视图后旧结果不显示反馈。
- Estimated effort: 1–2 小时（状态验证、中文反馈、服务/页面回归）

`e1f4bdd` 已修复 SP-S29。parser 显式区分字段开始、引用中和引用结束，
异常格式整份抛出无内容 CsvParseError；IO 翻译为当前语言，CSV 页面
捕获并显示错误，先检查发起视图再解析。保留 BOM、quoted empty、
escaped quotes、原始 CR/LF 内容，以及 CRLF/LF/CR 行结束。未删除
真实撇号、未猜测修复历史不合规范的文件；这些文件现在会明确拒绝。

新增两项 parser、三项真实服务/提取页面 handler 回归，覆盖残缺尾部
整份拒绝、内容不泄露、中文提示不带英文 Error 前缀、合法多行批量
字段及 A→B→A 旧选择不写入/不提示。Python stdlib writer 独立生成
**288** 组合法文档，含最小/全引用、BOM、空字段、空白、逗号、双引号、
CR/LF、多行、Unicode、尾行结束变体，全部字段保真。
`csv-valid-standard-corpus.log`、`csv-strict-import-final-targeted-complete.log`
及 `csv-malformed-io-green.log` 保存计数/布尔证据。

真实浏览器验证在全新 headless Edge profile、独立 Vite dev 实例完成：
菜单→文件选择→FileReader→主页面，异常文件显示中文提示，合成库仍为空
且 dirty=false；合法文件导入一条，多行 notes/title 原值保留，pageerror=0。
`csv-import-browser-final.log`、760×520 dark 截图
`csv-import-feedback-dark.png` 已检查，临时 Vite 进程已终止。最初长期
开发服务器 fixture 未进入主视图而超时；独立服务后 locator 又同时匹配
桌面/移动两个节点，限定可见行后通过。这些失败未计为产品问题。去掉
英文 Error 前缀的提取 handler 测试曾因 VM/服务不同 Error 构造器失败，
绑定与实际应用一致的构造器后通过；失败日志完整保留。

正式 `verify-csv-strict-import-complete.log` 终态 exit 0：前端 **309**、
Rust harness **561**、ACL **2**，格式、Svelte、150 键契约、构建与
clippy 通过，RUST_TEST_THREADS=2，无外部过滤。四个原生剪贴板 body
因缺省显式开关 skip，不称为实际 OS 互操作通过。开发数据/安全/模块
引用同步，TODO 仅新增由这些证据覆盖的 CSV 语法项。无新 IPC/配置/
依赖或 CSS/markup；浏览器 fallback 不证明原生/Android 运行行为。
当前卡片 **40**（High 10 / Medium 24 / Low 6），原始 15 项/6.1 分不变。
整体目标继续：XML 解析错误反馈、提示色彩语义、逐 URI 迁移及其余
系统/UI 覆盖仍待核查，手机 v1.6.5 另存为仍缺 APK/真机验证。

### Finding: SP-S30 XML 导入解析失败没有界面反馈（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Frontend state / Accessibility
- Status: Confirmed
- Affected area: main-window KeePass XML import
- Evidence:
  - File: `src/routes/+page.svelte:1144`、`src/lib/utils/kdbx-xml.ts:115`（`576cdd9` 修复前）
  - Function / Module: `handleImportXml` / `xmlToImportEntries` / `parseKdbxXml`
  - Relevant behavior: XML parser 会明确拒绝损坏/外部加密字段，页面在 try/catch 外调用，入口又由 void callback 调用；拒绝变成 unhandled promise，没有用户反馈。选择器还列出 parser 不支持的 CSV filter。
  - Reproduction: 全新 headless Edge、独立 Vite、纯合成空库，真实菜单→XML 选择器→FileReader→页面。损坏 XML、带 foreign Protected=True 字段 XML 各出现 1 个 pageerror、0 个 tip；库保持 0 条且 dirty=false。合法 PlainXml 对照导入 1 条。`xml-import-browser-red.log` 仅记录计数与布尔值。
- Problem: 用户选择无法导入的文件后只看到界面没有变化，不知道文件损坏或需要非加密 PlainXml，无法采取正确重试动作。
- Why it matters: parser 的安全拒绝必须被呈现给用户，否则会误以为软件无响应；这是反馈缺失，没有证据表明安全校验绕过或用户数据库损坏。
- Realistic failure scenario: 从其他 KeePass 客户端取得带 stream-encrypted Protected 字段的 XML，当前 parser 正确拒绝，但界面不显示已有的兼容性错误说明。
- Minimal fix: 页面捕获本地化、无字段值的 parser 错误，绑定发起视图并停止写入；选择器仅列出实际支持的 XML filter。
- Better long-term fix: 对所有导入格式的解析、提示和写入边界做同样的真实页面验证，继续核查反馈严重度和跨格式兼容。
- Regression test suggestion: 两类实际 parser 拒绝均显示反馈且无 unhandled error/写入；合法字段仍正确映射，过时 picker 结果不解析/不提示，XML picker 不声称支持 CSV。
- Estimated effort: 0.5–1 小时（页面反馈、filter 和回归）

### SP-S30 修复与独立复核（2026-11-10）

`3119cf0` 已修复 XML 页面错误反馈：现有 parser 的损坏 XML 与外部
加密字段拒绝在发起视图显示本地化提示，错误不包含字段值；过时 picker
结果不解析、不写入。选择器只列 XML，合法映射字段保持原行为。
`xml-import-handler-red.log` 的三个回归在修复前失败，green 日志通过。
真实全新 Edge 的 `xml-import-browser-red.log` 两类各 0 tip / 1 pageerror，
`xml-import-browser-green.log` 两类各 1 tip / 0 pageerror，合成库保持为空
且 clean，合法 PlainXml 对照成功。独立 Vite 进程已停止。

完整 `verify-xml-import-feedback.log` 终态 exit 0：前端 **312**、Rust
harness **561**、ACL **2**，格式、Svelte、构建与 clippy 通过。
RUST_TEST_THREADS=2，无外部过滤，四个 live clipboard body 默认 skip。
文档 currency gate 同步 data-contracts/project-structure；TODO 仅标记
本次反馈/filter/view 边界。无 parser/IPC/依赖或 CSS/markup 变化。
当前卡片 **41**（High 10 / Medium 25 / Low 6），原始 15 项/6.1 分不变。
完整 XML 保真、原生互操作、其他系统与 UI 范围继续验证；手机 v1.6.5
云端另存为仍缺新 APK/真机证据。

### Finding: SP-S31 操作失败被显示为绿色成功提示（补充发现）

- Severity: Low
- Confidence: High
- Category: Design / Frontend state / Accessibility
- Status: Confirmed
- Affected area: main-window failures, IO/group/editor notification seams
- Evidence:
  - File: `src/routes/+page.svelte:426`、`src/lib/services/io.ts:43`、`src/lib/services/group-flows.ts:24`、`src/lib/composables/useEntryEditor.svelte.ts:29`、`src/lib/components/TipsHost.svelte:60`（`ca8e9d7` 修复前）
  - Function / Module: `flash` / `IoHost.notify` / `GroupFlowHost.notify` / `EntryEditorOptions.notify` / `TipsHost`
  - Relevant behavior: 全局 TipKind 已区分 success/error，TipsHost 已有 danger token 样式；页面 flash 只转发文字，主页面失败和服务 notify 默认产生 success。真实 CSV 失败截图 `csv-import-feedback-dark.png` 呈绿色，文本仍正确。
  - Reproduction: `tip-kind-red.log` 执行实际 flash、文件 picker 拒绝、batch IPC 拒绝、group rename 拒绝和 editor create 拒绝，五类预期 error 全部得到 success；成功默认及过时视图静默对照通过。仅合成输入及模拟 IPC，无用户库/原生剪贴板。
- Problem: 失败提示使用成功颜色，与错误文字冲突，快速扫视时会误判操作结果。
- Why it matters: 保存/导入等失败应有一致的错误反馈；文字仍包含失败说明，因此判为 Low，不声称数据损坏或权限绕过。
- Realistic failure scenario: 云端另存为被 ACL 拒绝，提示虽写着失败但使用绿色成功样式。
- Minimal fix: 传递已有 TipKind，在明确失败分支标记 error；成功默认保留，不通过翻译文本猜测类型。
- Better long-term fix: 继续独立检查 warning/info、对比度及长消息窄屏布局；参考只读 sibling clipboard 的显式类型设计，不引入平行样式。
- Regression test suggestion: 实际失败 seams 产生 error，正常完成仍为 success，旧视图不显示提示；真实 dark/light/custom 和窄屏检查现有 danger 样式。
- Estimated effort: 1–2 小时（显式种类透传和运行复核）

### SP-S31 修复与独立复核（2026-11-10）

`414e049`在 page flash 和 IO/group/favicon/editor
notify 类型中传递已有 TipKind，明确失败分支使用 error。没有按翻译文字
推测类型，没有新 token/样式/布局。`tip-kind-red.log` 五类实际源模块
失败得到 success，`tip-kind-green.log` 五类得到 error；成功默认和
A→B→A 旧视图静默对照通过。相邻 IO tests **38** 通过。

真实 Edge 的 `tip-kind-browser.log` 复核 CSV/XML 两类失败和成功导入，
深色/浅色/自定义 × 1100×720、760×520、390×720 全部通过，无 pageerror；
九张截图及 `tip-kind-contact-sheet.png` 已检查。浅色错误文本对比度另有
独立问题（约 3.13:1），未混入本提交或误称已解决。外部修改脚本初次
缺少 TypeScript 模块解析、又未处理调用尾逗号，均在格式检查中发现并
修正；这些工具问题不计产品缺陷。

`verify-tip-kind.log` 完整门禁终态 exit 0：前端 **312**、Rust harness
**561**、ACL **2**、格式/check/build/clippy 通过；两线程、无外部
过滤，四个 live clipboard body 缺省 skip。文档 currency gate 更新
三份开发引用。未新增低影响样式的永久测试，复用既有行为测试和外部
运行探针。仅合成库/模拟 IPC，全新 profile；无真实库/OS 剪贴板/设备。
当前卡片 **42**（High 10 / Medium 25 / Low 7），基线 15 项/6.1 分不变。
浅色对比度、其他系统/UI 覆盖及手机 v1.6.5 另存为 APK/真机仍待验证。

### Finding: SP-S32 浅色提示文字对比度不足（补充发现）

- Severity: Low
- Confidence: High
- Category: Accessibility / Design
- Status: Confirmed
- Affected area: global TipsHost success/error text
- Evidence:
  - File: `src/lib/components/TipsHost.svelte:46`、`src/lib/components/TipsHost.svelte:55`、`src/lib/types/theme.ts:69`（`c4adb66` 修复前）
  - Function / Module: `.tip` / `.tip.error` semantic text color mixes
  - Relevant behavior: 11px 正文把 success/danger 色与 20% 白混合，浅色 surface 上进一步变浅；正文被固定调亮而没有使用主题正文 token。
  - Reproduction: `tip-contrast-red.log` 从实际 Edge getComputedStyle 读取 sRGB 文字与不透明背景，按 WCAG 相对亮度计算，18 组中 12 组低于普通正文 4.5:1：light error 3.13、light success 2.59，custom error 3.62、success 3.38。dark 对照通过；三窗口尺寸结果一致。实际 CSV/XML 失败及合法导入，未读取用户库。
- Problem: 浅色主题下消息较难阅读，短暂显示的错误原因尤其容易漏读。
- Why it matters: 这是可访问性及可读性问题，不声称密码泄漏或功能无法执行。
- Realistic failure scenario: 用户在浅色主题、390px 窗口中读取多行中文导入错误，颜色对比不足且提示短暂。
- Minimal fix: 正文复用 `--text-primary`，成功/错误身份继续由现有语义背景、边框与文字内容体现，不增加 token 或尺寸。
- Better long-term fix: 在其他短时反馈中复核语义色的对比度，同时独立检查自定义颜色组合；无法保证用户自选低对比度主题。
- Regression test suggestion: 真实 dark/light/custom 的 success/error 正文至少 4.5:1，保持尺寸、种类及窄屏反馈正确；type/build 不能代替颜色测量。
- Estimated effort: 0.5–1 小时（两处颜色和三主题运行复核）

### SP-S32 修复与独立复核（2026-11-10）

`34b6768` 仅将 TipsHost 正文改为既有 `--text-primary`，删除 error 的
平行文字色覆盖。背景、边框、字体、间距、定时及 TipKind 都保留。
只读 sibling clipboard 的 Toast 正文也是 primary token，借鉴其语义
边框/背景分工，未复制更大的 toast 尺寸或修改 sibling。

`tip-contrast-red.log` 18 组有 12 组不满足 4.5:1；green 18 组全部通过，
全部组合最小约 12.53:1，正文不再固定混白。18 张 green 截图及
`tip-contrast-contact-sheet.png` 已检查，真实 CSV/XML 拒绝及成功导入
仍正确，pageerror=0，独立 Vite 已停止。宽度测量字段与 viewport 分开
记录；此前 severity 日志的 width 字段被元素宽度覆盖，截图命名仍准确。

`check-tip-contrast.log` check 零错误警告、`build-tip-contrast.log` build
终态 exit 0，format:check 通过；样式低影响使用外部运行测量，未新增
永久实现镜像测试。文档 currency gate 同步 css-theming；现有完整门禁
checkpoint 是 `414e049`，未伪称本 CSS 提交已有全套 Rust 运行证据。
仅合成 demo 库，原生/设备和用户自选低对比度主题不在本次保证范围。
当前卡片 **43**（High 10 / Medium 25 / Low 8），原始基线保持不变。
长错误提示窄屏边界、其他系统/UI 和手机 Save As 真机验证继续。

### Finding: SP-S33 长文件名和 URL 提示横向越出窗口（补充发现）

- Severity: Low
- Confidence: High
- Category: Accessibility / Design
- Status: Confirmed
- Affected area: global transient tip viewport geometry
- Evidence:
  - File: `src/lib/components/TipsHost.svelte:20`、`src/lib/components/TipsHost.svelte:33`（`b8dde71` 修复前）
  - Function / Module: `.tips-host` / `.tip` fixed positioning and intrinsic text width
  - Relevant behavior: centered fixed host 没有宽度上限，长未断开的 URL/文件名抬高 intrinsic minimum width；消息自身没有可断长词规则，拖大整个 stack。
  - Reproduction: `tip-wrap-red.log` 实际 TipsHost 显示三个纯合成公开错误文本，dark/light/custom × 1100/760/390 均失败：390px 窗口里消息宽约 1422–1528px，左侧起点为 -516/-569px，失败前缀和信息两端看不见。`tip-wrap-red-light-390.png` 独立截图确认；pageerror=0，无用户库/设备/剪贴板。
- Problem: 文件路径或远端错误包含长串时，用户只能看到中间一段，无法读取操作结果及完整原因。
- Why it matters: 错误反馈需要留在当前窗口内；纯布局问题，没有数据库写入或敏感信息泄漏证据。
- Realistic failure scenario: 云端/文件错误把长 URL 或单词型文件名放入 tip，手机窄屏上裁掉“另存为失败”和结尾说明。
- Minimal fix: host 用内容宽度并限制到窗口留边与紧凑上限，单条消息遵守 host 宽度并允许长词换行；短消息保持紧凑居中。
- Better long-term fix: 独立复核超长/多条消息的高度与显示时间，并检查其他 popover 的内在尺寸边界。
- Regression test suggestion: 三主题三尺寸中所有消息边缘在窗口内、无横向文字 overflow；短消息保持原 padding/font，实际导入失败仍可读。
- Estimated effort: 0.5–1 小时（局部 CSS 和运行截图复核）

### SP-S33 修复与独立复核（2026-11-10）

`42b43cb` 将 host 设为内容宽度，上限 min(560px, 100vw−24px)；tip
尊重 host 的 100% 宽度并对长单词/URL 使用 overflow-wrap:anywhere。
现有 padding、字号、border/background、定时和 z-index 均保留。
`tip-wrap-red.log` 三主题 × 三尺寸全部横向越界；green 全部在视口
留边内且 scrollWidth 不超过 clientWidth。390px 上消息宽 366px、
左右各 12px，前缀/结尾均能读取。真实 `tip-wrap-contact-sheet.png`
九张截图已检查；测试使用公开合成错误串，未模拟真实网络上传失败。

`tip-wrap-integration-browser.log` 与 18 张截图复核实际 CSV/XML 失败
及成功导入、三主题/三尺寸，18 组对比度仍通过，pageerror=0。
`tip-wrap-integration-contact-sheet.png` 已检查，短 tip 仍紧凑居中。
两个自有 Vite 实例各自运行结束后停止。

完整 `verify-tip-wrap-integration.log` 终态 exit 0：前端 **312**、Rust
harness **561**、ACL **2**、格式/check/build/clippy 全通过；两线程，
无外部过滤，四个 live clipboard body 缺省 skip。此次集成门禁也覆盖
前一 CSS 正文颜色修复。文档 currency gate 同步 css-theming，仅四条
几何/wrap 声明，没有新 token 或永久低影响样式测试。
当前卡片 **44**（High 10 / Medium 25 / Low 9），基线 15 项/6.1 分不变。
整个目标仍 active；逐 URI/regex 交互、迁移保真、KDF 校准、其他 UI
及原生验证继续，手机 v1.6.5 云端另存为仍未有新 APK/真机证据。
超长 stack 的高度/阅读时长尚未验证，不用本次横向边界替代。

### Finding: SP-S34 迁移和本产品 CSV 导回丢失收藏标记（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / Frontend state
- Status: Confirmed
- Affected area: Bitwarden JSON/CSV normalization and bulk import metadata
- Evidence:
  - File: `src-tauri/src/vault/import.rs:36`、`src-tauri/src/vault/dto.rs:449`、`src/lib/utils/csv.ts:138`、`src/lib/services/io.ts:142`、`src-tauri/src/vault/entries.rs:298`（`64a662c` 修复前）
  - Function / Module: ImportRow / ImportEntryInput / parseCsvRows / importRowsToEntries / import_entries
  - Relevant behavior: Bitwarden favorite 和自身 CSV 的 Favorite 输出没有对应导入字段；即使 parser 或外部调用传入 favorite，serde DTO、frontend mapper 和原生插入均丢弃，重新打开库全部变为非收藏。
  - Reproduction: 三个 Rust red 回归在有效编译后失败：真实 parser→wire→KDBX 重开丢失、原生 bulk 丢失、非法 favorite 未拒绝。三个前端 red 回归分别覆盖 CSV writer→reader、结构化 mapper→bulk IPC 和 browser bulk。`import-favorite-rust-red-corrected.log` / `import-favorite-frontend-red.log`。早先测试未声明 mutable session 的 compile 错误不是产品证据。
  - Independent reference: Bitwarden 固定 commit 1a8bdbfe625bff90a9c0cabef2002e37f6038fa1 的 CipherExport 定义/写出 boolean favorite；individual exporter CSV 写 1/null，JSON 保留 favorite。公开源码 SHA256 见 `bitwarden-export-metadata-provenance.json`；本产品 buildCsv/export_csv 也写 true/false 列。
- Problem: 用户依赖的收藏筛选及标记在迁移或本产品 CSV 导回后消失，提示却报告导入成功。
- Why it matters: 这是可复现的非秘密元数据丢失；密码、条目数量与保护旗标不因本问题而改变，不夸大为整个数据库损坏。
- Realistic failure scenario: 导出 SecPivot 的一份含收藏条目 CSV，再导入新库，收藏过滤变为空；Bitwarden 迁移同样丢失 favorite=true。
- Minimal fix: 以 optional favorite 导入元数据贯通 parser、TS mapping、IPC/native batch 和 browser，原子写入既有 SecPivot.Favorite；CSV 显式 Favorite header 保留标记，普通编辑不覆盖收藏。
- Better long-term fix: 单独核查密码历史、组织集合、ZIP 附件和逐 URI；普通官方 JSON 排除 deletedDate 非空条目、ZIP 包含附件，不能把未导出的内容当 importer bug。
- Regression test suggestion: true/false/absent 在 command/wire/KDBX 和 browser 保真，非法 boolean 在写入前拒绝，旧接口默认与普通编辑保持，失败 batch 不产生部分收藏条目。
- Estimated effort: 1–2 小时（跨层导入 metadata 与真实 KDBX 回归）

### SP-S34 修复与独立复核（2026-11-10）

`ed5a86d` 将 optional favorite 从 Bitwarden/CSV parser，经 ImportRow、
IO mapper、ImportEntryInput、import_entries 一直保留到既有 KDBX 收藏
字段。Native/browser 都在创建时设置，旧 DTO 缺省和普通编辑保持原旗标；
custom 标签继续来源前缀隔离。所有分组/附件仍在插入前验证，未绕过 batch
原子边界。Bitwarden 显式 favorite 只接受 boolean；CSV 仅从显式 header
读取 true/false（不分大小写）、1/0，空值未指定，非法值整份拒绝并显示
中英本地化内容无敏感字段的提示。无新增设置、markup/CSS 或依赖。

`import-favorite-rust-red-corrected.log` 的三个实际回归全部由失败转为
通过；最初 fixture 少 mutable 修饰的编译失败独立排除。六个 targeted
favorite tests green，KDBX 保存重开、普通编辑连续性、错误 batch 不写入
和 false/absent/源 custom 名称对照均通过。前端 CSV 自身 writer→parser、
reordered flags、structured mapper→bulk IPC、browser bulk 以及非法 cell
共四项新增回归通过，原 CSV/OTP/protected-field/view 测试保留。

全新 Edge 的 `import-favorite-browser.log` 真实菜单/chooser/FileReader
确认非法 Favorite 不写入且不回显字段值，合法 true/false 两条的内存状态
和收藏图标一致；760×520、390×720 截图已检查，pageerror=0，自有 Vite
进程停止。仅合成库/字段，不作为真实 Bitwarden/Android 原生导出证据。

完整 `verify-import-favorite.log` 终态 exit 0：前端 **316**、Rust harness
**564**、ACL **2**、format/check/build/clippy 通过；两线程、无外部
过滤，四个 live clipboard body 缺省 skip。文档 currency gate 更新
数据/结构引用与当前 command 的旧“skip card”注释；TODO 只补 favorite
证据，Bitwarden 全兼容仍为 partial。当前卡片 **45**（High 10 / Medium
26 / Low 9），基线 15 项/6.1 分保持。

固定官方来源还证实普通 JSON 排除回收站记录、ZIP 才含附件，已收窄
最初 deletedDate 怀疑；密码历史确实在官方 JSON 输出，当前 mapper 未
保留，继续独立审查。完整 URI、集合/历史/ZIP、其他系统及 UI、手机
v1.6.5 Save As APK/设备验证仍未完成，目标 active。

### Finding: SP-S35 KPRPC 空正则放宽为全站匹配（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Data integrity / Concurrency boundaries
- Status: Confirmed
- Affected area: Bridge/RPC/auto-type URL allow and block regex rules
- Evidence:
  - File: `src-tauri/src/vault/helpers.rs:211`、`src-tauri/src/vault/helpers.rs:258`、`src-tauri/src/vault/helpers.rs:420`、`src-tauri/src/vault/helpers.rs:475`（`0ab8e2b` 修复前）
  - Function / Module: regex_matches / kprpc_matches_url / walk_match / walk_match_candidates
  - Relevant behavior: shared helper 把空字符串交给 Rust regex；空表达式匹配每个输入，空 allow 列表项覆盖原 host 约束，空 block 又会阻止全部合法匹配。官方 KPRPC 在调用 Regex.IsMatch 前先排除空表达式。
  - Reproduction: 真实合成 KDBX 写入并重开后，在主网址 authorized.example、regExURLs=[""] 下，不相关 unrelated.example.net 的 Bridge/RPC 与无关窗口 auto-type candidates 计数均为 1，期望均为 0；`kprpc-empty-regex-red-complete.log` 显示 (1,1,1)。空 block 回归使合法 host 查询无条目，原始 corrected log 显示 0/1 差异。未执行 OS 按键/扩展/共享剪贴板，也未读取真实密码库。
  - Independent reference: pinned KeePassRPC dc0a59b60b3ac21da9944bc5b98fe96e706af80a，KeePassRPCService.JSONRPC.cs:873/924、1334/1385 使用 !String.IsNullOrEmpty；`kprpc-empty-official-jsonrpc.cs` 与先前公开来源 hash 一致。`kprpc-empty-dotnet-oracle.json` 实际 .NET 复核 raw empty=true、官方 guard=false，单空格及明确 .* 对照保留。
- Problem: 来自现有 KDBX 的空占位 regex 被解释为全局允许，使条目出现在并不属于其网址的查询/自动输入目标中。
- Why it matters: 浏览器匹配承担秘密返回的范围限制；实际 host 回归已经返回一个合成 credential，属于已复现匹配范围扩张，不声称已存在远端利用或真实用户秘密泄漏。
- Realistic failure scenario: 在 KeePass/外部编辑工具生成的 KPRPC config 中残留空列表项；官方插件忽略它，SecPivot 却把该项当作全站规则。
- Minimal fix: 在共享 regex helper 排除真正空表达式，所有三类匹配入口同步生效；不 trim 有意义空白、不修改明确 wildcard/非空有效规则。
- Better long-term fix: 独立复核 .NET/Rust regex 方言、oversize block 行为、规则列表解析和多网址请求优先级，不能以本项修复声称全部正则互操作完成。
- Regression test suggestion: 重开 KDBX、两种 Domain 配置下无关 Bridge/RPC/auto-type 不匹配，空 block 不吞合法条目；非空空格、.*、invalid regex 和既有 block/allow 优先级仍受覆盖。
- Estimated effort: 0.5–1 小时（共享边界及真实 host 回归）

`1a146c2` 已修复共享空表达式边界。最终针对性 6 项回归全部通过，完整
`verify-kprpc-empty-regex.log` exit 0：316 frontend、567 Rust、ACL 2；
默认跳过四个原生剪贴板 live body。更新了匹配契约、安全边界和模块引用，
原始基线与评分保持不变。正则方言、过大 block、多网址请求、完整迁移、
其余系统/UI 与手机 v1.6.5 的新 APK/真机另存为仍待验证，目标 active。

### Finding: SP-S36 属性校验失败仍修改匹配规则且缓存隐藏改动（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / Stability / Configuration
- Status: Confirmed
- Affected area: Native update_entry_flags validation and snapshot invalidation
- Evidence:
  - File: `src-tauri/src/vault/entries.rs:399`、`src-tauri/src/commands/vault.rs:613`（`2c42492` 修复前）
  - Function / Module: VaultSession::update_entry_flags / update_entry_flags command
  - Relevant behavior: 原生方法先写 override_url/quality_check，再校验 foreground_color；颜色无效返回 Err，未到 mark_dirty，不刷新缓存快照或 revision。
  - Reproduction: 两个真实合成 KDBX 回归在合法属性保存后，提交新网址或清空网址、quality=false、无效颜色；都收到预期错误。直接 db.entry 检查已经改变，state() 却仍显示旧属性和 clean/revision。设置分支实际 Bridge 合法/无关网址计数变成 0/1，清空分支合法计数变为 0；显式保存并重开均保留拒绝请求的部分变更。`entry-flags-atomic-red-complete.log` exit 101，两项均失败；仅记录布尔与匹配计数。
  - Independent verification: 对比缓存 DTO、原始 KDBX entry、实际 BridgeHost 查询及保存重开的四个观察面；初次仅检查缓存的测试恰好显示旧值，随后增加原始条目检查，排除仅展示问题。
- Problem: 校验错误不是无副作用失败；实际匹配规则和质量开关发生未标脏改动，前端无法从错误或缓存察觉。
- Why it matters: 用户收到失败反馈后仍可能把部分更改写入库；dirty/revision 不能代表真实内存状态。未复现真实秘密泄漏，也不把能调用合法属性编辑的 renderer 视为新的越权来源。
- Realistic failure scenario: 原生命令收到带其他属性的无效颜色请求后，后续显式保存持久化未确认修改；当前编辑器颜色选择器通常只产生合法值，正常点击路径与真机 UI 未复现，故按受约束边界缺陷评为 Medium。
- Minimal fix: 在取得目标条目后先完成可失败的颜色校验，再写任何属性；保留 UUID/条目错误顺序及 absent/empty/valid 三态语义。
- Better long-term fix: 逐个复核其他 mutation 的校验先行、事务边界、缓存失效和错误后的持久化；本项不引入通用事务重写。
- Regression test suggestion: 设置/清空两种失败分支同时检查 raw entry、缓存、dirty/revision、匹配与 KDBX 保存重开；既有合法/absent/clear/invalid-color tests 保持。
- Estimated effort: 0.5–1 小时（局部校验顺序与持久化回归）

`d3b3264` 已先校验颜色再写入属性；针对性 4 项全部通过，完整
`verify-entry-flags-atomic.log` exit 0：316 frontend、569 Rust、ACL 2。
原始库、缓存及重开均保持拒绝操作之前的属性，合法/缺省/清空继续通过。
数据契约及模块引用同步，四个原生剪贴板 live body 默认未执行。
原始审计基线不改写；完整迁移、其他系统/UI 和 Android 真机另存为仍待验证。

### Finding: SP-S37 Bitwarden 官方密码历史导入后静默丢失（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / Privacy / Migration compatibility
- Status: Confirmed
- Affected area: Bitwarden JSON parser and imported credential source history
- Evidence:
  - File: `src-tauri/src/vault/import.rs:49`、`src-tauri/src/vault/import.rs:192`（`9c9168b` 修复前）
  - Function / Module: parse_bitwarden_json / ImportRow / import_entries
  - Relevant behavior: 解析器映射 login/fields/typed values/favorite，却完全忽略 passwordHistory；无效 history 也被当作导入成功。
  - Reproduction: 实际 parse command→ImportRow JSON→ImportEntryInput→原生 bulk→保存重开的合成库没有任何源密码历史；同名 custom field 控制保留，但源历史仍丢失。`bitwarden-history-red-corrected.log` exit 101，3 fail/1 pass；最初测试缺少 mutable binding 的编译错误属于 fixture，未计作产品失败。
  - Independent reference: 官方 Bitwarden clients 固定 commit `1a8bdbfe625bff90a9c0cabef2002e37f6038fa1`，cipher.export.ts:290–291 将 passwordHistory 映射至导出，password-history.export.ts:27–38 包含 password/lastUsedDate；证据 provenance 保存路径与 SHA256。keepass 0.13.22 History 存完整 Entry，源格式并未提供完整旧条目，不应虚构旧版本。
- Problem: 用户迁移正常未加密 JSON 时旧密码和对应时间全部消失，成功提示无法反映数据不完整。
- Why it matters: 旧密码可能用于恢复或核对历史变更；源文件存在这些数据，丢失属于已确认迁移保真缺陷。未使用真实用户库或真实 Bitwarden 导出运行。
- Realistic failure scenario: 将含非空 passwordHistory 的官方格式 JSON 导入并保存，然后删除源文件，旧密码记录无法在目标库中找回。
- Minimal fix: 将非空源数组以唯一 source-prefixed 受保护 custom field 归档，保留顺序/重复/时间/附加字段；整份拒绝无效结构，错误不含秘密。空/缺省/null 不添加无用字段。
- Better long-term fix: 提供明确的源密码历史查看/恢复界面；原生完整 Entry history 需要独立语义方案，当前归档不能冒充已经支持原生历史迁移。
- Regression test suggestion: 真实命令/wire/bulk/重开保持 Unicode、重复、空旧值和可选时间；同名字段不覆盖，DTO 不包含旧秘密，空源与无效混合文件有明确边界。
- Estimated effort: 1–2 小时（受保护归档和 KDBX 回归）；原生历史交互另行估算

`17dd196` 已将非空源历史保存在唯一受保护字段；4 项针对性回归通过，
完整 `verify-bitwarden-history.log` exit 0：316 frontend、573 Rust、ACL 2。
归档保留源数据，正常快照没有旧密码；没有添加新的 DTO/IPC 或模拟完整
历史版本。contract/security/module 引用与 partial TODO 同步，未复核成
完整 Bitwarden 兼容。四个原生剪贴板 live body 默认跳过；初始 fixture
编译失败已与有效 red 证据区分。原始基线与评分不变，目标仍 active。

后续继续检查原生历史的附件恢复、其他 mutation 的错误后副作用、完整
逐 URI/集合/ZIP/1PUX、KDF/剪贴板生命周期及其余系统/UI 覆盖。手机
v1.6.5 云端库另存为的 ACL 源码修复已有 host tests，但没有新版 APK
与 Android provider/选择器真机结果，不能标记已恢复。

### Finding: SP-S38 附件替换/删除破坏历史数据并产生无效引用（补充发现）

- Severity: High
- Confidence: High
- Category: Data integrity / Stability / Dependency correctness
- Status: Confirmed
- Affected area: Native attachment mutations and keepass history attachment ownership
- Evidence:
  - File: `src-tauri/src/vault/serialize.rs:720`、`src-tauri/src/vault/entries.rs:354`、`src-tauri/src/vault/entries.rs:1130`（`93c7953` 修复前）；keepass 0.13.22 `src/db/types/entry.rs:380/454/473/832`
  - Function / Module: sync_attachments / add_attachment_payloads / EntryMut::remove_attachment_by_name / EntryTrack::drop
  - Relevant behavior: track_changes 在 drop 才追加快照，没有保留待追加快照的附件所有权。附件移除清掉同 entry 的全部 refs，而非仅当前版本；旧数据被删除后快照仍指向旧 id。next_free 可重用该 id，历史随即指向新数据。
  - Reproduction: 合成 KDBX 先保存并重开，再通过实际 update_entry 替换/删除或 add_attachments 同名替换。`historical-attachment-red.log` exit 101：三个回归均失败；前两项访问历史附件在 AttachmentRef 内 panic，后一项内存旧值不等且保存重开仍不等，save 本身却成功。
  - Independent verification: 固定官方 KeePass PwEntry.cs commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5（SHA256 a68f2d7df7a0d49940b63387ed20634f849e60de6f4bb01c4f0ab8a4becba230）CreateBackup/CloneDeep 保留旧 Binaries。只读检查 crates.io 0.13.25 与 0.17.0，相关删除/track/drop 逻辑仍相同，不能声称升级会解决。
- Problem: 合法附件编辑后，旧历史不再保留原数据，可能指向不存在的二进制或另一个新附件。
- Why it matters: 历史恢复与审计无法依赖历史附件；命令可以在变更途中 panic，或成功保存错误历史。没有真实用户库损坏或整进程崩溃证据，不按 Critical 的不可恢复当前库损坏定级。
- Realistic failure scenario: 修改已有附件，之后查看/恢复旧版本，却读取新内容或触发无效引用；删除附件同样使旧快照失去原始数据。
- Minimal fix: 修正依赖的版本级引用和待追加历史保留，快照落地后重建实际引用；历史裁剪/删除后重新索引并只清理真正孤立数据。保留原 pinned version 与明确源码来源，不修改本机 Cargo registry。
- Better long-term fix: 将最小引用修复反馈至上游并在有真实修复的版本上移除本地补丁；恢复入口完整附件恢复仍需独立验证，不能用 catch_unwind 或忽略缺失值代替历史保真。
- Regression test suggestion: 编辑器替换/删除、详情替换、已有多个历史与共享附件、历史裁剪/删除、待追踪期间修改、KDBX 保存重开均保留旧值；回收只删除无当前/历史/待追加拥有者的内容。
- Estimated effort: 3–6 小时（依赖局部修复、保真与引用回收回归）

### Finding: SP-S39 回收站清理后 KDBX 二进制序号错位导致其他附件丢失（补充发现）

- Severity: Critical
- Confidence: High
- Category: Data integrity / Storage / Dependency correctness
- Status: Confirmed
- Affected area: keepass KDBX4 XML Binary Ref versus inner-header binary order
- Evidence:
  - File: keepass 0.13.22 `src/format/xml_db/mod.rs:56–78`、`src/format/xml_db/entry.rs:190–200`、`src-tauri/src/vault/entries.rs:980`
  - Function / Module: xml_db::to_xml / Entry::db_to_xml / empty_recycle_bin
  - Relevant behavior: 文件头按存活 AttachmentId 排序后只输出二进制内容，序号成为连续 0..N；XML Binary Ref 却继续使用原来的稀疏 id。解析器仅接受实际存在的头序号，静默忽略超出序号的引用。
  - Reproduction: 创建三个合成条目，各有独立附件；通过实际 delete_entries/empty_recycle_bin 删除中间条目，保存成功并重开后最后一个未删除条目没有附件。`sparse-attachment-ids-red.log` exit 101。回归执行时仅引用管理已补丁，XML writer 与原始 registry 文件逐字节一致，因此不是新增 writer 行为。
  - Independent verification: 独立小项目仅依赖原始 registry keepass 0.13.22，`sparse-attachment-registry-independent-red.log` exit 101 复现同样丢失；对比 header 的排序/连续枚举和 XML 旧 id，同一现象在历史删除后的保存重开回归出现。KDBX4 头的二进制位置必须与 Ref 保持一致；不把内存 AttachmentId 当作文件内序号。
- Problem: 删除操作可使未删除条目的当前附件引用失效或指向另一段内容，且保存仍返回成功。
- Why it matters: 这是正常操作触发的当前数据损坏，符合 Critical 的正常操作数据丢失/损坏条件；影响限定于附件映射，不声称密码字段丢失、整库不可解密或远程攻击。尚未接触真实用户库。
- Realistic failure scenario: 清空回收站释放中间二进制 id，随后保存并重开；后面的附件丢失，用户再次保存/回收时还可能丢弃已无法从条目访问的孤立内容。
- Minimal fix: 同一次序列化为所有 Binary Ref 建立与头数组完全相同的 dense-id 映射，包含分组递归/历史；无法映射的引用明确拒绝保存。加载时重建所有当前/历史附件反向引用，防止后续回收误删其他条目的数据。
- Better long-term fix: 上游修复并加入独立 KDBX 客户端的稀疏二进制互操作；现有局部历史修复必须连同 dense-id 写入检查，不能先发布会进一步释放 id 的版本。
- Regression test suggestion: 清理中间条目、多个稀疏 id、当前/历史与跨条目共享附件保存重开仍保持数据；缺失引用保存拒绝、旧目标保持、加载后全局回收只删除孤立数据。
- Estimated effort: 2–4 小时（序列化映射、加载索引及真实 KDBX 回归，与 SP-S38 同一完整引用不变量）

`4fbc748` 已同时修复 SP-S38/SP-S39 的完整附件引用不变量。14 项附件相关
应用测试、2 项依赖共享/guard unwind 测试通过；最终
`verify-attachment-integrity-final.log` exit 0：316 frontend、582 Rust、ACL 2，
`RUST_TEST_THREADS=1`、无外部过滤，四个剪贴板 live body 默认跳过。
局部 keepass 补丁保留原 pinned version 与 MIT license；57 文件逐字节不变，
仅 3 个 Rust 文件修改，provenance/格式 gate 通过。原始 archive 重新解压后，
用显式 `core.autocrlf=false` 应用 zero-context patch，全部 60 文件 SHA256
与 provenance 相同；初次默认行尾转换的差异不作为产品缺陷。

首轮完整门有一次 S3 100-page fixture 在第 82 页连接失败；原测试单独重跑
与最终串行 full gate 都通过，没有据此归因产品或环境。初始 fixture 编译
错误、非 workspace 测试方式和 blanket rustfmt drift 均已纠正，与有效产品
red 证据区分。未执行原生 KeePass 客户端：现存参考 ZIP 无法解压，官方
源码对照只支持格式/语义结论，不替代客户端互操作。

本提交同步 data/security/project-structure/maintenance 引用，并将整条目
历史 TODO 保留 partial：当前恢复入口仍未替换附件集合。旧损坏文件恢复、
完整历史恢复、外部附件修改快照及其余系统/UI 审计继续进行；手机 v1.6.5
云端库另存为仍缺新版 APK/选择器/provider 真机证据。原始基线和评分不变，
目标继续 active。本报告更新不改变产品契约，无额外 reference 修改需要。

### Finding: SP-S40 恢复历史版本仍保留当前附件集合（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / Testing authenticity / Domain compatibility
- Status: Confirmed
- Affected area: Native restore_entry_version and its regression expectations
- Evidence:
  - File: `src-tauri/src/vault/entries.rs:761–839`、`src-tauri/src/vault/tests.rs:1463–1472`（`141ba99` 修复前）；`src/lib/services/vault.ts:1697`、`src/lib/i18n/zh-CN.ts:154`
  - Function / Module: VaultSession::restore_entry_version / entry_history_tracks_versions_and_restores
  - Relevant behavior: 恢复命令替换 fields/attributes 却不触及 attachments；命令返回成功，UI 提示已恢复历史版本，原测试明确要求附件保留当前值。
  - Reproduction: 实际原生保存重开的合成库包含同名新/旧附件与增删附件；恢复后字段变旧，但同名附件仍为新内容。`restore-attachment-set-red.log` exit 101，1 fail；加强原测试的空附件恢复期望后，`restore-empty-attachment-set-red.log` exit 101，1 fail，新附件仍存在。旧历史仍可读取，因此没有归类为不可恢复的数据丢失。
  - Independent reference: 官方 KeePass PwEntry.cs 固定 commit `d1eec63f1bd73dc2d5273eaf94528f616b553ce5`（SHA256 `a68f2d7df7a0d49940b63387ed20634f849e60de6f4bb01c4f0ab8a4becba230`），RestoreFromBackup:607–619 调 AssignProperties，后者第 510 行 CloneDeep Binaries。未执行原生 KeePass 客户端。
- Problem: 恢复后的当前记录混合旧字段和新附件，成功提示与实际恢复范围不一致。
- Why it matters: 用户依靠历史回滚附件变更时会得到错误版本；绿测试固定了不完整行为，不能证明恢复保真。影响是已确认的恢复功能缺口，不是远程攻击或旧数据必然丢失。
- Realistic failure scenario: 把附件从旧合同改为新合同，选择旧历史恢复后保存重开，字段显示旧版本，但附件仍是新合同；或恢复无附件版本后附件仍残留。
- Minimal fix: 变更前在原生会话复制历史 binary Values，保留 protection flags，再替换完整附件集合；追踪 scope 保留恢复前附件供撤销，现有历史限制继续生效。
- Better long-term fix: 独立检查所有历史 mutation 和保留边界；格式/源码对照不替代原生客户端互操作和 UI 运行验证。
- Regression test suggestion: 同名替换、旧附件被移除、新附件新增、Unicode/空二进制、双 protection flags、空集合恢复、限制为 1 时清理所选历史、保存重开与撤销恢复均保持准确值；普通 DTO 只带附件名字/大小。
- Estimated effort: 1–2 小时（原生恢复、KDBX 回归及契约同步）

`b5bec5c` 已替换历史恢复的完整附件集合，保留原 protection flags；
恢复前附件进入 undo 快照，原数量/大小限制继续应用。27 项历史相关
回归与完整 `verify-restore-attachment-set.log` exit 0 通过：316 frontend、
583 Rust、ACL 2，串行且无外部过滤，四个 live clipboard body 默认跳过。
新增恢复回归覆盖数量上限 10/1、同名变更/新增/移除、Unicode 空附件、
保护/非保护标记、保存重开与撤销；没有真实用户库、剪贴板、原生客户端
或手机运行证据。data/security 引用与 partial TODO 同步。

中途两个更严格的历史清理期望失败：当前源码将数量 0 当无限且大小
不足时仍保留最新一版；官方源码语义不同。这是下一项独立兼容性复核，
不能由附件恢复通过推断这些边界已修复。本报告提交不改变契约，无新
reference 修改需要；基线与评分保持不变，目标仍 active。

### Finding: SP-S41 历史保留的零值与严格大小上限未执行（补充发现）

- Severity: Medium
- Confidence: High
- Category: Privacy / Configuration / Data retention / Testing authenticity
- Status: Confirmed
- Affected area: Shared native history_cap/history_limits/trim_entry_history_by_size
- Evidence:
  - File: `src-tauri/src/vault/serialize.rs:384–452`、`src-tauri/src/vault/entries.rs:28–36`、`src-tauri/src/vault/tests.rs:2461–2499`（`80c297a` 修复前）
  - Function / Module: history_cap / history_limits / trim_entry_history_by_size
  - Relevant behavior: 数量 0 被映射为 usize::MAX；大小 0 被当作无限；大小裁剪始终至少保留最新一版。旧测试明确要求数量 0 保留全部历史，并对小预算强行访问最新版本。
  - Reproduction: 合成库通过实际 update_database_settings 配置、保存重开，再用 update_entry 替换旧密码和附件；数量 0、大小 0、大小 1 三个边界仍有历史。`history-retention-limits-red-corrected.log` exit 101，3 fail；首轮缺少设置后 save 导致条目不存在，属于 fixture，未用于产品结论。
  - Independent reference: 固定 KeePass PwEntry.cs commit `d1eec63f1bd73dc2d5273eaf94528f616b553ce5`，MaintainBackups:659–684 对 count/size >=0 执行删除直到不超限，没有最少一版例外；源 SHA256 `a68f2d7df7a0d49940b63387ed20634f849e60de6f4bb01c4f0ab8a4becba230`。未运行原生 KeePass binary。
- Problem: 用户或外部 KeePass 库配置禁止/限制历史后，SecPivot 仍将已经替换的旧密码和附件保留在加密库中，大小限制也可能被单个快照突破。
- Why it matters: 保留行为违反可见设置与格式语义，扩大旧敏感数据留存；没有已发生的对外泄露、磁盘秘密泄漏或进程 OOM 证据，因此按 Medium 定级。
- Realistic failure scenario: 用户将 HistoryMaxItems 设为 0 禁用历史，然后改掉旧密码；保存重开后仍可访问旧版本。或从设置 HistoryMaxSize=0 的 KeePass 库继续编辑，旧快照仍存在。
- Minimal fix: 数量/大小非负值按精确上限执行，0 清理所有快照，负数保留无限语义；允许删除超出预算的最新一版，历史重建后回收仅失去所有者的旧附件。
- Better long-term fix: 单独复核缺省大小、metadata 更新的清理时机、每个 mutation 入口及原生客户端互操作；不将历史清理描述为磁盘安全擦除。
- Regression test suggestion: 真正设置/保存重开的 count=0、size=0、size 小于单快照，旧历史/旧附件移除且当前密码/附件保持；负数保留多个版本；实际存储总大小 <= 预算；恢复时 count=0 仍保留恢复目标内容但不制造 undo 历史。
- Estimated effort: 1–2 小时（共享规则、KDBX 回归和契约同步）

`653ef66` 已修复零值与严格大小保留规则。4 项新增原生回归、纠正原
数量 0/最少一版假设的测试，以及 cap=0 恢复附件回归通过；31 项针对
历史的检查通过。完整 `verify-history-retention-limits.log` exit 0：
316 frontend、587 Rust、ACL 2，串行无外部过滤，四个剪贴板 live body
默认跳过。原生设置/保存重开后执行实际编辑，旧历史/旧附件释放，当前
密码/附件保持；负数无限也保存重开保持多个版本。

大小不足时，原历史测试仍索引最新版本而失败；改为核对实际累计大小
<= 预算并只在仍有版本时检查顺序。首轮 red fixture 未保存设置后关闭
丢掉新条目，未用于产品结论；有效 red 在补齐 save 后三项均复现。
此次只改共享保留规则，没有全库主动清理、依赖/IPC/UI 结构变更；
data/security/DTO 注释和 partial TODO 同步，未把修复称作磁盘安全擦除。
本报告提交没有新的产品契约变化，无额外 reference 修改需要。

下一步独立复核外部附件修改回导的历史快照、settings 的 JSON null 重置
边界、缺省大小及其余系统/UI/迁移能力；这些候选不加入 Confirmed 统计。
手机 v1.6.5 云端库另存为已有 ACL 源码修复，仍缺新版 APK 与系统
选择器/provider 真机验证。原始基线、评分和未验证的平台边界保持不变，
全面目标继续 active。

### Finding: SP-S42 外部附件修改回导不生成历史快照（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / History / Testing authenticity
- Status: Confirmed
- Affected area: Native external-viewer attachment reimport
- Evidence:
  - File: `src-tauri/src/vault/entries.rs:1125–1140`、`src-tauri/src/commands/vault.rs:853–875`、`src/lib/services/vault.ts:1400–1419`、`src-tauri/src/vault/tests.rs:5252–5320`（`77de788` 修复前）
  - Function / Module: import_attachment_bytes / import_attachment_from_temp / importAttachmentFromTemp
  - Relevant behavior: 外部查看器回导直接移除/新增附件，没有 EntryTrack、last_modification 更新或 history count/size 裁剪；已有普通编辑/详情 add 路径则使用这些共同规则。
  - Reproduction: 先保存重开合成 KDBX，再从实际 session-bound temp store 读回修改内容；加强原测试后发现历史数量 0 而预期 1，`external-attachment-snapshot-red.log` exit 101；已有历史情况下回导后仍只有旧快照而没有修改前当前版本，`external-attachment-retention-red.log` exit 101。未启动真实外部编辑器/Tauri handler，未接触用户文件。
  - Independent reference: 路由与 token reader/原生 mutation 逐层对照；普通 add_attachments 的相同附件替换已由 EntryTrack 回归验证。固定官方 PwEntry.cs 的 CreateBackup/CloneDeep（commit `d1eec63f1bd73dc2d5273eaf94528f616b553ce5`）保留 Binaries，支持一般 backup 数据语义，但不冒充原生外部编辑器运行证据。
- Problem: 外部编辑附件这一正常入口成功替换内容，却无法通过条目历史恢复修改前版本；修改时间和保留规则也与普通编辑不一致。
- Why it matters: 用户依赖每次修改快照的承诺回滚附件时，这个入口留下缺口；已有绿测试只验证新内容保存。没有未删除其他当前条目损坏或外部泄露证据，因此为 Medium。
- Realistic failure scenario: 在外部编辑器误改附件，确认回导后保存，再尝试恢复修改前版本；历史里没有这次编辑前的内容。
- Minimal fix: 在同一原生会话用 EntryTrack 包住附件替换，更新 last_modification，scope 结束后应用共享数量/大小限制。保留 token/session reader 与 IPC schema。
- Better long-term fix: 逐个审查会修改 Entry 的命令入口及编辑提交过程，复核所有历史规则和跨客户端互操作，不把这两个入口通过视为每次修改完整覆盖。
- Regression test suggestion: 实际 temp store 读取、保存重开保留修改前附件且可恢复；已有历史与 cap 10/1/0、size 小于单快照、旧时间快照及新修改时间、禁用历史时旧无拥有者 binary 回收。
- Estimated effort: 1–2 小时（原生追踪接线、保存重开及契约同步）

`3530ee8` 已接入外部附件回导的 pre-edit snapshot、修改时间与共享保留
规则。30 项附件回归通过；完整 `verify-external-attachment-snapshot.log`
exit 0：316 frontend、588 Rust、ACL 2，串行无外部过滤；四个原生
剪贴板 live body 默认跳过。实际 temp store 文件读取与 KDBX 重开验证
旧附件可恢复，cap 10/1/0、size 不足时清理正确，时间快照保持旧值。
未启动原生外部编辑器/Tauri handler；data/security 引用和 partial TODO
同步，没有新 IPC/依赖/UI 变更。本报告更新没有新的 reference 修改需要。

公开 v1.6.5 ARM64 APK 已只读下载，SHA256
`754228d99d293363685c1045b09c3b707737825dcba44d740102736c99cc99be`
与官方 SHA256SUMS-android.txt 一致；native library SHA256
`eb64275194f42b78b86e825778a7124bc684a09bc4fb4a92a97f5872fa92ac11`。
完整 literal 检查找到 plugin:saf 的 create/open_document，没有
plugin:safe|create_document；初次通用 regex 拼接到下一字串已纠正。
assets/tauri.conf.json 的 capabilities=[] 与 grant 名称字符串缺失，
单独都不能证明 ACL 未授予。v1.6.5 tag 的 android capability 使用
screens 而非 windows，接着核对框架解析/范围。没有构建、安装或发布
新版 APK，手机系统选择器/provider 真机证据仍缺失；该研究不加入新
Confirmed 卡片。原始基线与评分不变，目标 active。

### Finding: SP-S43 设置的 JSON null 重置被当作空补丁（补充发现）

- Severity: Medium
- Confidence: High
- Category: Configuration / Backend API / Testing authenticity
- Status: Confirmed
- Affected area: DatabaseSettingsPatch nullable fields at Tauri JSON boundary
- Evidence:
  - File: `src-tauri/src/vault/dto.rs:346–368`、`src/lib/components/DatabaseSettingsDialog.svelte:107–115`、`src-tauri/src/vault/session.rs:24–39`、`src-tauri/src/vault/tests.rs:838–849`（`e11eeb3` 修复前）
  - Function / Module: DatabaseSettingsPatch / save / update_database_settings / apply_database_meta_patch
  - Relevant behavior: 四个 Option<Option<T>> 仅有 serde(default)，JSON null 被折叠为外层 None，is_empty 将全 null 补丁视作无操作。前端清空输入传 null，原 Rust 直接构造 Some(None) 测试绕过真实解码。
  - Reproduction: 实际 DTO 从 JSON 的 historyMaxItems/historyMaxSize/recycleBinEnabled/entryTemplatesGroup:null 解码全为 None；`settings-null-wire-red.log` exit 101。已配置/保存重开的合成库提交这个实际 JSON 补丁后仍返回未修改状态，`settings-null-reset-native-red.log` exit 101。后者失败于 dirty 断言，修复前没有跑完其最终保存重开断言，控制流与解码结果支持 no-op 结论。
  - Independent verification: 前端实际 save 的空输入分支明确生成 null；会话 metadata setter 只执行外层 Some，原 icon DTO 已使用专门解码器避免相同问题。新解码后的原生 metadata 与存储重加密组合均另行保存重开验证。
- Problem: 用户清空历史数量/大小上限或模板组并保存，界面关闭但旧设置继续保留；显式 API recycle-bin 默认重置同样被忽略。
- Why it matters: 成功反馈掩盖未执行的配置更新，实际 IPC 不满足声明契约；旧测试证明不了 null 线路。没有已发生秘密泄漏、数据库损坏或恶意攻击证据，因此 Medium。
- Realistic failure scenario: 用户将历史版本上限输入清空以恢复默认，保存后再次打开设置，旧数字仍在；清空模板组无法解除模板绑定。
- Minimal fix: 四个 nullable 字段用同一 typed 反序列化 helper 保留外层 Some；缺省仍 None，null 为 Some(None)，普通类型值为 Some(Some(value))，错误类型保持拒绝。
- Better long-term fix: 所有声明三态/部分更新契约的测试都应至少穿过真实 JSON 解码和实际状态持久化。此轮搜索发现其他 Option<Option<T>> 仅 icon，已有独立 icon decoder；不引入宽泛的字符串/数字猜测转换。
- Regression test suggestion: 缺省/null/普通值、四个字段的实际 wire 解码、错误类型、原生设置与 compression 重写组合保存重开、未涉及的 KDF/cipher 保持。
- Estimated effort: 1–2 小时（nullable helper、native wire/KDBX 回归、契约与 TODO）

`8887e2e` 已修复四个 nullable 设置的实际 wire 重置语义；3 项新增回归
覆盖类型/缺省/null、12 种错误类型及 metadata-only/压缩重写组合保存
重开。11 项设置相关检查通过；完整 `verify-settings-null-wire.log` exit 0：
316 frontend、591 Rust、ACL 2，串行无外部过滤，四个剪贴板 live body
默认跳过。data/security 引用与 TODO 的真实 JSON 证据已同步；没有新增
DTO 字段、IPC/UI/依赖变化。本报告提交没有新的 reference 修改需要。

手机 ACL 根因进一步复核：公开 v1.6.5 tag 指向
`006b810a349f7c2358b9bb775e91091e7bba5bdb`，capability 使用 screens；
同标签锁定的 tauri 2.11.5 / tauri-utils 2.9.3 只解析 windows/webviews。
独立同版本 resolver 小项目 `saf-v1.6.5-source-resolver-check.log` 2 pass，
旧写法的 create/open grants 均有空窗口/WebView 范围，当前写法允许 main。
同版本 RuntimeAuthority::resolve_access 第 459–460 行明确要求至少一项
窗口或 WebView glob 匹配，空范围不能授权。与经过 SHA256 校验的公开
APK 调用 literal 一起，支持发行源 ACL 问题及当前源码修复；没有逆向
验证 native binary 中的授权对象，也没有用户实际安装包哈希、真机选择器
或 provider I/O。没有构建/安装新版 APK、验证签名或发布。初次 GitHub
CLI 未登录已用公开 API 继续；未加引号的 PowerShell Git peel 参数错误
已用确切引用重读，不用于产品结论。该研究复核既有 ACL 发现，不增加
新卡片统计。后续历史提示文案、其他 mutation、缺省保留大小和剩余
系统/UI/竞品迁移覆盖继续进行；原始基线与评分保持不变，目标 active。

### Finding: SP-S44 历史页固定数量提示与数据库保留设置冲突（补充发现）

- Severity: Low
- Confidence: High
- Category: Documentation / UI / Configuration
- Status: Confirmed
- Affected area: EntryDetail history hint in Chinese and English
- Evidence:
  - File: `src/lib/i18n/zh-CN.ts:204`、`src/lib/i18n/en.ts:209`、`src/lib/components/EntryDetail.svelte:1352`（`34e466a` 修复前）；`src-tauri/src/vault/entries.rs:30–34/839–841`
  - Function / Module: detail.historyHint / history_limits / restore_entry_version
  - Relevant behavior: 两种语言都写死“最多 10 个版本”并保证恢复被记为新版本；实际数量/大小可配置，负数无限、零值禁用，恢复前快照也会按限额裁剪。
  - Reproduction: headless Edge 挂载真实 EntryDetail，合成浏览器会话和 12 个 mock 历史条目；`history-hint-browser-red.log` 与 12 张 baseline 截图同时显示 12 行和“最多 10”提示。仅历史读取返回合成数据，页面组件和主题/语言服务为真实代码。
  - Independent verification: 已通过的原生 SP-S41 数量/大小与 cap-zero restore 回归证明可配置语义；本项不把 browser mock 当作原生历史生成证据。
- Problem: 提示错误地宣称固定上限和无条件恢复留档，与用户保存的数据库设置矛盾。
- Why it matters: 用户难以判断保留设置和恢复后的可撤销范围；缺陷集中在辅助文案，不影响保存行为，因此 Low。
- Realistic failure scenario: 数据库设置无限历史，列表超过 10 条仍提示最多 10；设置零历史后恢复一个旧版本，文案承诺的新快照实际不保留。
- Minimal fix: 两种语言描述数据库数量与大小限制，并明确恢复前版本也受限制，沿用现有 history-hint 样式。
- Better long-term fix: 涉及可配置行为的辅助文字与已验证域语义同步，避免将默认值当作固定限制。
- Regression test suggestion: 真实组件在中英文、dark/light/custom、桌面/窄屏下显示新文字，保留超过默认数量的列表且无横向溢出；低影响文案修正不新增只断言相同字符串的镜像测试。
- Estimated effort: 0.5–1 小时（文案、主题 gate、浏览器与文档验证）

`a88ff62` 已修复中英文提示，css-theming 引用同步了可配置文案规则；
没有 CSS/markup/token/IPC 改动。`history-hint-browser-green-fresh.log`
12 组检查通过，三主题 × 两语言 × 1100×720/420×600 的 12 张截图
已检查。check 零错误警告、build、316 frontend 和 format 均通过。
最新完整 Rust/ACL 门仍为 `8887e2e`（591 Rust / ACL 2），本次只改
文案，未重复声明原生验证。初次夹具错误使用不存在的 appSettings.update；
修复后的 baseline 有效。文案改动后的热更新服务器出现组件未调用所
mock 的 service（calls=0），重启同一开发服务器后的完整检查通过；
失败日志保留，不计产品缺陷。截图合成的缺少 sharp 和 Python 默认编码
错误改用 bundled Pillow/显式 UTF-8 修正，不用于产品结论。临时服务器
均已通过自己的 session handle 停止；未访问用户库、系统剪贴板或安卓真机。
没有相关完整 TODO 可新勾选。本次报告无需额外 reference 修改。

当前详细卡片 **55**（Critical 1 / High 12 / Medium 32 / Low 10，全部
Confirmed），基线 `b7c5d9f` 的 15 项/6.1 分保持。手机 v1.6.5 云端
另存为的发行源 ACL 根因与当前源码修正已验证，替换包与真机/provider
仍缺证；整体系统/竞品/界面审计继续，目标 active。

### Finding: SP-S45 缺省历史大小与“恢复默认”使用无限预算（补充发现）

- Severity: Medium
- Confidence: High
- Category: Configuration / Privacy / Data retention / Interoperability
- Status: Confirmed
- Affected area: history_limits default byte budget
- Evidence:
  - File: `src-tauri/src/vault/entries.rs:28–35`、`src-tauri/src/vault/dto.rs:336`、`src/lib/components/DatabaseSettingsDialog.svelte:110–111/216–222`（`c7ffe08` 修复前）
  - Function / Module: history_limits / update_database_settings / import_attachment_bytes / update_custom_field
  - Relevant behavior: history_max_size 缺省映射为 -1 无限；设置空值却标为默认，并发送 JSON null 清除 metadata。数量缺省为 10，不能限制大附件造成的累计历史字节数。
  - Reproduction: `history-default-size-red-final.log` 两个真实原生回归均失败。先设置无限、产生 4 MiB 快照、实际 JSON null 重置/保存重开，再产生第二份 4 MiB 快照，历史为 2 而预期 1；新库缺省大小保留超过 6 MiB 的单份快照，历史为 1 而预期 0。两个 red 都在首个保留断言停止，未执行各自最终保存重开/显式负数对照；这些在 green 另行完成。
  - Independent reference: 固定 dlech/KeePass2.x commit `d1eec63f1bd73dc2d5273eaf94528f616b553ce5`。PwDatabase.cs:47–48 的默认为 10 条/6*1024*1024 字节，93–94/569–570 初始化与 Clear 恢复这些值；KdbxFile.Read.Streamed.cs:265–268 仅在相应 XML 元素出现时覆盖。PwEntry.MaintainBackups 使用此预算，与此前 SP-S41 的固定源码一致。`keepass-default-history-provenance.json` 记录三个确切 URL/文件/SHA256，PwDatabase 为 `3470bf8193e95d211d422eafba901a39a68f2a4d796ce11a0e44778d87ad01b0`。未运行官方客户端。
- Problem: 空设置或重置默认后，旧敏感数据实际采用无限大小保留，偏离界面和 KeePass 默认契约。
- Why it matters: 旧附件/密码留存超过默认预算，也可能扩大加密库及同步体积。只复现预算超限，没有真实内存耗尽、外泄或安全擦除证据，因此 Medium。
- Realistic failure scenario: 含大附件的条目在默认设置下反复修改，每个旧版本都被保留，累计超过 6 MiB；从无限改为空值恢复默认后继续编辑，旧无限规则仍生效。
- Minimal fix: 在共享 history_limits 中以 6 MiB 作为缺省大小；显式负数仍无限，零值和指定正值规则保持。仅后续已有 tracked mutation 执行裁剪，不在 load 或 metadata-only 更新时主动清理整个库。
- Better long-term fix: 继续核对所有 mutation 入口与跨客户端保留语义；默认值、设置文案和原生实现一同验证，避免只验证 metadata null 能保存。
- Regression test suggestion: 合成 KDBX 实际 JSON reset/save/reopen 后多个 4 MiB 快照裁剪最旧内容，当前与最新历史附件准确；超过 6 MiB 单份快照在缺省时删除、显式负数时保留；既有数量零值、大小零值和自定义正值继续通过。
- Estimated effort: 1–2 小时（共享默认、原生持久化/大附件回归、契约同步）

`c45d7b9` 已修复缺省预算，2 项新增原生回归及 27 项历史相关检查
通过。`verify-history-default-size.log` 完整门 exit 0：format、check 零
错误警告、150-key 契约、build、316 frontend、593 Rust、ACL 2、clippy
全部通过。此轮采用默认 Rust 测试调度（RUST_TEST_THREADS 未设置），
无外部过滤，四个 OS clipboard live body 的显式开关保持关闭，不算真实
剪贴板执行。重复保存重开验证最新/当前附件的完整合成字节、过时附件
回收和显式无限对照；当前大附件没有删除。data/security 引用、DTO 注释
与 partial TODO 同步；其余 mutation 和跨客户端证据未因此标完成。
初次夹具的 Option 解包与临时 borrow 编译错误修正后才取得有效 red；
它们不是应用缺陷。早期尝试读取既有参考 ZIP 发现其不是合法 ZIP，
随后使用固定公开源码；没有从该文件推导默认值。没有 UI/IPC/schema/
依赖变更、用户库/系统剪贴板/原生客户端/Android provider 运行。
本次报告提交无需额外 reference 修改。

当前详细卡片 **56**（Critical 1 / High 12 / Medium 33 / Low 10，全部
Confirmed）；基线 15 项/6.1 分保持。最新完整应用门包括历史文案修复
和 6 MiB 默认修复；手机替换 APK/provider、其余系统/UI/竞品迁移
继续审查，整体目标 active。

### Finding: SP-S46 独立条目属性修改沿用旧时间戳，产生单边合并伪冲突（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Backend API / Merge integrity
- Status: Confirmed
- Affected area: update_entry_flags and update_entry_autotype
- Evidence:
  - File: `src-tauri/src/vault/entries.rs:405–447/1272–1305`、`src-tauri/src/commands/vault.rs:613–626/926–937`、`src-tauri/src/vault/merge.rs:351–357`（`c52fb8a` 修复前）
  - Function / Module: update_entry_flags / update_entry_autotype / merge_databases
  - Relevant behavior: 两个独立原生命令直接写属性并 mark_dirty，但不修改条目 Times.last_modification。合并按条目时间识别更新，两侧时间相同且内容不同会报同一时间戳冲突。
  - Reproduction: `entry-attribute-tracking-red-corrected.log` 两个原生合成回归均在首个单边合并断言失败。固定旧时间的共同 KDBX 基线，一边分别改 OverrideURL/QualityCheck/前景色或 Auto-Type，保存重开，与未修改副本仍无法合并。最初夹具 borrow 编译错误修正后才得到有效 red。
  - Independent verification: commands 层直接转发这两个方法；`useEntryEditor.svelte.ts:154–163` 的常规编辑路径先执行 updateEntry，已经负责 primary 字段快照与时间，本项确认的是独立属性接口。固定官方 KeePass PwEntryForm.cs commit `d1eec63f1bd73dc2d5273eaf94528f616b553ce5`，1095–1117 属性赋值后 Touch(true,false)，1124–1127 无实际改变则恢复旧修改时间。`entry-attribute-time-reference-provenance.json` 记录确切 URL/SHA256 `407388e73f47281e09ec32c4d660335822179b33e05874cb77ca3db25b04358e`；未运行原生 KeePass。
- Problem: 原生属性修改已经持久化，条目仍呈旧修改时间；合并将正常单边变化视为冲突，要求覆盖或下载。
- Why it matters: 破坏独立 API 的同步与修改时间契约，阻断自动合并。复现的是伪冲突，不是已经覆盖远端或丢失密码，按 Medium。
- Realistic failure scenario: 独立调用已暴露的条目属性命令改存量记录后保存；另一份同库副本未改变，合并仍显示同一时间戳冲突。
- Minimal fix: flags 在校验全部通过后比较实际属性；Auto-Type 比较规范化后的完整配置。值实际改变才将条目修改时间设为当前秒精度，重复相等值保留原时间；现有 dirty/revision 与历史安排保持。
- Better long-term fix: 独立复核分组与收藏 mutation、秒精度重复修改和完整编辑器事务；单命令属性历史与一次编辑器保存的 primary 快照需一起设计，避免额外中间快照改变恢复范围。
- Regression test suggestion: 两条真实 API 分别修改/保存重开，在两种 local/remote 方向合并都保留新属性；修改时间从固定旧值更新。先保存规范化属性后固定旧时间，再提交相同带空格输入，时间保持，避免秒精度掩盖无条件刷新。既有无效颜色原子拒绝回归继续通过。
- Estimated effort: 1–2 小时（两条时间更新、KDBX/merge 回归与契约同步）

`5687202` 已修复独立属性时间戳。2 项原生回归通过；
`entry-attribute-time-green-final.log` 覆盖双向合并、保存重开及
规范化相等输入的时间保持。完整 `verify-entry-attribute-time.log`
exit 0：316 frontend、595 Rust、ACL 2、format、check 零错误警告、
150-key 契约、build、clippy 全部通过；RUST_TEST_THREADS=1，无过滤，
四个 OS clipboard live body 开关关闭。最后仅补充秒精度说明，格式门
再次通过。data-contracts 与源码注释同步，没有可因此勾选的完整 TODO。
初版 red 夹具还包含历史/恢复断言，但在首个 merge 断言停止，没有
执行它们；最终回归按时间修复收敛，未声称属性独立快照已补齐。
常规编辑器的 primary 快照安排继续保留。没有 UI/IPC/schema/依赖
改动、用户库/系统剪贴板/真实 Tauri handler/Android provider 运行。

同一秒内实际变更仍可能无法由时间区分，属于现有 KDBX 精度与 merge
规则的下一项独立复核；不将本次旧时间修复宣称为所有合并边界完成。
分组/收藏时间与其他 mutation、竞品迁移和手机替换包/provider 证据
继续审查。本报告无需额外 reference 修改。当前详细卡片 **57**
（Critical 1 / High 12 / Medium 34 / Low 10，全部 Confirmed），基线
`b7c5d9f` 的 15 项/6.1 分保持，整体目标 active。

### Finding: SP-S47 分组内容修改保留旧时间戳，阻断单边合并（补充发现）

- Severity: Medium
- Confidence: High
- Category: Stability / Backend API / Merge integrity
- Status: Confirmed
- Affected area: rename_group / set_group_icon / update_group_meta / update_group_autotype
- Evidence:
  - File: src-tauri/src/vault/entries.rs:1070–1137/1330–1356、src-tauri/src/vault/dto.rs:519–528（a7d19e9 修复前）
  - Function / Module: Group content mutations / merge_databases
  - Relevant behavior: 名称、图标、备注、标签、搜索和 Auto-Type 写入未刷新 Group.times.last_modification；merge 对相同时间的不同内容报错。
  - Reproduction: group-content-time-red-final.log 的 8 项实际合成 KDBX 回归均失败。六项 nested/root 名称、图标、metadata、Auto-Type 保存重开后在首个合并断言失败；两项仅改标签在保存重开的时间断言失败。red 未执行后续反向合并与相等输入断言。
  - Independent verification: keepass-0.13.22/src/db/merge.rs:315–359 以修改时间决定胜出分组，时间相等且内容不同拒绝；实际 GroupAutoTypeDialog.svelte:65 用省略 enabled 请求继承。原 DTO/引用的“absent keeps”描述错误，原后端缺省/null 恢复继承的行为应保留。
- Problem: 持久化成功的分组变更仍携带旧修改时间，普通单边更新被当作冲突。
- Why it matters: 阻断存量分组的自动同步；本项复现伪冲突，没有密码丢失证据，按 Medium。
- Realistic failure scenario: 一份共同库只改分组名称或自动填充设置，保存后与未改变的副本合并仍失败。
- Minimal fix: 四个 mutation 比较规范化后的真实内容，改变时设置当前秒精度修改时间，相等值保持旧时间。root metadata/Auto-Type 共享同一处理；保留 enabled 缺省/null 继承和序列缺省保持/空值清除。
- Better long-term fix: 分别复核标签合并保真、收藏时间、秒精度重复变化及分组移动/展开边界。
- Regression test suggestion: 固定旧时间的原生库，实际 API 修改/保存重开后双向合并检查内容；只改标签先检查时间和直接存储。保存规范化属性后再固定旧时间，相等输入不得刷新。root/nested × JSON missing/null enabled 四场景检查继承和后代实际序列。
- Estimated effort: 2–3 小时（时间修复、九个回归与契约同步）

5f85f45 已修复。group-content-time-green-corrected.log 的 8 项通过，
group-inheritance-wire-green.log 的 1 项包含四个 JSON/原生继承组合。
完整 verify-group-content-time-final.log exit 0：316 frontend、604 Rust、
ACL 2，格式、check、契约、build、clippy 均通过；RUST_TEST_THREADS=1，
无过滤，四个 live clipboard body 关闭。第一次完整门仅因引用 Markdown
格式失败，Prettier 修正后最终完整门通过。初版夹具编译错误已排除；
第一补丁误落到 add_group 后已恢复并改到 rename_group，最终回归通过。

文档门更新 data-contracts 与 DTO/backend 注释，没有改变 enabled 的
既有继承语义，没有可据此勾选的完整 TODO。标签两项只证明时间和直接
保存重开，不能证明合并标签保真：独立临时 group-tags-merge-red.log
的 root/nested 两项仍在首方向 tags 断言失败，将作为独立问题修复。
无 CSS/markup/schema/IPC/vendor 修改、真实设备/provider/用户库运行。
当前卡片 58（Critical 1 / High 12 / Medium 35 / Low 10），原始
b7c5d9f 的 15 项/6.1 分保持，整体目标 active。

### Finding: SP-S48 分组标签在合并胜出时遗漏，新建带标签分组产生伪冲突（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data Integrity / Synchronization / Dependency behavior
- Status: Confirmed
- Affected area: Native group merge, root metadata and newly created subtrees
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/merge.rs:188–202/345–359、src-tauri/src/vault/merge.rs:389–393（bc66fc3 修复前）
  - Function / Module: keepass merge_groups / SecPivot merge_databases
  - Relevant behavior: 原始依赖在 Created/Updated 两个分组属性复制分支均遗漏 tags。Updated 成功却保留旧标签；Created 随后比较新建的目标分组与源分组，因标签不同且时间相同而拒绝。
  - Reproduction: group-tags-merge-red-corrected.log 共 4 failed / 2 passed。root/nested 更新在首方向 tags 断言失败；清空标签仍是 old-tag，停在 nested 的首次检查；新增带标签子树在首次 merge 返回同时间戳冲突。red 未执行反向、最终重开或后续 root-clear 断言。初次保存 helper 参数顺序编译错误已修正，不计产品证据。
  - Independent verification: 单独项目原始 registry keepass =0.13.22（Cargo.lock 为公开 archive checksum a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e，开启 _merge，没有应用补丁）的两个实际 save/reopen/merge 测试均失败：更新后 [] 而源为 synthetic-tag，新建带标签组 merge 拒绝。group-tags-registry-independent-red.log 与 group-tags-reference-provenance.json 保存来源。固定官方 KeePass PwGroup.cs commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5:503–544 的 AssignProperties 显式复制 m_lTags；SHA256 1836977793c3f634bb94e2d3cd2b669f709ae0519bd80a0761b565ed245066d2。未运行官方客户端。
- Problem: 成功合并遗漏较新分组标签；清空意图被撤销；新建带标签分组无法正常合并。
- Why it matters: 破坏元数据保真和分组同步。本次没有密码、附件或用户真实库丢失证据，按 Medium。
- Realistic failure scenario: 一台设备给分组添加/清空标签，另一台合并后仍显示旧标签；新建标签分类分组后同步直接提示同时间戳冲突。
- Minimal fix: 第一方临时 source clone 仅对 destination 尚不存在的分组清空标签，避免依赖新增分支的立即伪冲突；合并成功后按实际 Group Created/Updated 日志为仍存在的胜出分组复制 original remote tags，包含空集合。现有同时间戳冲突、本地较新版本、删除及回收站决策保持。
- Better long-term fix: 上游修正两个分组复制分支后用相同回归复核，再考虑去掉第一方兼容层。不要仅因版本升级或编译通过认为数据保真已恢复。
- Regression test suggestion: root/nested 修改和清空标签，真实 API/save/reopen 后双向合并，再保存合并库重开检查标签与时间。新建父子分组保留各自标签和父子关系；两个方向确认同时间戳差异仍拒绝、删除的带标签分组不复活。
- Estimated effort: 2–3 小时（兼容层、六个标签场景、原始依赖复核和完整门）

3eb8194 已修复。group-tags-merge-green-final.log 的 8 项通过：
六项标签合并/边界场景与两个已有标签回归，其中四项为新增测试、
两个既有时间测试扩展到真实合并/合并库重开。完整
verify-group-tags-merge.log exit 0：316 frontend、608 Rust、ACL 2，
format、check 零错误警告、契约、build、clippy 全部通过；Rust 单线程，
无过滤，四个 live clipboard body 关闭。依赖 provenance gate 仍为
60 文件/3 reviewed patches/57 upstream byte-identical，没有 vendor 编辑。
data-contracts 已同步合并契约，无可据此勾选的完整 TODO。
没有用户库、真实 Tauri handler/官方 KeePass 客户端/Android provider
或替换 APK 运行。源码修复不是手机已安装 v1.6.5 的现场验证。

当前详细卡片 59（Critical 1 / High 12 / Medium 36 / Low 10），
原始 b7c5d9f 的 15 项与 6.1 分保持。整体目标 active：继续独立
复核收藏修改时间/缓存、秒精度历史冲突、其余系统/UI/竞品迁移。

### Finding: SP-S49 收藏切换沿用旧修改时间，轻量返回与缓存缺少变更元数据（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data Integrity / Frontend State / Backend API
- Status: Confirmed
- Affected area: toggle_favorite command, favorite delta, entry modified/size cache
- Evidence:
  - File: src-tauri/src/vault/security.rs:94–122、src-tauri/src/vault/dto.rs:300–312、src/lib/services/vault.ts:293–319/1349–1362、src/lib/types/vault.ts:195–209（7b5e8d5 修复前）
  - Function / Module: toggle_favorite_delta / applyBackendDelta / invokeSessionDelta
  - Relevant behavior: 实际收藏命令修改 SecPivot.Favorite 自定义字段，却不刷新条目 last_modification；Favorite delta 只返回 revision/uuid/favorite。前端也只修改 favorite，未应用条目的 modified/size。
  - Reproduction: favorite-metadata-red.log 的两个新增原生测试失败（已有五项通过）：固定旧时间的存量合成库，真实 toggle_favorite_delta/save/reopen 后在首次单边合并断言失败；另一条真实 DTO JSON 检查同时识别 modified、size 与完整快照不一致。第一条 red 未执行反向合并/合并库重开及取消收藏分支，第二条 red 停在首次收藏，不声明原 baseline 已运行取消收藏。
  - Independent verification: commands/vault.rs:910–920 的实际 toggle_favorite handler 返回 toggle_favorite_delta；services/vault.ts 的正常 UI 路径调用该 command。真实服务模块图仅 mock Tauri invoke 的 favorite-service-metadata-red.log 为 1 failed / 3 passed，即便模拟后端返回 modified/size，前端仍保留旧 2020 时间与 4301 大小而不是 2030/4321。该联合断言同时复现两个字段，后续节点/dirty/路由断言未执行。merge.rs 与 pinned keepass 的实际比较拒绝同时间不同内容；serialize.rs:246–248/278–340 定义标准时间格式和真实大小计算。
- Problem: 正常收藏切换保存成功，却因旧时间无法单边合并；列表/详情缓存的修改时间与条目大小滞后，直到完整快照刷新。
- Why it matters: 破坏日常收藏操作的同步与轻量更新契约。复现自动合并阻断和元数据缓存滞后，没有已丢失密码/附件或用户库证据，按 Medium。
- Realistic failure scenario: 一台设备仅收藏/取消收藏存量条目，保存后与未修改副本同步仍提示同时间戳冲突；本机修改时间和大小列未随收藏更新。
- Minimal fix: 收藏 mutation 设置当前秒精度条目修改时间，返回同一格式时间及实际 entry_size 总量。Rust/TypeScript Favorite delta 增加 modified、size，前端与 favorite 一起做不可变树补丁。浏览器 demo 切换也更新自己的条目时间。
- Better long-term fix: 独立复核同秒变化、历史时间重复与其他 delta 对缓存元数据的影响；不要用 renderer 时钟推测 native 条目时间或为收藏额外生成编辑历史。
- Regression test suggestion: 收藏/取消收藏都在真实 API/save/reopen 后双向合并并保存合并库重开；原生 JSON delta 的 modified/size 与完整快照和实际存储统计相等。真实服务 mock invoke 的 delta 同时更新时间/大小/favorite，旧快照与无关条目保持，sessionId/command/revision/dirty 正确。
- Estimated effort: 1–2 小时（原生时间、两端 delta、服务/原生回归与维护契约）

6ce4570 已修复。favorite-metadata-green.log 的 7 项通过，含两个新增
原生回归；favorite-service-metadata-green.log 的 4 项通过，含一个新增
真实服务模块图回归。最终测试覆盖收藏及取消、双向合并、合并库
保存重开、JSON 元数据与不可变节点、命令/session 路由。
完整 verify-favorite-metadata.log exit 0：317 frontend、610 Rust、
ACL 2，check 零错误警告、契约、build、clippy、format 全部通过。
Rust 单线程无过滤，四个 live clipboard body 关闭。完整门启动后
仅补全两处 reference 字段表，favorite-metadata-final-format.log 再次
通过。data-contracts 的类型表/命令表/详细契约同步，无新 TODO 勾选。
没有额外条目历史、CSS/markup、vendor、密码返回字段或 IPC command
变化；新增返回字段仅时间与大小。原生 Tauri handler/真实列表渲染/
Android provider/用户库/共享剪贴板未执行；浏览器服务测试不替代
真机证据，秒精度仍无同秒单调保证。

当前详细卡片 60（Critical 1 / High 12 / Medium 37 / Low 10），
全部 Confirmed；原始 b7c5d9f 的 15 项/6.1 分保持。完整审计目标
active，继续复核同秒历史/合并与其余系统、UI 和竞品迁移能力。
手机已安装 v1.6.5 的云端另存为 ACL 发行源码根因已确认，当前
源码授权已修复；替换 APK、真机和具体 provider 仍未验证。

### Finding: SP-S50 附件合并遗漏载荷、混用编号并丢失待安装版本的所有权（补充发现）

- Severity: High
- Confidence: High
- Category: Data Integrity / Synchronization / Dependency behavior
- Status: Confirmed
- Affected area: Native entry merge, attachment pool and detached recycle-bin history
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/merge.rs:439–446/550–584、src-tauri/src/vault/merge.rs:168–181/359–366（316116c 修复前）
  - Function / Module: merge_entries / merge_databases / restore_bin / EntryMut::remove
  - Relevant behavior: 原始依赖 Created 直接克隆 Entry，但不复制数据库 binary pool；Updated 复制普通属性但遗漏 attachments。两库 binary ID 仅在各自库内有效。第一方回收站恢复只复制 current 附件，历史引用仍指向 detach/remove 后失去的载荷。
  - Reproduction: merge-attachments-red-extended.log 四项失败，分别在源胜出后仍读到旧附件、源新增条目读取缺失载荷 panic、两侧新增条目编号碰撞读到另一侧字节、回收站历史恢复缺失载荷 panic 的首个断言停止。未执行后续反向合并/最终保存覆盖断言，不声称这些 red 已证明用户云端覆盖或永久丢失。
  - Independent verification: 不带任何 Cargo patch 的原始 registry keepass =0.13.22，两个真实合成 KDBX save/reopen/merge 测试均失败，见 merge-attachments-registry-red.log。公开 archive checksum a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e；merge-attachments-provenance.json 保存独立项目测试 hash。固定官方 KeePass PwEntry.cs commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5:490–516 的 AssignProperties 显式深复制 Binaries，SHA256 a68f2d7df7a0d49940b63387ed20634f849e60de6f4bb01c4f0ab8a4becba230。未运行官方客户端。
- Problem: 合并成功不保证附件对应正确字节，或读取/恢复时发生缺失载荷 panic；回收站历史也缺少完整载荷移植。
- Why it matters: 附件是密码库的持久数据，错误字节和不可读取的历史破坏数据保真，并可能中断会话。已证明合成库损坏与 panic，缺少实际用户覆盖/永久丢失证据，按 High。
- Realistic failure scenario: 两台设备各增加一个不同附件后合并，原本相同的数字 ID 指向不同内容；较新源条目的附件没有替换旧 current 集合；本地回收站记录在合并后无法恢复早期附件版本。
- Minimal fix: 仅在临时数据库 clones 中预先校验并统一 current/history binary ID，按精确字节和保护标记定义身份，SHA256 只索引候选。Created/Updated 实际日志决定 current 附件胜出；完整移植本地回收站各版本载荷。预备租约保护尚未登记的合并所有者，完成后重建 reverse refs 再释放；删除同 ID 的多个文件名时只访问一次。
- Better long-term fix: 上游修正载荷所有权后用相同回归复核，再考虑移除兼容层；分别审查同 current 时间的历史联合、同秒不同内容与历史去重策略。不要从附件编号修复推断全部同步语义完成。
- Regression test suggestion: 实际原生双向合并与合并库 save/reopen 检查较新源、新条目、编号碰撞、回收站历史、分叉历史、空附件、保护位、删除、header 编号重排及共享所有者的删除边界。依赖层另测缺失引用原子拒绝、精确内容/保护去重、已有租约拒绝、共享新所有者生存和重复文件名删除。
- Estimated effort: 6–10 小时（原始包复核、载荷移植、所有权生命周期、补丁重建与完整门）

46eaec4 已修复。merge-attachments-green-owner-final.log 的 8 项原生回归
和 merge-attachments-dependency-green-owner-final.log 的 7 项依赖层回归
均通过。提交前对中间补丁再次审查，发现 Created 在删除旧 entry 前直接
克隆、尚未登记 reverse owner；merge-attachments-shared-delete-red.log
独立一项仍因缺失载荷失败。因此增加 prepared leases 与去重删除，最终
8/7 回归通过。该 red 来自中间修复，未冒充原始 baseline。

最终 verify-merge-attachments-owner-final.log exit 0：317 frontend、
618 Rust、ACL 2，format、check 零错误警告、150-key 契约、build 和
clippy 全部通过；Rust 单线程，无过滤，四个 live clipboard body 开关
关闭。之前 617-test 门仅覆盖中间补丁，未用作最终验证。最初夹具 Value
限定名编译错误已修正，不计产品缺陷。

公开原包重新应用 zero-context patch，merge-attachments-patch-reconstruction-owner-final.log
验证全部 60 hashes；57 upstream 文件保持原字节，三个 reviewed 文件
和 provenance/patch 同步。仅对 owned patched 文件格式化。补丁未引入
第四个 upstream 编辑文件。data-contracts、security-model、project-structure
和 KEEPASS_PATCH 文档同步；提交前 comment/docs 变更再次通过完整格式门。
同步 TODO 收紧为部分完成，没有从标题字段回归推断完整历史保真。

当前详细卡片 61（Critical 1 / High 13 / Medium 37 / Low 10），全部
Confirmed。原始 b7c5d9f 的 15 项/6.1 分保持，整体目标 active。
仅用合成库，无用户库、共享剪贴板、真实 Tauri handler/官方客户端/
Android provider 或替换 APK 运行。手机 v1.6.5 云端另存为源码 ACL
修复已确认；真机验证与其余系统、UI、竞品能力审查继续进行。

### Finding: SP-S51 当前内容与时间相同即跳过历史联合，遗漏另一设备的独有旧版本（补充发现）

- Severity: High
- Confidence: High
- Category: Data Integrity / Synchronization / Dependency behavior
- Status: Confirmed
- Affected area: Equal-current-time native entry history merge
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/merge.rs:525–548（837b827 修复前）
  - Function / Module: merge_entries / merge_databases
  - Relevant behavior: 当前 last_modification 相等且内容比较相等时直接 continue，在读取或联合两边 History 之前返回该条目。当前版本一致不代表此前编辑路径一致。
  - Reproduction: merge-history-branches-red-distinct.log 的 equal-current 回归在首次历史查找失败。真实合成库两分支分别在 t20/t30 保存不同中间附件/内容，再在 t40 收敛到同一 current 内容，保存重开后合并仍缺少远程中间历史。red 未到反方向、最终合并库重开或重复合并断言。
  - Independent verification: 不带应用补丁的原始 registry keepass =0.13.22 的真实 KDBX save/reopen 同样缺少远程历史，merge-history-registry-red-distinct.log 为 1 failed。原 archive checksum a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e。固定官方 PwDatabase.cs commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5:875–897 在当前属性处理后单独调用 MergeEntryHistory；1635–1692 联合不同历史时间。SHA256 3470bf8193e95d211d422eafba901a39a68f2a4d796ce11a0e44778d87ad01b0，merge-history-reference-provenance.json 保存 URL/独立夹具 hash。官方对同历史时间也使用时间键，因此并非保留同时间不同内容的证据；未运行官方客户端。
- Problem: 正常返回成功的同步结果包含最新内容，却遗漏另一设备曾保存的中间版本及其附件。
- Why it matters: 破坏历史恢复承诺；当前记录仍正常，不以合成结果宣称用户密码丢失或真实远程永久覆盖，按 High。
- Realistic failure scenario: 两设备分别改记录后又恢复成相同当前值，同步时只有一边的中间版本保留，另一边的旧附件/字段无法从合并历史恢复。
- Minimal fix: 在临时规范化 binary namespace 上收集 current 时间相同且 history 不同的源历史；原始合并成功后联合到仍存在的条目，不因 current 相等跳过历史。按时间加内容去重，重建附件 ownership。
- Better long-term fix: 上游修复后复用相同真实 KDBX 回归；继续验证完整 current 属性、历史缺省/异常时间及所有跨客户端边界。
- Regression test suggestion: 两分支分别修改中间内容和附件，再使 current 内容/时间相同；双向合并、保存重开都保留三个历史版本和确切附件，重复合并不增长版本数。同时间但到期设置不同的历史分别可恢复。
- Estimated effort: 2–4 小时（联合策略、真实回归、原始包复核和维护契约）

### Finding: SP-S52 本地胜出时按时间跳过远程旧版本，丢弃同时间不同内容的历史（补充发现）

- Severity: High
- Confidence: High
- Category: Data Integrity / Synchronization / History identity
- Status: Confirmed
- Affected area: Local-winner remote-loser history insertion
- Evidence:
  - File: src-tauri/src/vault/merge.rs:116–142（837b827 修复前）
  - Function / Module: collect_remote_losers / push_losers_into_history
  - Relevant behavior: 已捕获本地较新条目对应的远程旧 current，但只要 merged history 有相同 last_modification 就 continue，没有比较版本内容。时间精度为秒，时间相同不能代表版本相同。
  - Reproduction: merge-history-branches-red-distinct.log 的 same-time-loser 回归在首次远程历史查找失败。共同库的本地 t20 版本、远程 t20 版本内容/附件不同；本地随后 t40 编辑胜出，远程旧 current 仍应保留，却因本地 t20 历史已存在被跳过。red 未执行后续重开或反向断言。
  - Independent verification: 对照保存重开后实际 native 两库时间/历史、collect_remote_losers 的内容差异条件与第一方 skip 控制流。原依赖 merge_history:636–656 对同时间不同内容会保留双方，DuplicateHistoryEntries 只在 enum 定义，未被实际发出；原注释的“重复时间必然导致后续合并失败”依据不成立。不是从官方字典去重行为推断保留双方。
- Problem: 第一方兼容层抹掉已经捕获的旧分支版本，成功同步后的历史无法恢复那次远程编辑。
- Why it matters: 同秒编辑是 KDBX 合法状态，当前最新版本仍可读，但历史保真失败；没有用户真实覆盖/永久丢失证据，按 High。
- Realistic failure scenario: 两设备在同一秒分别保存不同的字段或附件，之后本地再更新；合并时旧远程分支因时间碰撞被丢弃。
- Minimal fix: 将捕获的 loser 与全部已有历史按 timestamp 加完整内容去重。到期设置和规范化附件引用也参与内容身份；不同内容保留双方，相同版本只留一份。稳定降序排列，批量构建历史后重建附件 reverse refs。
- Better long-term fix: 不用时间戳代替版本身份；分别核对 current 同秒冲突、未跟踪的外部编辑、current 属性赋值和历史 retention 兼容。
- Regression test suggestion: 本地更近、已有历史与远程旧 current 同时间且不同字段/附件；双向合并和保存重开保留三个版本及正确附件，重复同步版本数保持。相同时间、同标题但不同到期设置的历史均可恢复。
- Estimated effort: 1–3 小时（共享联合函数、时间碰撞回归和契约同步）

89d4504 以一个完整历史联合修复同时处理 SP-S51/SP-S52。新增 3 项
原生回归；merge-history-branches-green.log 的 24 项合并测试通过，
merge-history-expiration-green.log 的 3 项覆盖双向、附件、重开、重复
联合与到期设置恢复。最终完整 verify-merge-history-branches-final-debug0.log
exit 0：317 frontend、621 Rust、ACL 2、format、check 零错误警告、
150-key 契约、build 和 clippy 全部通过。验证临时设置
CARGO_PROFILE_DEV_DEBUG=0 降低编译内存，不改仓库 profile，不优化或
过滤测试；Rust 单线程，无过滤，四个
live clipboard body 开关关闭。

首次原生编译因 rustc 无法分配 2 MiB 失败，未到产品断言；不改源码
重试后取得两个有效 red。最初 equal-current 夹具两边中间时间也相同，
随后分别固定为 t20/t30，将历史联合与同时间 loser 碰撞分开，最终两个
red 仍失败。官方时间键策略不是本次保留不同内容双方的依据。

第一轮完整门的 621 Rust 已通过，但 clippy 遇到 image/nom 缓存版本
不一致，不能计作总门成功。仅 cargo clean -p image -p nom 清理这两项
生成缓存（72 files / 131.8 MiB），没有改依赖版本或用户文件。提交前
去掉反复 add_entry 首部插入的批量重建，增加 patched History::from_entries
一次构造有序 Vec，并清除 nested histories；最终门覆盖该实际实现。
没有从构造改动宣称测得大型真实库性能提升。
最终源码的首次完整门在 Vite 阶段发生 Node Zone Allocation 内存分配
失败，尚未到后端测试；保留 verify-merge-history-branches-final.log，
未改源码/依赖版本重试在 rustc 编译阶段再次内存不足，日志
verify-merge-history-branches-final-retry.log 保留。随后用临时调试符号
覆盖取得最终门，不将这些构建失败计为应用发现。

merge-history-dependency-green-final.log 的 7 项通过；原公开包重新
应用补丁，merge-history-patch-reconstruction-final.log 验证全部 60
hashes，57 upstream 字节保持，仍只改三个 reviewed 文件。
data-contracts、project-structure、KEEPASS_PATCH 与 partial TODO 同步，
没有据此勾选完整同步能力。报告提交无需额外 reference 更新。

当前详细卡片 63（Critical 1 / High 15 / Medium 37 / Low 10），全部
Confirmed；原始 b7c5d9f 的 15 项/6.1 分保持。整体目标 active；当前
属性合并与同秒策略、其余系统/UI/竞品迁移继续审查。仅使用合成库，
没有用户库、共享剪贴板、真实 Tauri handler/官方客户端/Android provider
或替换 APK 运行。手机 v1.6.5 另存为 ACL 发行源码根因与当前源码修复
已确认，真机/provider 验证仍待完成。

### Finding: SP-S53 合并遗漏当前到期设置，并忽略到期差异的冲突与旧版本（补充发现）

- Severity: Medium
- Confidence: High
- Category: Configuration Safety / Data Integrity / Synchronization
- Status: Confirmed
- Affected area: Current entry expiration merge and historical expiration identity
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/merge.rs:525–534/564–584/808–820、src-tauri/src/vault/merge.rs:60–72/86–90（aba9da2 修复前）
  - Function / Module: merge_entries / have_entries_diverged / collect_remote_losers / merge_databases
  - Relevant behavior: 原始依赖较新 source 分支只复制 Times.last_modification，遗漏 expiry/expires；相等时间的内容比较将整个 Times 清空，包括到期设置。第一方 loser 比较沿用该方式，也无法识别到期唯一变化。
  - Reproduction: merge-current-expiration-native-red.log 的五项实际 API/合成 KDBX 回归均失败。启用仍为 None，延期仍为 2030 而不是 2040，取消仍为 2030，分叉修改仍保留较旧 2035；四项都在首次 current expiry 断言停止。同时间、不同到期设置的第五项没有报冲突。原 red 没有执行反方向、最终合并库重开、历史恢复或重复合并断言。
  - Independent verification: 不带应用补丁的 registry keepass =0.13.22 对真实 synthetic KDBX save/reopen 同样在首次合并后 expiry/expires 检查失败，merge-current-expiration-registry-red.log 为 1 failed，archive SHA256 a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e。固定官方 PwEntry.cs commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5:490–531 的 AssignProperties 显式复制 m_tExpire/m_bExpires，SHA256 a68f2d7df7a0d49940b63387ed20634f849e60de6f4bb01c4f0ab8a4becba230，完整来源和独立测试 hash 见 merge-current-expiration-provenance.json。未运行官方客户端。
- Problem: 较新的启用/延期/取消设置不能随 current 内容胜出；同时间不同到期设置被当作相等，旧分支的到期设置也不能恢复。
- Why it matters: 合并撤销用户已保存的到期配置，使到期状态和提醒不正确。本项证明配置保真与历史到期恢复失败，没有密码/附件字节丢失、真实云端覆盖或凭据泄露证据，按 Medium。
- Realistic failure scenario: 一台设备延长或取消某记录的到期，另一台合并后修改时间已更新，却仍显示旧到期设置；分叉修改后也无法从历史恢复旧设备的到期选择。
- Minimal fix: 以实际 Entry Created/Updated 日志复制 source expiry/expires。相等 current 时间的到期冲突候选在胜出时间改变前捕获，并在原生删除决策后仅对仍存在的差异条目报现有同时间戳错误，持久化前拒绝。到期设置纳入内容身份，双向捕获较旧 current 并按时间加内容联合历史。
- Better long-term fix: 逐项核对 current 属性/时间元数据与外部客户端的真实兼容；继续审查缺少修改时间的对象、同秒策略及完整历史保真，避免以一个设置修复认定整个合并完成。
- Regression test suggestion: 启用/延期/取消经真实 update_entry/save/reopen 后双向合并，保存合并库重开保持 current date/enabled/time。分叉到期修改保留公共基线与较旧分支，历史恢复后再保存重开保持旧 date/enabled；重复合并不重复，等时间到期差异拒绝。较新到期更新后永久删除不得在任一方向复活。
- Estimated effort: 2–4 小时（current 补齐、版本比较、六个回归、独立包复核与契约）

47226b1 已修复。先补齐 current 设置与冲突检查后，
merge-current-expiration-intermediate-corrected.log 为 4 passed / 1 failed：
分叉回归已越过 current 断言，在“旧到期设置应留在历史”断言失败。
这份中间修复证据独立确认旧版本缺失，未冒充原始 red 已执行历史断言。
最终将到期纳入内容身份并捕获两侧较旧 current，31 项 merge 回归在
merge-current-expiration-green-final.log 通过，包括六项新增到期场景。

完整 verify-merge-current-expiration.log exit 0：317 frontend、627 Rust、
ACL 2，format、check 零错误警告、150-key 契约、build、clippy 全部
通过；Rust 单线程，无过滤，临时 CARGO_PROFILE_DEV_DEBUG=0，四个
live clipboard body 关闭。没有改仓库构建 profile 或依赖版本。
data-contracts 与 project-structure 同步现有命令/模块契约，TODO 仍
保持 partial；无 CSS/markup、IPC/DTO/schema、vendor 或密码返回改动。
依赖 provenance gate 仍验证 60 文件/3 reviewed patches/57 unchanged。

最初 event match 缺少 non-exhaustive/wildcard 分支导致编译失败，修正
后才取得有效中间回归，不是原应用缺陷。初次额外分组删除夹具错误地
认为条目会先删除再重建；实际 crate 在移出条目后才删除旧分组，条目
仍存活，同时间到期差异应报冲突。未为该错误预期削弱冲突规则；最终
以真实永久删除场景验证双向不复活。失败日志 merge-current-expiration-green.log
保留，不将这个 fixture 错误计为新增应用问题。

本报告统计 64 张 Confirmed 卡片（Critical 1 / High 15 / Medium 38 /
Low 10），原始 b7c5d9f 的 15 项/6.1 分保持。报告提交不需额外技能
引用变更。整体目标 active，继续其余 current 合并、系统/UI/竞品能力
审查。仅使用合成数据，无用户库/共享剪贴板/原生 Tauri handler/官方
客户端/Android provider 或替换 APK 运行；手机 v1.6.5 的另存为源码
ACL 修复与发行根因已确认，真机验证仍未完成。

### Finding: SP-S54 隐藏附件输入被当作 Tab 边界，弹窗焦点循环失效（补充发现）

- Severity: Medium
- Confidence: High
- Category: Accessibility / Frontend State / Keyboard ownership
- Status: Confirmed
- Affected area: Shared ModalShell keyboard focus trap, actual entry-editor attachment tab
- Evidence:
  - File: src/lib/components/ModalShell.svelte:66–106、src/lib/components/EntryEditorDialog.svelte:1509–1518/1917–1919（b6efc80 修复前）
  - Function / Module: focusables / handleKeydown / attachments tab file-input
  - Relevant behavior: input:not([disabled]) 仍匹配 CSS 隐藏且 tabindex=-1 的附件 file input。列表未检查有效 tabindex、布局、visibility、fieldset disabled 或 inert。末尾隐藏输入因此替代实际最后的“添加附件”按钮。
  - Reproduction: modal-tabbable-browser-red.log/json 在生产 EntryEditorDialog 的新建/附件页运行原生按键；dark/light/custom × 1100×720/420×600/420×320 共 9 组全部失败。首个保存按钮 Shift+Tab 仍停在保存，最后的添加附件 Tab 移到 synthetic-after（dialog 外）。没有 pageerror，卸载后回到 synthetic-invoker。
  - Independent verification: 按真实 CSS 布局/activeElement 判断，与原 handleKeydown 的 first/last 分支和隐藏 file input 核对；随后六项生产 handler 回归在修复前 4 failed / 2 passed（modal-focus-unit-red.log）。测试 DOM seam 不作为原生选择器或布局证据，浏览器实际 Tab 才是该证据。
- Problem: 焦点 trap 使用不可 Tab 的隐藏节点确定边界，导致反向循环失败且正向跳到背景。
- Why it matters: 键盘用户在正常附件操作中丢失当前弹窗焦点，必须重新定位。没有观察到秘密泄露、背景保存或库破坏，按 Medium。
- Realistic failure scenario: 打开条目附件页，从保存按 Shift+Tab 试图进入附件操作却不移动；从添加附件按 Tab 后焦点离开仍显示的弹窗。
- Minimal fix: 每次按键重新过滤 tabIndex>=0、有效 :disabled、inert 祖先、无布局和非 visible 元素；使用剩余真实候选作为双向循环边界。
- Better long-term fix: 继续检查其他业务弹窗及屏幕阅读器/移动外接键盘行为，新增控件时保留可见 Tab 边界；不从这一例认定全部弹窗可访问性完整。
- Regression test suggestion: 生产附件编辑页末尾隐藏输入的双向原生 Tab；负 tabindex、fieldset disabled、inert、display:none 和 visibility:hidden/collapse；动态移除最后控件、无候选 surface fallback、已消费按键及下层 modal 不抢归属；关闭恢复 invoker。
- Estimated effort: 1–2 小时（真实浏览器复现、六项回归、最小过滤、完整门与契约更新）

ac810e0 已修复。modal-tabbable-browser-green.log/json 的 9 组均通过，
无 pageerror；modal-focus-eligibility-green.log/json 另在实际 DOM 注入六种
不可 Tab 的尾部候选，双向循环全部通过。“添加附件”仍触发真实浏览器
filechooser，空选择不读取任何文件。九份截图及 contact sheet 已检查；
窄屏页签文字折行是单独待优化的视觉问题，不冒充焦点修复效果。

完整 verify-modal-focus.log exit 0：323 frontend、627 Rust、ACL 2，
format、check 零错误警告、150-key 契约、build、clippy 全部通过。
Rust 单线程、无过滤，临时 CARGO_PROFILE_DEV_DEBUG=0；四个 live
clipboard body opt-out，不改仓库 profile。modal-focus-final-format.log
再次通过。css-theming/project-structure 更新焦点契约；TODO 只纠正
不存在的 modal-shared.css 引用，未据此勾选新的完成项。无样式/markup、
IPC/schema、vendor 或凭据返回改变。

证据仅用新建模式合成 group、独立浏览器；未提交编辑器，未访问用户
库/共享 OS 剪贴板/原生 Tauri handler/Android provider。两处证据工具
编辑失败（patch 行匹配和 PowerShell 内联文档替换的引号）在实际断言前
修正，不计产品缺陷。自己启动的 Vite/Edge 已清理。原始 b7c5d9f 的
15 项/6.1 分保持；当前 65 张 Confirmed（Critical 1 / High 15 /
Medium 39 / Low 10），整体目标 active。手机 v1.6.5 另存为 ACL 源码
修复与发行根因已确认；替换 APK/真机/provider 仍待验证。

### Finding: SP-S55 编辑器页签在窄屏或较大字号下折行，标签难以阅读（补充发现）

- Severity: Low
- Confidence: High
- Category: Accessibility / Responsive UI / Visual design
- Status: Confirmed
- Affected area: EntryEditorDialog secondary tab labels and overflow
- Evidence:
  - File: src/lib/components/EntryEditorDialog.svelte:829–900/1541–1568（564751d 修复前）
  - Function / Module: editor-tabs / editor-tab layout
  - Relevant behavior: 页签 flex 项默认允许收缩，标签允许折行，整行未配置横向滚动。手机宽度把“字段”“附件”等拆成单字两行；英文和较大字号使相同问题出现在更多尺寸。
  - Reproduction: editor-tabs-browser-red.log/json 在生产新建编辑器、合成 group、附件页进行 36 组浏览器检查：三主题 × 中英文 × 11/15px secondary 字号 × 1100×720/420×600/360×320。33 组标签有多行，默认中文桌面三主题的 3 组通过；使用实际文字 Range rects 和截图，非 CSS 字符串判断。
  - Independent verification: 和原 scoped CSS 的 shrink/wrap 默认行为及实际浅色窄屏截图交叉核对；设置页/只读 clipboard 参考已有不收缩、横向滚动的密集导航模式。未把这项可读性问题升级为隐藏能力或数据丢失。
- Problem: 页签标签被挤成碎片式多行，导航高度和可读性随语言/字体意外改变。
- Why it matters: 手机屏幕和大字号偏好下增加辨认操作的成本。记录内容、保存与点击能力仍存在，按 Low。
- Realistic failure scenario: 手机宽度打开新建/编辑记录，附件和字段页签显示为纵向单字；切换英文或调大字号进一步压缩标签。
- Minimal fix: 标签保持 nowrap、项目不收缩，页签栏内部横向滚动；复用既有字体/颜色/滚动条 token 和控件 padding。焦点进入时 nearest 对齐完整按钮，现有焦点描边 inset 防裁切。
- Better long-term fix: 继续验证其他业务导航的窄屏、长标签、字体偏好及真实 Android WebView；保持紧凑主题，不以单组件效果认定移动端完整适配。
- Regression test suggestion: 中英文、dark/light/custom、默认和增大 secondary 字号的宽/窄/短窗口，测量每个标签单行与 dialog 边界；原生 Tab 遍历六个页签时完整露出当前控件，焦点描边可见，页签栏滚动不扩大弹窗。
- Estimated effort: 1–2 小时（布局复现、最小样式/焦点补齐、36 组渲染和维护契约）

d6501ab 已优化。最终 editor-tabs-browser-green-final-reveal.log 与
editor-tabs-green-final.json 的 36 组均通过，全部单行、每个原生 Tab
焦点完整露出、弹窗不横向溢出；无 pageerror，卸载恢复 synthetic-invoker。
36 张截图和三主题 contact sheets 已检查，默认中文桌面保持紧凑形态。
没有编辑器提交、文件读取或真实用户库操作。

首版仅增加 nowrap/overflow 时，33 组仍在 focused item 完整可见检查失败，
editor-tabs-green-immediate-measurement.log/json 保留。两帧渲染后仍失败
（editor-tabs-browser-green-final.log），确认不是单纯采样时序：浏览器
可能仅让控件部分可见。因此增加 focusin 的 nearest 滚动和 inset
描边，最终独立复测通过，不将中间 green 名称误作通过证据。
辅助 Python 首次未指定 UTF-8 的 JSON 读取失败已修正，不计应用缺陷。

editor-tabs-scoped-verification.log exit 0：check 零错误警告、build、
323 frontend、全仓 format（Prettier/cargo fmt）通过。本项只改变
UI/样式，原生集成门沿用 ac810e0 的 verify-modal-focus.log：627 Rust、
ACL 2，临时 debug=0、单线程、无过滤、live clipboard opt-out。
未额外重复后端测试。css-theming 同步 approved 单行/滚动/焦点模式，
无新主题变量、业务数据/schema/IPC、vendor 或 TODO 状态改变。

当前详细卡片 66 Confirmed（Critical 1 / High 15 / Medium 39 / Low 11），
原始 b7c5d9f 的 15 项/6.1 分保持。报告提交无需额外维护引用更新。
整体目标 active，继续其余系统/竞品/UI 覆盖。自己启动的 Vite/Edge
已清理；没有原生 Tauri/Android WebView、屏幕阅读器或设备触摸验证。
手机 v1.6.5 的另存为 ACL 源码修复已确认，新 APK/真机/provider 仍待验证。

### Finding: SP-S56 移动记录混入内容比较，同父分组合并遗漏较新位置时间（补充发现）

- Severity: Medium
- Confidence: High
- Category: Data integrity / Synchronization / Stability
- Status: Confirmed
- Affected area: native entry/group location merge and first-party history content identity
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/merge.rs:196–202/294–309/492–509/546–552/790–822、src-tauri/src/vault/merge.rs:61–72（3575b38 修复前）
  - Function / Module: merge_groups / merge_entries / have_groups_diverged / have_entries_diverged / content_diverged
  - Relevant behavior: 合并仅在父分组不同时比较 LocationChanged，返回同一父分组的较新移动时间被遗漏；PreviousParentGroup 被内容 equality 比较，entry 当前 parent 也污染历史内容身份。分组为避免环而临时移到根时覆盖真实 previous parent。
  - Reproduction: merge-location-convergence-red-compiled.log 的四个实际原生回归全部失败。合成库由 create_session/add_group/add_entry 创建并保存重开，entry 经真实 move_entry 移出再移回，group 经真实 delete_group/restore_group 返回根。内容时间不变的两项报同时间内容冲突；另有真实 flags/rename 编辑的两项保留 t10 而非较新 t30 位置时间。初始 fixture 未限定 Times 的编译失败不计产品证据。原始红测停止在首次失败，后续重开/反向/第三设备断言只由最终绿测覆盖。
  - Independent verification: 未应用 Cargo patch 的独立 registry keepass =0.13.22，save/reopen/merge 四项也全部失败（merge-location-registry-independent-red.log）。host cargo metadata 确认实际 registry 来源，archive checksum a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e。固定官方 KeePass commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5 的 PwDatabase.cs:910–917/1405–1430/1457–1490，先 Relocate 再独立合并最大 LocationChanged 与对应 PreviousParentGroup；文件 SHA-256 3470bf8193e95d211d422eafba901a39a68f2a4d796ce11a0e44778d87ad01b0。来源记录 merge-location-reference-provenance.json，未运行官方客户端。
- Problem: 合并把移动簿记误当内容修改，同时忘记已返回原位置的较新时间；后续第三设备的旧移动可把对象重新移走，或纯位置差异阻止自动合并。
- Why it matters: 常规多设备移动与返回操作造成错误分组或不必要冲突。复现范围是位置/簿记/历史重复与合并拒绝，没有证实字段、密码或整个对象永久丢失，按 Medium。
- Realistic failure scenario: A 在 t30 将条目移走再移回根并在 t40 改属性，B 与基线合并后只留下旧位置时间；C 的 t20 旧移动随后被误认为更新，把条目放回旧分组。无内容修改的相同路径则可能先报“同一时间戳内容不同”。
- Minimal fix: 同父分组合并较新位置时间和其 previous parent；实际重定位/新建分组保留源 provenance，分组临时根移动完成后恢复它。Entry::content_eq 忽略时间/history/当前及前父分组，first-party 另比到期属性；未知时间的不同父分组不借用未执行移动的 source timestamp。
- Better long-term fix: 继续独立验证相同位置时间的并行移动、删除与较新移动的排序、跨客户端分组顺序及真实远端同步；不可仅按 LastModificationTime 标完整兼容。
- Regression test suggestion: 四种 entry/group 返回路径含内容有无编辑，保存重开、双向合并、第三设备旧移动；不同父分组相同内容、父子互换、真实同时间字段/名称冲突、较新永久删除、缺失时间和历史同内容去重。底层验证不存在 previous-parent 分组的原始 UUID 经 KDBX 保存重开仍保留。
- Estimated effort: 2–4 小时（原生复现、官方/registry 交叉核验、依赖补丁及完整门禁）

bab958a 已修复。10 个新增原生回归通过（merge-location-boundaries-final.log），
8 个依赖专属测试通过（merge-location-dependency-final.log），其中新测试直接
读取不再存在分组的原始 previous-parent UUID 并保存重开。上游合并测试
merge-location-upstream-compatibility.log 的 32 项全部通过（31 个原有 + 1 个新增）。
纯位置差异不再产生内容冲突，真实同时间字段/分组名称冲突仍拒绝，较新永久
删除不复活对象；缺失时间保持既有未知移动策略，未发明时间顺序。

中间边界探针曾 8 通过 / 2 失败：source 的 previous parent 指向被回收站
规则移除的 bin shell，公开 previous_parent() 只返回仍存在的分组，None
不能证明原始 UUID 丢失。纠正可解析 getter 的 fixture 期待后，再增加底层
原始 UUID 保存重开测试；不将这两项 fixture 失败当新产品缺陷。第一次
patch formatter 未显式 UTF-8 返回失败，已修正；全平台 offline metadata
因未缓存 js-sys 失败，实际 Windows target metadata 随后通过，均非产品缺陷。

verify-merge-location.log exit 0：323 frontend、637 Rust、ACL 2，format、
check 零错误警告、150-key 契约、build、clippy 全部通过；Rust 单线程，
无过滤，临时 CARGO_PROFILE_DEV_DEBUG=0，四个原生 live clipboard body
保持 opt-out。merge-location-final-format.log 再次通过。新拥有的 db/merge.rs
单文件通过 stdin rustfmt 格式化，未遍历其他上游文件。KEEPASS_PATCH、
provenance 和四份维护引用已同步；当前四文件补丁，56 个上游文件逐字节
不变，从新复制原始包应用 zero-context patch 后全部 60 hashes 匹配。

TODO 只补已验证位置/历史子边界，整体同步保持 [~]。本项无 UI/CSS、
IPC/schema、密码返回改变，未访问用户库/共享 OS 剪贴板/真实 Tauri handler、
第三方客户端/设备/provider；未证明真实用户旧位置覆盖或永久丢失。
当前详细卡片 67 Confirmed（Critical 1 / High 15 / Medium 40 / Low 11），
原始 b7c5d9f 的 15 项/6.1 分保持。整体目标 active，继续其余系统/竞品/UI
覆盖。手机 v1.6.5 另存为 ACL 源码修复已确认，新 APK/真机/provider 待验证。

### Finding: SP-S57 永久删除未写删除标记，旧副本合并使条目和分组复活（补充发现）

- Severity: High
- Confidence: High
- Category: Data integrity / Privacy / Synchronization
- Status: Confirmed
- Affected area: delete_entry / delete_entries / delete_group / empty_recycle_bin
- Evidence:
  - File: src-tauri/src/vault/entries.rs:600–601/646–647/968–969/1019–1025；src-tauri/vendor/keepass-0.13.22/src/db/types/entry.rs:681–736/812–820、src/db/types/group.rs:589–620/721–756（8dfcf40 修复前）
  - Function / Module: VaultSession permanent-removal branches and EntryMut/GroupMut vs EntryTrack/GroupTrack removal
  - Relevant behavior: 五处用户永久删除调用未追踪的 remove；只有 track_changes().remove 写入 DeletedObjects 时间。普通移除清理内存及附件引用，却不保存同步删除意图。分组追踪删除会递归记录整个子树。
  - Reproduction: permanent-deletion-native-red-corrected.log 的六项实际应用 API 回归全部失败：关闭回收站后单个/批量条目或分组永久删除，已回收的条目/分组再次删除，以及清空回收站。合成库先保存重开并保留旧副本，真实删除后再保存重开；首次与未改动旧副本合并时被删除条目重新出现。初始三个分组 fixture 因查找器没有遍历最深层而失败，已修正，不计产品证据。原始有效红测停止在第一次复活断言，后续反向/合并重开/重复合并及标记检查由最终绿测覆盖。
  - Independent verification: 没有 Cargo patch 的 registry keepass =0.13.22 两项直接 API 对照（permanent-deletion-registry-independent.log）通过：untracked 删除没有标记，双向合并后对象和子树返回；tracked 删除有标记且重开、双向合并保持删除。这是第一方接口选择错误，不是声称依赖的普通移除接口违约。固定官方 KeePass commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5，MainForm_Functions.cs:5116–5117/5215–5220/5245–5280 明确永久删除写 DeletedObjects，并在组/清空中调用 DeleteAllObjects；PwGroup.cs:1367–1387 递归写每个条目/分组标记。两个 SHA-256 分别 5174c3280e1487e016abb29ce695861fae4a09e471ff3d21eb326be8cefe59bb / 1836977793c3f634bb94e2d3cd2b669f709ae0519bd80a0761b565ed245066d2，来源见 permanent-deletion-reference-provenance.json。未运行官方客户端。
- Problem: 界面和重开都显示永久删除成功，但 KDBX 没有记录删除意图，下一次同步将旧副本当成新对象重新插入。
- Why it matters: 用户明确永久删除的凭据、条目和整个分组子树可以悄悄恢复，持久状态不符合删除意图，按 High；未证实秘密被第三方读取或真实用户库发生泄漏。
- Realistic failure scenario: 手机上永久删除记录并保存，随后另一设备上传仍含旧记录的数据库；选择合并后，被永久删除的记录重新出现在库中。清空回收站中的嵌套分组同样可能返回。
- Minimal fix: 单个/批量永久删除和清空循环用现有 tracked removal；分组递归写入每个 descendant 的 UUID/time 标记并保留 root 拒绝。临时 merge detachment 继续使用 untracked removal。
- Better long-term fix: 继续独立验证删除标记生命周期、相同时间/较新移动/嵌套修改冲突及真实客户端双向同步，区分永久删除意图与文件/备份安全擦除。
- Regression test suggestion: 六条真实 API 路径、三层子树和批量多个条目，KDBX 保存重开、双向旧副本合并、合并后重开及重复合并均不复活；每个 UUID 有具体时间，旧标记/未删除对象保留；普通回收无永久标记，无效批次不改 DB/revision/dirty；确实晚于删除的显式编辑仍按既有策略胜出。
- Estimated effort: 1–3 小时（真实删除/同步复现、官方/registry 对照及完整验证）

a44d2fd 已修复，8 个新增原生回归通过（permanent-deletion-native-green-final.log）。
六个主要场景验证旧副本不复活、每个子树对象的删除时间保存，以及未删除对象/
既有标记保留；两项边界验证 disabled/recycled 批次的无效 UUID 不产生部分
删除/标记或 dirty/revision 变化，较新真实 flags 编辑保持现有双向 winner 策略。
实际 `commands/vault` 和 `commands/groups` 转发到这些 VaultSession 方法；
没有把纯函数手工补 deleted_objects 的旧测试当作用户删除路径证据。

修复首次编译时缺少四处 mut（track_changes 需要 mutable receiver），在测试
执行前已纠正，不能计为产品回归。旧移动与永久删除时间策略本轮没有直接
改为 max：固定官方 ApplyDeletions 比较 LastModificationTime，这项不能仅按
方法名称推断跨客户端期望；后续需单独明确冲突策略。本项确认并修复的是
实际用户删除缺少标记，不将上述未完成边界标为已解决。

verify-permanent-deletion.log exit 0：323 frontend、645 Rust、ACL 2，format、
check 零错误警告、150-key 契约、build、clippy 全部通过。Rust 单线程、
无过滤、临时 CARGO_PROFILE_DEV_DEBUG=0，四个原生 live clipboard body
保持 opt-out。现有 60 文件/4 补丁 provenance gate 通过，未改 vendor 或
依赖版本。data-contracts/security-model/project-structure 同步追踪删除契约，
并纠正过时的“分组内容移到 root”说明；同步 TODO 补充已验证子边界，保持 [~]。

本项无 UI/CSS、IPC/schema 或秘密返回改变。没有真实 Tauri handler、用户库、
共享 OS 剪贴板、真实 S3/WebDAV/provider/第三方客户端/Android 运行验证，
也未证明文件系统/备份的安全擦除。报告本轮只增加证据，无额外维护引用更新。
当前详细卡片 68 Confirmed（Critical 1 / High 16 / Medium 40 / Low 11），
原始 b7c5d9f 的 15 项/6.1 分保持。整体目标 active，继续其余系统/竞品/UI
覆盖；手机 v1.6.5 另存为 ACL 源码修复已确认，新 APK/真机/provider 待验证。

### Finding: SP-S58 祖先分组删除连带丢失较新的深层编辑（补充发现）

- Severity: High
- Confidence: High
- Category: Data integrity / Synchronization / Stability
- Status: Confirmed
- Affected area: native group merge and permanent-deletion conflict resolution
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/merge.rs:96–113/130–155/226–272（8695b74 修复前）；src-tauri/src/vault/merge.rs::merge_databases
  - Function / Module: get_last_update / merge_groups / GroupMut::remove
  - Relevant behavior: 删除祖先前的时间扫描只查看直属条目和子分组。祖先可以先排入删除队列，而孙层条目或分组按自身时间幸存；随后 GroupMut::remove 递归清空祖先整个子树，覆盖了子对象的胜出决定。队列依赖 HashSet 遍历，Deleted 事件在实际移除之前生成，分组移动又在删除之后执行。
  - Reproduction: nested-deletion-native-red.log 两项真实应用回归失败。用 add_group/add_entry 建三层合成库，保存重开后保留旧副本；真实 delete_group 永久删除并保存重开，另一个实际 VaultSession 对旧副本执行 flags 编辑或叶分组改名，修改时间固定晚于具体删除时间一分钟。以编辑侧为 destination 合并、保存重开，较新条目或叶分组消失。原始红测均停止在该方向的首次缺失断言，反向、旧兄弟节点与重复合并由最终绿测证明。
  - Independent verification: 无本地 Cargo patch 的原始 registry keepass =0.13.22 两项直接 API/save/reopen 探针同样丢失深层条目/分组（nested-deletion-registry-red-corrected.log；metadata 确认 registry 来源）。固定官方 KeePass commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5 的 PwDatabase.cs:1066–1130 使用子先父后的 ApplyDeletions，并由 SafeCanDeleteGroup 拒绝删除非空分组；保留对象时移除旧标记。来源复用 merge-location-reference-provenance.json，文件 SHA-256 3470bf8193e95d211d422eafba901a39a68f2a4d796ce11a0e44778d87ad01b0 已重新核对。未执行官方客户端。
- Problem: 较新的编辑虽然满足自己的胜出条件，却被另一层祖先删除隐式覆盖；合并完成后保存重开确认缺失。
- Why it matters: 嵌套分组的跨设备删除/编辑冲突会持久丢失记录或较新分组元数据，按受限并发冲突场景的 High；不是每次保存必然损坏，原始输入仍保留在本次纯函数调用之外，未读取或证明真实用户库受损。
- Realistic failure scenario: 一台设备删除整个父分组，另一台随后编辑孙层凭据或改名深层分组；选择合并后，较新工作被祖先删除连带清掉。
- Minimal fix: 每个对象按自己的标记决定去留，幸存条目/分组恢复所需祖先；完成全部条目/分组移动后，按子先父后只删除空候选分组。保留祖先清除旧标记，Deleted 事件只对应真实删除；临时新增又移除的空祖先不产生虚假事件。
- Better long-term fix: 单独复核全部删除标记传播和生命周期、相同时间冲突、未知时间的方向差异、回收站及真实客户端/provider；不能把递归 max 时间当成完整删除策略。
- Regression test suggestion: 深层较新条目和分组编辑双向合并/保存重开/重复合并保留，旧兄弟条目和空分组仍删除；移出对象不连带删除；父标记晚于子编辑也不能覆盖子自己的较新胜出；无子标记不发明删除，实际事件与幸存标记一致，空分组相等/缺失/未知时间保持既有边界。
- Estimated effort: 3–6 小时（原生/registry/官方复核、删除顺序修复和完整验证）

d1fe8c4 已修复。nested-deletion-native-green-final.log 四项原生回归通过：
两项真实深层编辑与永久删除，检查重开、两个方向、重复合并、旧兄弟节点及
祖先标记；两项移出后删除祖先，条目使用实际 move_entry，分组无第一方
移动命令，因此用依赖 tracked move_to 模拟外部客户端并执行真实 delete_group。
新增三项依赖回归覆盖独立父/子标记、无子删除标记、实际事件，以及空分组
时间边界。35 项 merge 模块测试通过（nested-deletion-dependency-merge-final.log），
11 项补丁专项测试通过（nested-deletion-dependency-all.log）。

明确排除无效证据：registry 首次因未声明 chrono 而编译失败，改用 std
Duration 后才取得两个有效红测；移动边界首次误调用不存在的 move_group，
也未执行测试。条目移出夹具随后因 pin_times 清空旧 LocationChanged 而
触发现有“缺时间保留 destination”策略，检查证明祖先仍非空，并非产品
修复后空组残留；补齐明确旧位置时间后通过。没有修改已有缺时间移动策略。

本项保留内容时间优先、无内容时间才回退位置时间的既有永久删除策略，
没有擅自改成 max。空组具体时间相等仍按旧策略删除，未知删除时间且
具体更新时间的 source-only/destination-only 差异也显式保留和记录；因此
不宣称与官方等时/未知时间策略完全一致。所有源标记的全局传播和条目
旧标记清理仍是独立后续边界，本次仅移除幸存祖先的相关旧标记。

verify-nested-deletion.log exit 0：323 frontend、649 Rust、ACL 2，format、
check 零错误警告、150-key 契约、build、clippy 全部通过。Rust 单线程、
无过滤、临时 CARGO_PROFILE_DEV_DEBUG=0，原生 live clipboard 保持 opt-out。
最终四文件补丁从原始 archive 在全新目录重建，60 hashes/56 untouched
文件均符合 provenance；版本、Cargo 接线和未拥有的 vendor 文件未变。
维护 gate 更新 data-contracts/security-model/project-structure、KEEPASS_PATCH
和同步 TODO 子边界（保持 [~]）。报告提交仅更新证据，无额外维护引用变更。

本项无 UI/CSS、IPC/schema、秘密返回改变，没有实际 Tauri handler、用户库、
共享剪贴板、云服务/provider、真实 Android/第三方客户端运行或安全擦除证据。
当前详细卡片 69 Confirmed（Critical 1 / High 17 / Medium 40 / Low 11），
原始 b7c5d9f 的 15 项/6.1 分保持。整体目标 active；手机 v1.6.5 的
另存为 ACL 源码修复已确认，替换 APK/真机/provider 验证仍缺。

### Finding: SP-S59 缺失对象的删除标记未合并，第三副本使永久删除失效（补充发现）

- Severity: High
- Confidence: High
- Category: Data integrity / Privacy / Synchronization
- Status: Confirmed
- Affected area: merge_databases and the absent-object DeletedObjects registry
- Evidence:
  - File: src-tauri/src/vault/merge.rs:433/477–481（daa0a05 修复前）；src-tauri/vendor/keepass-0.13.22/src/db/merge.rs:89–94/219–221/480–495/768–781
  - Function / Module: first-party merge_databases and pinned Database::merge deletion branches
  - Relevant behavior: 依赖只在实际移除 destination 中已有的条目/分组/图标时写来源标记。双方都缺失的对象不进入这些分支；来源独有的标记被丢弃，双方同 UUID 的较新删除时间也未联合。第一方包装层未补全这张注册表。
  - Reproduction: deletion-registry-native-red-final.log 四项实际应用/保存重开回归失败。两个副本均用真实 delete_entry 或三层 delete_group 永久删除，生成每个对象的具体标记后固定合成 t20/t40；第三实际会话 flags 编辑固定 t30。先合并两个已删除副本并保存重开，再与第三副本合并保存重开，条目/子树复活。另两项使用实际创建条目/分组前已保存重开的同根库作为从未见对象的接收端；来源真实删除后标记也未传入，下一次旧副本合并复活。原始红测均停在第一个方向的复活断言，后续方向/标记/重复断言由最终绿测覆盖。
  - Independent verification: 无 Cargo patch 的原始 registry keepass =0.13.22 四项直接 API/save/reopen 探针同样复活（deletion-registry-independent-red.log），当前 metadata 再确认 registry 来源。固定官方 KeePass commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5 的 PwDatabase.cs:925–928/1019–1064 总是合并删除表，MergeInDeletionInfo 保留较新的 DeletionTime，不要求对象当前存在。来源复用 merge-location-reference-provenance.json；文件 SHA-256 3470bf8193e95d211d422eafba901a39a68f2a4d796ce11a0e44778d87ad01b0 已核对。未运行官方客户端。
- Problem: 即使真实删除已经写入并保存了标记，后续合并仍可能把传播所需的标记或最新时间丢掉；两副本当前均无对象的表象掩盖了第三副本风险。
- Why it matters: 永久删除意图无法在多副本同步中可靠传递，较旧凭据或整个子树会返回，按受限多副本冲突路径的 High；未证明真实用户数据泄漏或遭第三方读取。
- Realistic failure scenario: 两台设备分别删除同一记录，较早删除的副本接收较晚删除副本却保留旧时间；第三台设备上传两次删除之间的编辑，合并将其当成较新编辑恢复。创建后删除、另一台尚未见过的记录也能从旧副本返回。
- Minimal fix: 在实际第一方同步入口完成合并和本地回收站恢复之后，对最终缺失 UUID 联合双方原始 DeletedObjects 与现有 merged 标记，保留较新具体时间，具体时间优先未知时间；最终幸存条目/分组/自定义图标不导入新标记。
- Better long-term fix: 继续验证活跃对象旧标记生命周期、回收站特殊标记、等时/未知时冲突和真实三客户端/provider；另行复核不再引用的源附件字节，不将删除表正确等同于安全擦除。
- Regression test suggestion: 实际单条/三层子树删除和中间编辑三副本复现；从未见对象的实际创建前接收端；两个合并阶段的双向保存重开/重复合并；双方独有/较旧/较新/未知标记联合不倒退，晚于最新删除的真实编辑及其附件仍保留；最终活动/回收站/图标不引入新标记；失败合并不改变输入注册表或对象。
- Estimated effort: 3–6 小时（真实三副本/registry/官方复核、最终树注册表修复及完整验证）

1b04387 已修复。新增私有 merge_absent_deletion_info，在 restore_bin 成功后
调用；最终所有 entry/group/custom-icon UUID 构成集合，双方原始注册表
逐项与现有 merged 标记取最新具体时间。先前合并阶段可能覆盖的本地
时间也从原始输入恢复。复杂度为最终对象数与双方标记数之和；没有新增
IPC/schema 或全局状态，唯一应用 Database::merge 调用仍经这个包装层。
没有改 vendor：原始 registry 缺陷仍由第一方补充处理，升级不能替代回归。

deletion-registry-native-green-final.log 七项回归通过。四条主体路径包含
实际 API、明确创建前副本、每个具体删除标记、两个阶段的双向 KDBX
保存重开/重复合并；两条最新删除场景还验证 t50 的真实 flags 反向修改
仍胜出并保留附件。三项边界验证八种独立/未知/新旧时间组合，最终活动
或回收站分组/条目和根自定义图标不引入标记，等时内容冲突仍失败且
输入删除表及对象不变。已有幸存对象标记和本地 bin collision 标记政策
保持；这不是对所有活跃对象旧标记进行全局清理。

原始四项有效红测先用移除后的无标记状态代表未见对象，随后改为实际
创建前已保存重开的同根库；最终四项再次失败并作为主要复现证据。
首次修复绿测的两项主体已不复活，但随后“附件总数应为 0”的断言失败：
没有条目引用时仍有一个源二进制随合并保存。此观测保留于
deletion-registry-native-green-initial.log，独立于注册表修复；最终主体回归
验证记录/注册表，较新幸存版本验证实际附件字节。未引用附件保留的
责任/删除语义仍待单独 probe，本项不认定二进制安全擦除，也不把这个
观察自动计为另一项已确认缺陷。没有为使测试通过而修改附件所有权代码。

verify-deletion-registry.log exit 0：323 frontend、656 Rust、ACL 2，format、
check 零错误警告、150-key 契约、build、clippy 全部通过。Rust 单线程，
无过滤、临时 CARGO_PROFILE_DEV_DEBUG=0，原生 live clipboard 保持 opt-out。
60 文件/4 补丁 provenance gate 通过，vendor/version/Cargo 接线未改变。
维护 gate 更新 data-contracts/security-model/project-structure 和同步 TODO
的已验证子边界（保持 [~]）；报告提交仅更新证据，无额外维护引用变更。

本项无 UI/CSS、秘密返回或用户设置改变，没有实际 Tauri handler、用户库、
共享 OS 剪贴板、云服务/provider/Android/第三方客户端运行或安全擦除证据。
当前详细卡片 70 Confirmed（Critical 1 / High 18 / Medium 40 / Low 11），
原始 b7c5d9f 的 15 项/6.1 分保持。整体目标 active，继续未引用附件边界
及其余系统/竞品/UI；手机 v1.6.5 另存为 ACL 源码修复已确认，APK/真机待验。

### Finding: SP-S60 跟踪永久删除释放临时保留后留下无引用附件（补充发现）

- Severity: Medium
- Confidence: High
- Category: Privacy / Data integrity / Storage
- Status: Confirmed
- Affected area: tracked permanent entry/subtree/bin deletion and persisted attachment ownership
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/types/entry.rs:403–431/681–714/812–820/937–960（472ecb6 修复前）；src-tauri/vendor/keepass-0.13.22/src/db/types/mod.rs:245–285
  - Function / Module: EntryMut::track_changes, EntryMut::remove, EntryTrack::remove/drop, reindex_entry_attachments
  - Relevant behavior: 本地补丁为尚未加入历史的当前附件持有计数保留。永久删除先移除 entry 及全部反向引用；历史附件无此保留可正常回收，当前附件暂留。drop 无条目可写入快照，解除保留后重建索引；已删除条目的 desired 与旧反向引用均为空，原当前 ID 不进入 affected 集合，二进制持续保存。
  - Reproduction: tracked-delete-native-red-valid.log 六条实际路径失败：禁用回收站的单条/批量/三层子树、回收站内单条/三层子树及 empty_recycle_bin。每个目标与幸存者均经实际 update_entry 生成不同的当前/历史附件、save/close/open；删除再 save/close/open 后附件数分别比预期多 1/2/2/1/2/3。失败均在后续 merge 之前，不把未执行的合并断言当红测证据。
  - Independent verification: 独立无 patch 的 keepass =0.13.22 项目真实 tracked remove/save/reopen 后附件数为 0，两项原版对照通过（merge-orphan-registry-control.log）；keepass-orphan-registry-metadata.json 确认 registry 来源，原 archive SHA-256 a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e。原版没有本地保留机制；本项归因于本地补丁，而非声称原版普通移除 API 不履行合同。
- Problem: 用户永久删除了拥有者及历史，当前附件却继续以无引用二进制存于新保存的 KDBX 内；应用列表看不到这些残留。
- Why it matters: 删除语义与持久化内容不一致，重复增删附件还会增加库体积和保存/同步成本。按有约束的内容保留及增长风险定 Medium；附件仍受整个 KDBX 加密，没有证明明文泄漏、第三方读取或安全擦除承诺。
- Realistic failure scenario: 用户永久删除带附件的记录或清空回收站，以为该新保存库已经不含这些载荷；以后继续同步这份库，残留仍随文件保存和传输。
- Minimal fix: EntryTrack drop 重建索引后显式审查其 retained_attachments 中已无保留、无反向引用的 ID，递归核对最终所有 current/history 拥有者，仅回收真正无主的本次释放载荷；不全库 GC。
- Better long-term fix: 独立验证 prepared merge 保留结束与最终历史/回收站恢复后的清理契约；源旧副本、既有孤立头数据和文件系统/备份擦除分别处理，不将本项回收推广为全面擦除。
- Regression test suggestion: 六条真实永久删除 API 生成当前/历史附件、保存重开并保留未删条目原字节；别名共享 ID、另一 current/嵌套 history 所有者、嵌套 guard 最后一个保留释放、额外准备阶段保留、原先无引用头数据；原版 registry 对照、无 _merge feature 编译和完整 provenance gate。
- Estimated effort: 2–4 小时（独立对照、六路径回归、最小 drop 修复与包来源验证）

fb3b9c3 已修复。仅将本次解除保留的候选纳入回收，现有索引重建逻辑不变。
只有候选非空时递归读取当前数据库的拥有者；仍被 current/history 引用
或仍有保留的附件不会被回收，未参与此次保留的无引用头数据保持。
现有递归 collect_entry_attachment_ids 取消 _merge 限定供普通跟踪使用；
没有新增公开 API、IPC、schema 或持久化字段。批量/子树删除原本逐条
处理，本项未证明大库性能改善，性能基准仍缺证。

tracked-delete-native-green.log 八项通过，增强已有六条路径而非新增
同名空测；初始未提交的两项 merge-orphan 回归在实际删除前置断言失败，
据此改为验证真实删除，保留其 merge-orphan-native-red.log 观察但未将
尚未运行的合并段当成证据。最终六路径同时检查幸存当前字节与历史
字节，随后原有双向旧副本合并/保存重开/重复及 tombstone 断言通过。
本项对合并结果没有“全部附件为 0”的断言，合并导入残留单独追踪。

tracked-delete-vendor-green-final.log 14 项通过，其中三项新增覆盖别名、
共享 current/原始嵌套历史、双层真实 guard、模拟额外 prepared 计数和
不相关孤立头数据。现有真实 prepare/merge 共享拥有者案例仍通过；
模拟计数不是完整 provider/merge finalization 验证。首版嵌套夹具使用
add_entry 自动去掉嵌套历史，已改为原始嵌套结构再绿测；两次私有字段
编译失败和该夹具失败均不用于扩大产品缺陷结论。

verify-tracked-delete.log exit 0：323 frontend、656 Rust、ACL 2，format、
check 零错误警告、150-key 契约、build、clippy 全部通过。Rust 单线程、
无过滤、临时 CARGO_PROFILE_DEV_DEBUG=0，live clipboard 保持 opt-out。
tracked-delete-no-merge-check.log 无 _merge 的 save_kdbx4 编译通过。
tracked-delete-reconstruction.log 从新拷贝原始 archive + 零上下文补丁
重建全部 60 hash；仍为 4 补丁 / 56 原文件。仅 entry.rs 的 drop/tests
和 types/mod.rs 的 helper gate 改动，未扩散到 untouched group.rs。
维护 gate 更新 data-contracts/security-model/project-structure、补丁说明
与专门删除附件 TODO；完整同步 TODO 继续 [~]，未宣称跨客户端完整兼容。

删除修复后，独立 prepared 对照的两个方向均在合并结果最终附件数
失败（merge-orphan-patched-independent-red.log）；删除前置阶段已通过。
该观察显示第二条准备阶段无引用载荷清理路径，保持独立待审计记录，
本项没有在 complete_attachment_merge 提前回收可能尚属 losing-history
或本地回收站的载荷。独立探针不会读取真实库、云端或共享 OS 剪贴板。

本项无 UI/CSS/版本或秘密返回改变，没有原生 Tauri handler、真实用户库、
Android/APK/device/provider 或第三方客户端运行证据。手机 v1.6.5 另存为
ACL 修复仍待替换 APK/真机验证。71 Confirmed（Critical 1 / High 18 /
Medium 41 / Low 11），原 b7c5d9f 的 15 项/6.1 分保持，整体目标 active。

### Finding: SP-S61 合并重新保存已拒绝记录的无引用附件（补充发现）

- Severity: Medium
- Confidence: High
- Category: Privacy / Data integrity / Synchronization
- Status: Confirmed
- Affected area: prepared canonical binary merge and final history/bin ownership
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/db/types/mod.rs:323–374/379–397；src-tauri/src/vault/merge.rs:452–453/468/500–506（7a22885 修复前）
  - Function / Module: prepare_attachment_merge, complete_attachment_merge, first-party merge_databases
  - Relevant behavior: 准备阶段把来源 current/history 载荷导入 destination 并临时保留全部 prepared 二进制。对象随后因较新删除标记被拒绝或移除，完成阶段仅重建反向引用和解除保留；第一方历史联合/回收站恢复后没有最终回收，已无拥有者的载荷仍在 binary pool 并被保存。
  - Reproduction: SP-S60 删除修复后，merge-orphan-native-red-valid.log 三项实际应用/save/reopen 失败。实际单条/三层子树永久删除保存重开时附件数已为 0，随后与原两附件 current/history 旧副本合并，entry/group 保持删除但附件数为 2 而非 0。另一实际 KDBX 夹具解除历史引用并重建索引，创建一个本地原有孤立头；合并后应只保留这一个旧头，却保留两个。红测在首个方向最终数量断言失败，第二方向和重复/字节断言由绿测覆盖。
  - Independent verification: 同一原始外部夹具在无 patch registry keepass =0.13.22 两方向控制通过（merge-orphan-registry-control.log）；加本地 prepare/complete 后，两方向均最终 num_attachments 1≠0（merge-orphan-patched-independent-red.log），删除前置为 0 已通过。增加最终 finalize 调用后，两项通过（merge-orphan-patched-independent-green.log）。独立项目不经第一方 merge；registry/local metadata 确认来源，原夹具和 SHA256/feature 差异保存在 merge-orphan-independent-before-finalization.rs 与 merge-orphan-independent-provenance.json。原 registry metadata 的 Cargo 下载提示已分离到 raw.log，JSON 由原捕获内容解析生成，未事后替换来源。
- Problem: 记录的删除标记正确，仍会在准备阶段把其当前/历史附件重新导入，或保留已删除的 destination 载荷；可见树正确掩盖了文件内容残留。
- Why it matters: 删除后的新合并库持续携带不需要的载荷，反复合并不同旧版本可增加保存和同步成本，删除保留政策也不一致。按受限的附件保留/增长风险定 Medium；整个 KDBX 仍加密，未证明明文泄漏、未授权读取或磁盘安全擦除失效。
- Realistic failure scenario: 用户在一台设备永久删除带附件的记录并保存，随后与另一台旧副本同步；记录未复活，但旧附件内容再次写入合并文件并跟随后续保存和同步。
- Minimal fix: 准备阶段返回 opaque AttachmentMergeState 记录 prepared ID 与原始 destination 孤立头 ID；第一方完成实际 winner、equal/losing histories 和本地 bin 恢复之后，核对最终递归拥有者/保留/载荷，仅回收 prepared 范围内无主且非原孤立头的 ID。
- Better long-term fix: 继续真实跨客户端和大型附件库验证；原孤立头、来源孤立头排除、merge 后新建载荷与旧文件/备份的保留政策分开。早于历史/回收站恢复的全库 GC 会破坏迟到的合法拥有者，不适合作为本项替代。
- Regression test suggestion: 实际单条/子树删除、current/history 两载荷、双向合并/保存重开/重复、marker/原输入不变；本地原孤立头字节保留且来源孤立头不导入；canonical alias 原头先/后编号、晚加入二进制/嵌套历史、仍有保留/缺失载荷前置拒绝；既有全部 current/losing/equal-history/bin/shared/protection 回归与独立 registry/prepared/finalized 对照。
- Estimated effort: 3–5 小时（区分原头保留、最终时序设计、两层独立回归及包来源验证）

879d37f 已修复。prepare_attachment_merge 返回只含 ID 集合的 in-memory
AttachmentMergeState；原始孤立头 ID 在 canonical 引用变更前记录，
prepared ID 在导入后记录。complete 继续安装 current winners 并解除
准备保留，不提前回收；merge_databases 在 history union/loser insertion
和 restore_bin 之后调用 finalize，成功后再完成删除注册表联合。
finalize 拒绝尚存保留或任何最终递归拥有者的缺失载荷，失败检查前不
改变 pool；仅删除 prepared 且非原头且无最终 current/history 所有者
的二进制。原头与相同字节的 canonical 共享编号先/后情况均保留原
头的实际 ID/字节；后续加入的 ID 不在清理范围内，不执行全库 GC。
这两个 ID 集合不含附件值、密码或主密钥，没有 IPC/schema/磁盘元数据。

merge-orphan-native-green.log 三项通过。单条和三层子树从实际创建/
update/delete/save/reopen 出发，两方向删除对象和 t40 marker、最终
附件为 0、重复保存重开、原输入两附件不变均通过。原头场景从实际
历史载荷解除引用并保存重开验证 one orphan；local 保留原历史字节，
remote 不导入孤立头，两方向重复合并符合既有政策。

merge-orphan-vendor-green.log 17 项通过，其中三项新增验证原头与
当前内容相同的 canonical alias 两种编号顺序、准备后引入的无引用
header、complete 后才写入的原始嵌套历史 owner、active lease 和
missing payload 前置拒绝保持 entries/pool/leases。直接生命周期夹具
使用空 MergeLog 模拟最终恢复时序，实际 native merge 与完整门验证
是应用行为证据；不把这个模拟称为真实 provider 或客户端互操作。
既有 prepared shared-owner、protection/dedup 与 unwind 回归仍通过。

verify-merge-orphan.log exit 0：323 frontend、659 Rust、ACL 2，format、
check 零错误警告、150-key 契约、build、clippy 全部通过。包括原有
current-source/created/shared/branched/equal/losing-history 和回收站
历史附件的实际合并/保存重开回归。Rust 单线程、无过滤、临时
CARGO_PROFILE_DEV_DEBUG=0，live clipboard 保持 opt-out。无 _merge
save_kdbx4 检查通过；原包 + zero-context patch 新目录重建全部 60
hash 通过，4 patched / 56 untouched 不变。维护 gate 更新三引用、
patch 说明/专项测试数和专门 TODO；完整同步 TODO 保持 [~]。

本项无 UI/CSS/版本/公开秘密返回改变，无真实 Tauri handler、用户库、
云服务、第三方客户端、Android APK/device/provider 或物理擦除证据。
手机 v1.6.5 另存为 ACL 修复仍待替换 APK/真机验证。72 Confirmed
（Critical 1 / High 18 / Medium 42 / Low 11），原 b7c5d9f 的 15 项/
6.1 分保持，整体目标 active，继续其余系统/竞品/UI 边界复核。

### Finding: SP-S62 Argon2 参数截断后实际派生成本远小于文件声明（补充发现）

- Severity: High
- Confidence: High
- Category: Security / Configuration / Data integrity
- Status: Confirmed
- Affected area: shared Argon2d/Argon2id derivation and local/remote database read/write
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/crypt/kdf.rs:75–87；src-tauri/src/vault/persist.rs:165/210/254/292/412/454（3602a93 修复前）
  - Function / Module: Argon2Kdf::transform_key and every Database::parse/save caller
  - Relevant behavior: KDBX iterations 和 memory 字段为 u64，transform_key 将 iterations as u32 与 (memory / 1024) as u32 不检查转换；高位丢失。4294967297 次实际变 1 次，4398046576640 字节实际变 64 KiB，保存和重开仍保留原声明。
  - Reproduction: kdf-overflow-native-red.log 三条真实应用路径均失败：本地打开、远程 SaveLocal 打开接受原版生成的异常文件；实际 save 接受并发布无法表示的 Argon2id 配置。前两红测停在首个迭代溢出文件的拒绝断言；另一个内存文件和失败后的会话/镜像/旧文件断言在绿测覆盖。原红测的后续断言不是已执行证据。
  - Independent verification: 无 patch keepass =0.13.22 的两项独立 save/parse 探针均失败（kdf-overflow-registry-red.log），实际计算仅 1 次和 64 KiB，保存/重开配置与巨大声明一致。kdf-overflow-registry-metadata.json 确认 registry 来源。固定官方 KeePass commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5 Argon2Kdf.cs:61–65/150–155/213 的 MaxIterations 与 checked KiB 转换明确拒绝越界；SHA-256 329942510a856078e72e5e19ad88cee4879b49c4e120c335c259c4d881d663d6，来源见 kdf-overflow-reference-provenance.json。官方实际内存政策更严格，本项仅修正表示折返；未运行官方客户端。
- Problem: 不受支持的声明被作为低成本参数执行，文件公开加密参数不再描述实际密码派生操作；严格实现无法按相同声明解密这种文件。
- Why it matters: 加密边界缺少必要输入校验，实际派生成本可能远低于其声明，按安全敏感参数缺乏验证定 High。未证明普通 SecPivot 默认库弱化、实际凭据泄漏、鉴权绕过、CPU/内存耗尽或攻击者获得未知密码。
- Realistic failure scenario: 用户打开同类截断生成器产生的高声明参数库并再次保存，应用继续用低成本处理而不拒绝；以后迁往严格客户端会失败，文件上的迭代/内存值也会误导安全评估。
- Minimal fix: 在共用 Argon2 transform 执行前检查 u32 转换，超范围返回已有 TimeTooLarge/MemoryTooMuch，沿全部原生读写失败路径处理，不限于单个 IPC。
- Better long-term fix: 独立设计有效范围内昂贵 KDF 的资源预算、取消与用户反馈；进行受控基准和真实跨客户端验证。本项不实现任意内存上限、不降低合法参数、不自动修复已经错误派生的文件。
- Regression test suggestion: 原版有界合成异常库验证本地/远程拒绝、原会话及 dirty/revision 保留、不生成镜像；失败保存旧文件仍按原 KDF 重开；Argon2d/id 两种变体越界拒绝和正常独立向量；全部 KDF 默认/设置/重加密/合并回归、fixture/hash/来源与完整门。
- Estimated effort: 2–4 小时（独立与实际三路径复现、受审依赖范围扩展、最小转换修复及完整验证）

1ce2452 已修复。crypt/kdf.rs 使用 u32::try_from 检查两项参数，映射到已有
Argon2 错误，再构造并运行 Config。memory 仍按字节 /1024 转为 KiB；
标准算法、AES、盐/主密钥处理及默认 64 MiB/3 次不变。所有 open/save
调用自动覆盖，没有新 IPC/schema/UI 或参数猜测回退。该 upstream crate
为 Rust 2018，首个中间编译未导入 TryFrom，已显式补齐；该编译失败
和 PowerShell helper 转义失败不作产品缺陷证据。

kdf-overflow-native-green-final.log 三项通过。两份空合成 Argon2d KDBX
由原版实际 save/parse 生成，有效 header/HMAC，已存 tests/fixtures/
kdf-overflow-* 及哈希/声明/生产者 provenance。本地/远程打开分别检查
两个文件都拒绝、旧 db/path/dirty/revision 不变；远程原对象不变且
镜像目录未生成。Argon2id 实际保存对两个 overflow 配置均拒绝，旧
文件字节不变且原密码/AES KDF 仍可重开。未修改或读取用户数据库。

kdf-overflow-vendor-green.log 19 项通过，其中两项新增各覆盖 d/id
的溢出错误种类，原有附件/history/merge 所有权专项仍通过。
kdf-normal-vector-green.log 独立显式 Argon2 Config 的 64 KiB 向量通过。
kdf-overflow-independent-green.log 相同外部 save/parse 探针在 local
patch 下两项通过；kdf-overflow-patched-metadata.json 确认实际本地来源。
有界红测不会分配 TB 内存或执行数十亿次迭代；未运行表示范围内的
资源耗尽测试，不能从安全的小成本复现推出进程 OOM/挂死。

verify-kdf-overflow.log exit 0：323 frontend、662 Rust、ACL 2，format、
check 零错误警告、150-key 契约、build、clippy 全部通过。Rust 单线程、
无过滤、临时 CARGO_PROFILE_DEV_DEBUG=0，live clipboard 保持 opt-out。
包含默认 Argon2id、AES/Argon2/Argon2id 写重开、正常参数和现有全库
行为回归。新的第五受审文件 crypt/kdf.rs 纳入 verifier/provenance/
zero-context patch；55 upstream 文件逐字节不变，原 archive/许可证与
全部 60 hash 保持，kdf-patch-reconstruction.log 在新目录重建验证。
维护 gate 同步 maintenance-workflow/data-contracts/security-model/
project-structure、patch 说明/专项数和独立 KDF TODO；没有修改原来的
四个补丁文件内容。SKILL.md 的稳定路由无需额外更新。

用户新增“避免出现安全问题”已纳入后续安全边界优先审查。当前发现
普通 Bitwarden字段会把意外类型静默转为空值，仍为待进一步按官方
schema 与实际写入复核的候选，不计 Confirmed；KDF 有效大参数的
预算/取消、其他系统/UI/竞品迁移和手机 v1.6.5 另存为新 APK/provider
继续验证。无真实 Tauri handler、云服务、用户库、实际剪贴板、原生
第三方客户端、Android device/APK 或完整安全保证证据。73 Confirmed
（Critical 1 / High 19 / Medium 42 / Low 11），b7c5d9f 原 15 项/6.1
分保持，整体目标 active。

### Finding: SP-S63 Bitwarden 普通字段错误类型被静默丢弃且自定义保护类型被猜测

- Severity: Medium
- Confidence: High
- Category: Data integrity / Type safety / Security
- Status: Confirmed
- Affected area: Bitwarden JSON parser, command rows and KDBX import
- Evidence:
  - File: src-tauri/src/vault/import.rs (8fc838d before repair), import/bitwarden_tests.rs
  - Function / Module: parse_bitwarden_json, command parse_bitwarden_json, actual ImportRow wire -> import_entries -> save/reopen
  - Relevant behavior: name/notes/folderId and login username/password/totp use as_str().unwrap_or(""); invalid fields containers or members are dropped; invalid custom names/values become empty, nonnumeric/unknown field types default to unprotected text. Malformed folder maps are dropped.
  - Reproduction: bitwarden-shapes-observed-final.log passes a diagnostic that actually saves/reopens malformed synthetic input. Four values are empty; a string "1" hidden-field type is stored and exposed unprotected. The temporary diagnostic is removed from final regression tests. Missing get_notes and mut compilation errors are harness errors, not product findings.
  - Independent verification: fixed official Bitwarden commit 1a8bdbfe625bff90a9c0cabef2002e37f6038fa1 export models define ordinary values as strings and fields as arrays of FieldExport; enum 0/1/2/3 specifies Text/Hidden/Boolean/Linked. Four source files and SHA-256 recorded in bitwarden-shapes-reference-provenance.json. This is source contract corroboration, not a claim that official runtime rejects all malformed values; normalizeFieldTypeForSdk itself defaults unknown values to text.
- Problem: malformed input succeeds while losing data; uncertain protection metadata is guessed into an unprotected value.
- Why it matters: the import can look successful even when credentials/notes/custom data are discarded. String hidden metadata demonstrates local exposure through the normal unprotected DTO, but malformed source is required; no normal valid export leak, exploit, unknown credentials or full client interoperability proof.
- Realistic failure scenario: a damaged or transformed JSON file changes a password to an object or a field type to "1"; SecPivot imports empty credentials or a visible custom secret instead of reporting an error.
- Minimal fix: reject unexpected non-null string/container values and invalid custom-field enums before returning rows; keep missing/null optional values and legacy missing field type as Text; use content-free record/folder errors. Do not guess by stringifying values or coercing types.
- Better long-term fix: typed exhaustive boundary models and real client migration tests, including unsupported collections/attachments/multiple URI behavior.
- Regression test suggestion: ordinary string/container errors, malformed members and enums reject whole document even after valid earlier records; invalid folders reject before rows; optional missing/null, numeric 0–3 and legacy missing type preserve semantics through actual command/wire/KDBX reopen and protected DTO masking; all prior import regressions.
- Estimated effort: 2–3 hours for bounded reproduction, minimal parser validation and integration evidence.

2c71c58 已修复。optional_string/optional_array 保留合法缺省/null，拒绝错误
标量/容器；login 非空须为 object，folders 须有可映射字符串 id，普通
名称/备注/登录值和自定义 name/value 须为字符串。字段 type 缺省保留
历史 Text；显式值必须是数字 0–3，既有 type-3 目标 resolver 不变。
整份解析失败不返回部分行，不 stringify/coerce 错误数据，不改变
DTO/IPC/UI。unknown folderId 与重复 id 的既有政策未扩大修复。

bitwarden-shapes-red-final.log 四项中合法对照通过，三项拒绝测试失败：
32 个普通值/容器、20 个自定义成员/类型、11 个分组错误被接受。
bitwarden-shapes-observed-final.log 的单独诊断实际执行 command→wire→
批量导入→保存重开，四值变空及 string "1" 字段未保护并进入 DTO
得到复核；诊断留在证据目录，未作为期待错误行为的回归留在源码。
最早合法备注 CRLF 的对照失败是 KDBX 普通字符串换行归一化的另一个
候选，已明确改为 LF 合法对照后重跑红测；本项不把该现象算作已修复。
初期不存在 get_notes 方法、缺少 mut、独立模块位置/uuid 依赖错误
均为探针搭建错误，不作为产品缺陷。

bitwarden-shapes-green.log 30 个 Bitwarden 测试全部通过，其中四项
新增回归覆盖 63 个错误案例，以及合法 null/缺省/字符串、四种数字
字段类型与历史缺省 Text 的实际保存重开。密码 CRLF、用户名空格、
备注 LF/Unicode、显式 hidden 与 linked 的保护/DTO 遮罩保持。
分组名在 parser 行验证，后续 imported helper 绑定 root；不声称
该专项证明真实前端多级分组创建。前端原导入回归由完整门覆盖。

独立两个小型可执行程序直接编译固定 8fc838d 与修复后 parser 的
四个源文件（hash 在 bitwarden-shapes-independent-provenance.json），
没有 Tauri/测试模块。三个相同异常输入原版全部接受，修复版全部拒绝，
正常字符串/隐藏值对照均保持；见 independent-*-final.log。独立
Cargo.lock 保留依赖版本；这是 first-party 源码隔离复核，不是官方
Bitwarden 运行时或 registry parser。官方四个固定提交导出模型/枚举
及 hash 佐证字段契约；未知类型 normalizeFieldTypeForSdk 的 fallback
明确保留为官方偏差说明，未把官方源码说成运行时全拒绝证明。

verify-bitwarden-shapes.log exit 0：323 frontend / 666 Rust / ACL 2；
format、零错误警告 check、150-key 契约、build 和 clippy 均通过。
Rust 单线程、debug=0、无测试过滤，live clipboard opt-in 保持 unset；
四个原生剪贴板测试体未执行，不计原生互操作。受审 keepass 五补丁/
60 hashes 未改变。维护 gate 同步 data-contracts/security-model/
project-structure 和独立 TODO；maintenance-workflow 与 SKILL 路由
没有新事实需改。没有真实用户库/共享剪贴板/云服务或 native-client/
Android APK/provider 运行。报告 lint、strict JSON/74 卡片及原始
b7c5d9f/15 项/6.1 分复核后提交。

historyMaxSize 现为 i64 且当前 Windows/Android 发布仅 64 位，对应
as isize 截断候选不能成立为当前产品发现，未计问题。普通 KDBX 字符串
CRLF、有效昂贵 KDF 的预算/取消、其余系统/竞品/UI 边界以及手机
v1.6.5 另存为替换 APK/真机验证继续保留。74 Confirmed（Critical 1 /
High 19 / Medium 43 / Low 11），整体目标 active。

### Finding: SP-S64 KDBX 保存后普通字符串的回车及 CRLF 被归一化

- Severity: Medium
- Confidence: High
- Category: Data integrity / Interoperability
- Status: Confirmed
- Affected area: KDBX4 internal XML serialization for current/history entries, groups and metadata
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/format/xml_db/mod.rs:111–113 (5926f46 before repair), entry.rs::StringValue and group/meta serializers
  - Function / Module: to_xml -> quick_xml::se::to_string_with_root; parse_xml
  - Relevant behavior: serializer writes literal CR, and standards-based XML parsing normalizes CR/CRLF to LF. Ordinary entry values, keys, group and metadata strings can change after successful save/reopen. Inner-protected values are base64 encrypted and retain CR.
  - Reproduction: previous real Bitwarden command/wire/KDBX CRLF note control changed to LF (bitwarden-shapes-red.log), retained as a separate candidate during SP-S63. New native direct add/edit/current+history/group/meta and Bitwarden command/wire imports exercise the actual persisted boundary.
  - Independent verification: unpatched keepass =0.13.22 registry probe actually save/parses a bounded AES-1 synthetic database: nine ordinary CR positions mismatch while protected and literal-entity controls retain values. kdbx-whitespace-registry-red-final.log. The same probe verifies QuoteLevel::Full still normalizes CR; quick-xml 0.41.0 se/simple_type.rs::escape_list escapes XML metacharacters only. Registry source and Cargo.lock remain recorded. No native third-party client execution.
- Problem: successful encrypted persistence changes valid source strings instead of preserving their exact line endings.
- Why it matters: credential usernames, imported multiline notes, custom data, field keys and historical content may change; native/foreign files with unprotected passwords also expose this correctness boundary. No default protected-password weakening or credential disclosure is proved.
- Realistic failure scenario: a valid imported multiline note or custom value with CRLF is saved and reopened; the app reports success while that text and its historical copy now contain LF.
- Minimal fix: represent raw CR as XML character references after trusted serialization, preserving the existing XML parser's standard normalization and inner-encryption behavior. Literal entity text must remain literal. All model fields are text content, not CDATA; no schema/IPC/UI change.
- Better long-term fix: complete scalar fidelity and independent cross-client migration tests. Pure-whitespace entry values are independently emptied by cs_opt_string, a separate pending boundary; this CR encoding change must not be represented as repairing every whitespace value.
- Regression test suggestion: actual current/history/group/meta save/reopen and Bitwarden wire import, protected/literal controls; direct serializer checks character references plus external standards parser, explicit CR entity parse control; full KDF/attachments/merge regressions and patch reconstruction.
- Estimated effort: 2–3 hours for native/independent bounded evidence and minimal serialization repair with full integration.

This card is recorded before changing the serializer. Initial probe compile
failures (temporary GroupMut lifetime, missing helper import and EntryRef clone)
are harness errors, not product evidence. Pure-whitespace registry red stops on
the first space-only case and is not full current-application proof for that
separate finding. Existing SP-S18 concerns explicit plaintext XML export, while
this issue occurs inside the encrypted KDBX's ordinary persistence path.

41a596c 已修复。可信 serializer 输出中 raw CR 编为 &#xD;，不修改读入
文件的标准归一化，不猜测修复旧内容。模型当前字符串为文本而非 CDATA；
保护值仍经原 inner cipher/base64，字面 &#xD; / &amp; / <tag> 由原
serializer 转义，不再二次解析。无 CR 的输出直接 into_bytes，避免
原 as_bytes().to_vec 拷贝；有 CR 时仅执行必要转换，不声称基准性能提升。
不改变 KDF/cipher/schema/IPC/UI。纯空白值的 cs_opt_string 清空和非法
XML scalar 是独立边界，该项未修复、未标完整字符串保真。

kdbx-carriage-native-red-final.log 两个实际应用测试失败，全部完成
保存重开后分别统计 14 个 current/history/group/meta 位置与 4 个
真实 Bitwarden command→wire→batch 字段的 CR 内容变化；密码/隐藏
字段与 literal 控制在红测就通过。kdbx-carriage-native-green.log 两项
通过，包含普通字段名 CR、当前/历史保护标记与值、group notes/name、
root notes 和 metadata description。group 名称/notes 走实际 API；
description 为原生数据库内 seed 后实际 save/reopen。

原 registry keepass =0.13.22 的外部独立实际 save/parse 探针先发现
9/9 个普通 CR 位置变化，Full QuoteLevel 控制依旧会归一化；源/lock
在 kdbx-carriage-registry-metadata.json 与 original-Cargo.lock 记录。
同一探针用明确 local patch 后 CR 9/9 全保留；对应 metadata source
为 null。独立套件整体仍有 1 个 pure-whitespace 失败（首个 space-only
案例，6/6 值清空），2 个 CR/Full 控制通过，见 patched-pending.log；
未把这份套件说成全绿，也未把该独立边界当成当前 native 全覆盖。
这些 AES-1 固定合成库探针计算有界，没有用户数据或昂贵 KDF。

依赖回归两项新增，先 writer red1/parser control green1，最终 21 个
secpivot_ 专项全通过；no-default-features + save_kdbx4 编译通过。
真实 to_xml 的 synthetic fixture 经临时测试生成、probe 随后移除。
独立 Python ElementTree 读取它时 CR key/value、literal 和 Protected
base64 控制保持；还验证 raw-CR 对照按标准归一化。该 fixture 为 Plain
inner cipher 的合成 XML，不是用户文件或完整加密 KDBX；真实加密路径
由应用和外部 save/parse 单独验证。XML/source hashes 见 standard
provenance。临时 GroupMut lifetime、helper import、EntryRef clone
编译错误及 docs patch context 失配不算产品问题。

竞品源固定官方 KeePass commit d1eec63f1bd73dc2d5273eaf94528f616b553ce5：
KdbxFile.Write.cs:203 使用 XmlUtilEx.CreateXmlWriter，XmlUtilEx.cs:139–156
工厂未显式设置 NewLineHandling/Entitize。两文件 hash 在 reference
provenance 中；不能仅据这些代码声称官方完整混合回车保真，未运行
客户端。选择标准字符引用的依据是实际复现与独立 XML parser 结果，
不盲目复制竞品默认行为。

verify-kdbx-carriage.log exit 0：323 frontend、668 Rust、ACL 2；format、
零错误警告 check、150-key 契约、build、clippy 均通过。根完整门单线程/
debug=0/无过滤，live clipboard opt-in unset，4 个原生 live 测试体
未执行。独立探针使用其记录的默认构建/调度，不混称根门配置。
补丁仍 5 文件，仅 xml_db/mod.rs 改变，另外 4 patch bytes 保持；
55 upstream 文件字节不变，fresh archive+patch 重建全部 60 hashes
通过。维护 gate 同步 data/security/structure、patch 文档/21 专项数与
独立 TODO；SKILL/maintenance 路由无需改。报告 lint、strict JSON、
75 卡片和 b7c5d9f 原 15 项/6.1 分保持后提交。

手机 v1.6.5 另存为仍待新 APK/真机/provider，pure-whitespace native
边界、有效昂贵 KDF 预算与其余系统/竞品/UI 继续审查；无用户库、
实际剪贴板、云服务、第三方客户端或完整安全保证证据。75 Confirmed
（Critical 1 / High 19 / Medium 44 / Low 11），整体目标 active。

### Finding: SP-S65 KDBX 打开和重写会清空纯空白条目值

- Severity: Medium
- Confidence: High
- Category: Data integrity / Interoperability
- Status: Confirmed
- Affected area: Native encrypted-KDBX current/history ordinary string fields and external-file opening
- Evidence:
  - File: src-tauri/vendor/keepass-0.13.22/src/format/xml_db/entry.rs::StringValue; custom_serde.rs::cs_opt_string; quick-xml 0.41.0 src/de/map.rs:237–239,274 and src/de/mod.rs::skip_whitespaces (92c1d14)
  - Function / Module: parse_xml -> ElementMapAccess -> optional string conversion -> xml_to_db_handle
  - Relevant behavior: XML object mapping skips blank text events before a field can deserialize, and cs_opt_string separately maps Unicode trim-only text to None. xml_to_db_handle turns absent text into an empty string. Unprotected values can therefore disappear on a successful open; protected Base64 values retain their original decoded content.
  - Reproduction: kdbx-whitespace-native-red-final.log contains two complete native failures: 48 changed current/history positions across six whitespace variants; 50 changed ordinary positions across initial open and save/reopen of the original-registry fixture. Protected values and flags, empty and ordinary controls retain their semantics.
  - Independent verification: original crates.io keepass =0.13.22 produces a seven-entry AES-1 synthetic encrypted fixture. Database::get_xml extracts its actual internal XML without entry deserialization; Python ElementTree independently confirms all 35 source ordinary values and seven protected attributes. Source/lock/fixture/XML hashes and registry identity are recorded. Direct library final red is one whitespace failure and one empty/missing/protected-contract pass. The earlier independent registry save/parse whitespace failure corroborates this; no native competitor client was run.
- Problem: valid all-whitespace field content becomes an empty string during ordinary database opening and can then be written back as empty.
- Why it matters: whitespace can be a legitimate username, imported custom value or an explicitly unprotected password; history can also lose the original value. This is a fidelity failure, with no proved default password exposure, protection downgrade or credential transmission.
- Realistic failure scenario: an external database contains a single-space ordinary field; SecPivot opens it successfully and an unrelated save persists that field as empty, losing the source value and any affected historical copy.
- Minimal fix: provide a scoped XML text-decoding path that retains whitespace for String/Value, and keep the generic optional-metadata trim rule out of entry values. Merely removing cs_opt_string or using direct String deserialization does not fix skipped ASCII whitespace; both withdrawn attempts remained red. Preserve inner encryption, empty-value semantics, missing-Value rejection, history and optional metadata behavior.
- Better long-term fix: establish a supported library-level text-preservation contract and independent client fixtures; avoid an unverified raw-text substitution or applying a global whitespace change to all metadata. Group/meta whitespace, invalid scalars and already-lost values require separate evaluation.
- Regression test suggestion: reapply the hashed red regression patch with its synthetic fixture; require actual current/history save/reopen, original-file open/rewrite, ASCII/Unicode whitespace, protected/unprotected password controls, empty/missing values and optional metadata. Include interleaved/nested/escaped XML cases appropriate to the final parser design, patch provenance and the complete integration gate.
- Estimated effort: 4–8 hours for a scoped parser/library fix, independent evidence and integration checks.

确认阶段（949c641）尚未修复。已撤回两个未通过的限定转换尝试；应用源码与五个现有
vendor patch 文件全部恢复为 92c1d14 的确切字节。红测 patch、源码快照、
合成 fixture/provenance、实际内部 XML 和失败日志保存在本任务证据目录。
fixture 不含用户内容，测试解锁值为公开 synthetic-only，KDF 仅 AES-1。
纯空白不等于 SP-S64 普通 CR 文本归一化：回车编码修复仍保留，而本项
在读取现有外部库时就可触发。内部 XML 的 ElementTree 检查只验证源
数据完整与保护属性，并不证明其独立解密、原生竞品或真实迁移兼容。

测试搭建阶段错误地假设缺少整个 Value 元素会被原实现接受；该控制
已改为期望原 missing-field 拒绝，最终仅空白复现失败，不计第二个产品
问题。原版 registry fixture 不含 CR，六种 native 输入另含 CR/混合 CRLF。
48 与 50 是完成对应检查后的不一致位置数，不是独立 bug 数。根红测
RUST_TEST_THREADS=1/debug=0/live clipboard unset；外部生产者默认 profile
独立记录，不混称配置。没有访问用户库、剪贴板、云服务或真机 provider。

源码恢复后沿用 SP-S64 的完整门证据（323 frontend / 668 Rust / ACL 2），
该全绿结果不能表示本项已覆盖或已修复。新的报告和文档门另行检查。
当前累计 76 Confirmed（Critical 1 / High 19 / Medium 45 / Low 11）；原
b7c5d9f 的 15 项/6.1 分保持。手机 v1.6.5 替换 APK/provider、有效昂贵
KDF 预算、其余系统、竞品与 UI 仍需继续，整个目标 active。

7c1cbe1 已完成限定修复：StringValue 从 `$value` 改为 `$text`，通过
直接 String 转换保留 Unicode 空白；本地固定 QuickXML 0.41.0 的
ElementMapAccess 仅在目标结构明确声明 `$text` 时保留空白事件。
结构缩进、一般 `$value` 与可选元数据维持原规则，未引入原始 XML
替换、临时标记或另一套解析器。保护属性／内部加密 Base64、空 Value、
缺失整个 Value 的拒绝、历史和普通回车处理的控制均通过。

本轮验证从原生两个失败转为两个通过：当前／历史六种 ASCII、CR/CRLF
和 Unicode 输入及原版 registry 合成库的打开／重写均保真。KeePass
23 个专项测试通过；QuickXML 开启 serialize 的 library 测试 1363
通过、4 ignored，包含新增明确文本／属性／CDATA／注释／PI 和结构
控制。独立探针固定原测试源码并指向当前两个 vendor，3 个测试通过，
包括普通 CR、五种 ASCII 空白、保护和字面实体控制；原生测试另覆盖
Unicode。独立默认 profile／调度不混称根门配置。

verify-kdbx-whitespace.log exit 0：323 frontend、670 Rust、ACL 2，
两套来源门、format、零错误警告 check、150-key 契约、build 和 clippy
通过。根门单线程／debug=0／无过滤，live clipboard opt-in unset；
4 个原生剪贴板测试体及 QuickXML 4 个 ignored 不计执行覆盖。QuickXML
测试源副本作为根包时出现 unused patch 提示，不影响执行该副本的
library tests；应用和独立探针的依赖路径另有来源验证。

KeePass 60 文件中新增第六个 owned patch entry.rs，其余原有五个 patch
字节全部保持，54 个 upstream 文件不变。QuickXML 完整 43 文件／MIT
包中只改 map.rs，42 文件保持原字节，registry 0.42 包未改。
原始 archive 加补丁重建 60／43 哈希及全部暂存字节检查通过。
搭建时的 diff 换行规范化导致重建失败，已用保留 CRLF 载荷的 diff_bytes
修正；最终依据 kdbx-whitespace-reconstruction-raw.log，不将搭建错误
计为产品问题。Git attributes 局部禁用新包／补丁 EOL 规范化，格式门
仅覆盖 owned source；测试在经路径验证的临时源码副本中运行，保留
published Cargo.lock 原字节。

竞品复核采用 KeePass 镜像固定 d1eec63f1bd73dc2d5273eaf94528f616b553ce5：
KdbxFile.Read.cs:759–781 的 ReadString/Raw 调用 ReadElementString，
Streamed.cs:101、512–513、958 后连接 reader factory 和普通 Value 路径；
XmlUtilEx:98–105 设置 IgnoreWhitespace=true。相同 System.Xml 设置和
ReadElementString 的 .NET 10.0.14 helper 控制跳过 3 个 ASCII 空白，
保留 2 个 ordinary/empty 控制。这只校准辅助方法，未运行 KeePass
应用／完整加密 KDBX，不能推出竞品真实迁移丢失。QuickXML 0.42 缓存
源码仍无明确文本分支，仅源码观察，未声称其运行验证。

文档 gate 已同步 SKILL/maintenance、data/security/structure、两套补丁
说明和 TODO。此项维持 Confirmed 并记录已修复，累计仍 76，不新增
重复卡片、不改原始基线。修复不能恢复已丢失内容；分组／元数据纯
空白、非法 XML scalar、完整跨客户端／真机 provider 和有效昂贵 KDF
预算仍待验证，整体目标 active。

### Finding: SP-S66 KDBX 保存发布了 XML 1.0 禁止字符

- Severity: Medium
- Confidence: High
- Category: Data integrity / Interoperability
- Status: Confirmed
- Affected area: Native encrypted KDBX serialization after structured import
- Evidence:
  - File: src-tauri/src/vault/import.rs::parse_bitwarden_json; entries.rs::import_entries; serialize.rs::write_fields; vendor/keepass-0.13.22/src/format/xml_db/mod.rs::to_xml (9798b77)
  - Function / Module: parse_bitwarden_json command -> ImportEntryInput wire -> bulk import -> native save -> XML serializer
  - Relevant behavior: JSON strings retain illegal ordinary XML characters, and the writer emits them without XML 1.0 Char validation. The same native parser accepts the saved file, so self-reopen alone misses the standards incompatibility.
  - Reproduction: kdbx-scalar-native-red.log: one complete failing native regression publishes nine synthetic imports containing U+0000, U+0001, U+0008, U+000B, U+000C, U+000E, U+001F, U+FFFE or U+FFFF; all nine reopen exactly with protected controls intact.
  - Independent verification: actual decrypted internal XML from all nine native files is rejected by Python ElementTree and System.Xml. A separate current-library encrypted-save probe also publishes nine illegal cases and preserves all 15 self-reopens; standard readers reject nine and accept six legal controls (TAB/LF/CR/NEL/U+1FFFE/U+10FFFF). System.Xml .NET 10.0.14 totals six legal accepts and 18 illegal rejects across both producers. These are XML parsing controls, not independent KDBX decryption or native competitor execution.
- Problem: a successful save can publish a KDBX whose internal XML is outside the XML 1.0 character contract.
- Why it matters: a structured import containing an embedded control character can produce a database unsuitable for standard XML readers used by other clients. Existing native values remain readable here; no native self-data-loss or secret exposure is proved. Medium reflects the unusual input and interoperability scope.
- Realistic failure scenario: an exported JSON note contains an escaped U+0000; import and save succeed, but the saved internal XML fails a standard XML reader when the user transfers the database to another implementation.
- Minimal fix: validate actual serialized XML text before encrypted destination publication and return a content-free error on forbidden scalars. Preserve existing destination bytes and live edits; protected Base64 payloads may contain arbitrary underlying string scalars. Do not silently sanitize or turn forbidden characters into equally forbidden character references.
- Better long-term fix: provide field-local editing feedback and independent native-client fixtures for import/export boundaries; keep allowed supplementary noncharacters and legitimate whitespace distinct from XML 1.0-invalid scalars.
- Regression test suggestion: enforce atomic native save refusal through real import, history/group/meta/key boundaries; allow TAB/LF/CR/NEL and supplementary controls, and retain protected-only invalid content through Base64. Require source-provenance, standard parser and complete integration checks.
- Estimated effort: 2–4 hours for a bounded serializer gate, atomic persistence regressions and documentation.

本项确认时尚未修复。原生诊断红测 0 passed / 1 failed / 670 filtered；
不能把筛选运行说成完整门。红测源码和哈希补丁保留在证据目录，工作区
测试文件恢复到 9798b77 原字节。此前完整门 323/670/ACL 2 仍是已提交
源码证据，不代表本项已覆盖。探针首次 String 借用比较编译失败是搭建
错误，修正后的实际生成／解析结果作为依据。

固定 KeePass 镜像 d1eec63f1bd73dc2d5273eaf94528f616b553ce5 的
StrUtil.cs:830–866 实现 SafeXmlString，按 XML 1.0 字符范围移除非法
UTF-16 单元；KdbxFile.Write.cs:912 用于普通 Value。源码及 SHA-256
保存于 kdbx-scalar-safe-string-reference-provenance.json。此为源码观察，
未运行竞品；静默删除会改变数据，因此选择保存拒绝并保留原内容。
Rust 字符串不含孤立 surrogate；未将所有 Unicode 非字符一律拒绝。

累计 77 Confirmed（Critical 1 / High 19 / Medium 46 / Low 11），
原 b7c5d9f 的 15 项和 6.1 分保持。手机 v1.6.5 替换 APK／真机
provider、有效昂贵 KDF 预算与其余系统／竞品／UI 仍未完成，目标 active。

9333209 已完成发布拒绝修复：在完整实际 XML 序列化之后、KDBX
目标发布之前按 XML 1.0 Char 范围检查文本，非法字符返回固定无内容
错误。不静默删除、不写非法字符引用、不更改输入读取或默认保护策略。
受保护值已经转为 Base64，因此其底层任意合法 Rust 字符串保持可存；
字段名、普通值、元数据、分组和历史中的实际 XML 文本均受校验。

三项原生回归通过：实际 Bitwarden command/wire/import 的 9 种非法
普通输入保存被拒绝，原文件字节保持且可重开、live dirty 编辑与保护
值保持；普通 current 修正后仍存在的非法历史阻止发布并保留两个版本；
6 种合法边界与 9 种 protected-only 非法底层值经过实际保存重开保真。
依赖新增 2 个测试覆盖 36 个非法 value/key/group/meta 位置以及合法／
受保护控制，全部 25 专项测试通过。独立探针保持相同源代码、当前
两个 vendor 路径和记录的 lock，9 个非法保存拒绝、6 个合法保存重开，
实际生成的 6 份 XML 均由 ElementTree 接受。路径、源／lock／XML
哈希记录于 kdbx-scalar-independent-green-provenance.json。

verify-kdbx-scalar.log exit 0：323 frontend、673 Rust、ACL 2，来源门、
format、零错误警告 check、150-key 契约、build、KeePass 25、QuickXML
1363（4 ignored）及 clippy 均通过。完整根门 serial/debug0/unfiltered，
live clipboard unset；4 个原生剪贴板测试体和 4 ignored 不计执行覆盖。
独立探针使用 debug0 dev/default scheduling，不混称根门。短暂 target
锁等待不计产品失败或性能证据；新增线性字符扫描尚无大库性能基准。

KeePass 仍 6 个 reviewed patch、54 upstream 不变，只有既有
xml_db/mod.rs 更新；其余五个 patch 文件保持。原始 archive 加新 patch
重建 60 个哈希、60／43 完整暂存字节及两个 patch identity 均通过，
QuickXML 来源／补丁未改。一次 rustfmt 跟随入口 module 格式化了五个
upstream 子文件，已恢复 exact HEAD 字节并在完整门前验证；维护文档
现在明确 --config skip_children=true，不把工具行为记为产品问题。

文档 gate 同步 data/security/structure、maintenance、patch 说明和
TODO；SKILL 路由与接口／schema／默认值未变。报告和 strict JSON
单独复核后提交，累计保持 77、原始 b7c5d9f/15/6.1 不变。
已有非法普通历史仍会阻止保存，需修正或移除；没有自动修复、完整
文本导入／导出、跨客户端、手机新 APK／provider 或完整安全保证。
整个审计／竞品／UI 目标 active。

### Finding: SP-S67 欢迎页拒绝后端支持的仅密钥文件凭据

- Severity: Medium
- Confidence: High
- Category: Stability / Frontend state
- Status: Confirmed
- Affected area: Welcome local/remote vault open and create forms
- Evidence:
  - File: src/lib/components/VaultWelcome.svelte:181/203/275/354 (32c23d9); src-tauri/src/vault/helpers.rs:35; src-tauri/src/vault/tests.rs:8184
  - Function / Module: confirmOpen / confirmCreate / confirmRemoteOpen / confirmRemoteCreate -> vault service -> build_database_key
  - Relevant behavior: 四个欢迎页入口在 keyfilePath 已选择时仍以空 password 直接返回；后端接受密码或密钥文件至少一项，LockScreen 的判断也允许仅密钥文件。
  - Reproduction: welcome-keyfile-red.log 的四个生产 handler 提交用例均未调用 vault service。18 项有界表单探针中 8 failed / 10 passed；另外四个失败是同一提前返回导致后端错误控制无法到达，不计第二项缺陷。VM 使用合成状态和服务替身，不读取文件或网络，不是 Android 运行证据。
  - Independent verification: welcome-keyfile-native-control.log 中既有原生 keyfile_only 两项通过，实际创建／改密后重开成功，缺少正确密钥组件失败；671 个 Rust 和 2 个 ACL 测试被筛选，不能称完整门。远程支持依据共享 prepare_remote_open/create 的 build_database_key 调用，不声称真实云端验证。
- Problem: 仅密钥文件的合法数据库无法从欢迎页打开或创建，前端错误要求输入本来不存在的主密码。
- Why it matters: 用户在安全设置把已有数据库改为仅密钥文件后，欢迎页无法再次打开；锁屏与欢迎页凭据语义不一致。没有发现认证绕过、数据丢失或秘密暴露。
- Realistic failure scenario: 用户成功以仅密钥文件改密，关闭数据库返回欢迎页，再选择原数据库和密钥文件，点击打开却收到“请输入主密码”，请求未到后端。
- Minimal fix: 四个入口仅在 password 和 keyfilePath 都为空时拒绝；提示说明可选择密钥文件。保留密码确认、后端验证、密钥文件场景不记忆密码的已有规则。
- Better long-term fix: 统一各凭据表单的支持条件并增加真实桌面与 Android 文档提供商覆盖；不要把浏览器 demo 或 VM 替身当作原生文件授权证据。
- Regression test suggestion: 运行四个实际生产 handler，覆盖仅密钥文件、无凭据、仅密码、后端失败和本地密码确认／数据库路径缺失；以原生合成 KDBX 创建、重开和错误凭据控制独立验证后端。
- Estimated effort: 1–2 hours for form guards, localized copy, regression and verification.

本项确认阶段尚未修改应用源码。证据位于本次任务目录的
welcome-keyfile-probe.mjs、welcome-keyfile-red.log 和
welcome-keyfile-native-control.log。累计 78 Confirmed（Critical 1 / High 19 /
Medium 47 / Low 11）；包含历史已修复项，不等于 78 项未修复。
原始 b7c5d9f 的 15 项与 6.1 分保持。

2026-11-10 用户要求排查内容拦截并继续有界防御审计。官方
[Models and Trusted Access](https://learn.chatgpt.com/docs/cyber-safety)
说明合法安全工作也可能触发防护；本次提示没有具体条款或请求日志，
不能确定触发原因。异常 KDF 高资源耗尽复现未执行，后续不恢复该计划。
工具记录目标为 blocked，不能照搬早先交接中的 active；本轮工作依据
用户明确继续授权执行，不据此宣称全局目标完成或状态已恢复。

密钥文件选择器候选亦已复核：Cargo.lock 锁定 tauri-plugin-dialog 2.7.3，
其 Android DialogPlugin.kt 使用 ACTION_GET_CONTENT 返回 URI，故“调用
桌面插件必然不出现手机选择器”的推断不成立，不计确认缺陷。与 SAF
的持久授权差异、迟到回调、新 APK／真机提供商行为继续保留待验。

73ec3c6 已修复 SP-S67：四个欢迎页入口只在密码与密钥文件均为空时
拒绝，中英文提示均说明两种凭据。密码确认、所选数据库路径检查、
后端认证失败与密钥文件请求不记忆密码的已有行为保留；未改变后端
KDF、IPC、密钥校验或界面样式。

原样 welcome-keyfile-probe.mjs 的 SHA-256 已比对确认未改动，同一
探针修复后 18/18 通过（welcome-keyfile-green.log）。仓库加入的
welcome-credentials.test.mjs 采用相同生产 handler 与合成服务边界，
使用仓库相对路径便于跨主机运行；不把该替身描述成真实文件选择或
远程网络运行。暂存六文件哈希与补丁见 welcome-keyfile-green-evidence.json
及 welcome-keyfile-fix.patch。

verify-welcome-keyfile.log exit 0：格式、零错误警告类型检查、150-key
设置契约、生产构建、341 frontend、673 Rust、ACL 2、KeePass 25、
QuickXML 1363（4 ignored）及 clippy 全部通过。Rust 根测试串行、
debug0、无筛选、live clipboard unset；四个原生剪贴板测试体与 ignored
不计执行覆盖。此前仅筛选原生两项的日志继续作为独立控制，不混称
完整门。提交前补充文档后的 format:check 亦通过。

文档 gate 同步 data-contracts 与 TODO 的精确前端完成项；既有
SKILL 路由、模块归属、认证／会话／主题合同保持，无额外引用更新。
报告和 strict JSON 另行复核，累计保持 78，原 15 项／6.1 基线保持。
用户随后已把全局目标恢复为 active；本轮有界工作完成后仍继续审计，
未宣称全部系统／竞品／UI 或手机新 APK／真机 provider 已完成。

### Finding: SP-S68 迟到的空闲锁到期回调被恢复活动延后

- Severity: Medium
- Confidence: High
- Category: Security / Frontend state
- Status: Confirmed
- Affected area: Renderer idle auto-lock deadline and pending closure
- Evidence:
  - File: src/lib/services/security.ts:86–168; src/lib/services/vault.ts:820–850; src/routes/+page.svelte:299–332 (8f6b31c)
  - Function / Module: armIdleLock / onActivity / installAutoLock -> lockVault -> topologyQueue closeAll
  - Relevant behavior: rearm cancels the previous timer and overwrites its deadline without checking expiry. Focus/visibility resume has no expiry listener. Until backend close resolves the published vault remains open, so subsequent activity can schedule another close.
  - Reproduction: idle-deadline-confirmed-red.log: 8 bounded tests, 5 failed / 3 passed. Advancing mocked wall time without delivering the due timeout, then pointerdown/wheel/focus/visibilitychange leaves close count 0. Releasing a gated first close after later activity reveals close count 2.
  - Independent verification: the probe imports the actual security/settings/vault service graph, mocks only IPC/localStorage/events/clock and exercises the real topology queue. Rechecking after gate release corrects the earlier weak pending assertion; original probe remains unchanged as historical evidence. Early activity, early resume and disabled mode controls pass. No Android/native suspension or shared clipboard execution.
- Problem: a configured idle deadline can be replaced by a fresh period after it has already expired; repeated activity while a close is pending can also enqueue a duplicate close.
- Why it matters: the resumed interface can remain unlocked beyond the user's selected idle duration. Medium reflects the timer-delivery ordering constraint and synthetic runtime scope; no credential disclosure, authentication exploit or device frequency is proved.
- Realistic failure scenario: a renderer resumes after its idle deadline while its timer callback is delayed; the first normal click cancels the due timer and grants another full idle interval.
- Minimal fix: check the existing deadline before rearming, check expiry on focus and visible resume without extending an unexpired deadline, and prevent duplicate idle closure while the same attempt is pending. Preserve zero-disabled and normal early activity semantics.
- Better long-term fix: validate native Android lifecycle/background locking and native backend timing separately; renderer resume checks cannot substitute for onPause/onStop or enforce deadlines while the renderer cannot run.
- Regression test suggestion: retain actual-service delayed-timer activity/resume controls and assert duplicate count after releasing closure; add hidden/early-resume, settings changes, cleanup and close/reopen lifetime controls.
- Estimated effort: 2–3 hours for a scoped deadline guard, regression and full verification.

本项确认阶段尚未修改应用源码。哈希、完整 checkpoint 与源码来源见
idle-deadline-evidence.json；5 个失败归入同一空闲锁生命周期卡片，
不计成五个独立缺陷。旧探针的 pending 用例未在释放 close 后断言，
其原先通过不能用作防重复证据；补强版释放后明确观察到两次关闭。

竞品固定 KeePass 镜像 d1eec63f1bd73dc2d5273eaf94528f616b553ce5：
MainForm.cs:1596–1654 的 timer 比较 UTC deadline，带重入 guard；
MainForm_Functions.cs:2863–2873 的 NotifyUserActivity 仍无条件刷新期限。
这里只借鉴显式期限与关闭防重思路，不声称竞品已解决相同恢复事件
交错；未运行竞品。两个原始源码的 SHA-256 已保存。

累计 79 Confirmed（Critical 1 / High 19 / Medium 48 / Low 11），包含
历史已修复项；原始 b7c5d9f / 15 项 / 6.1 分不变。此前拦截的准确
触发原因仍未知，高资源耗尽复现未执行且已放弃。原生 Android 后台锁、
Keystore、手机另存为替换 APK／真机 provider、完整竞品能力与 UI
优化仍待完成；整体目标 active。

193fe0a 已修复 SP-S68 的 renderer 空闲期限：活动／rearm 前检查旧期限，
focus 和 visible resume 只检查是否到期，未到期不延长；隐藏事件不替代
原生后台锁定策略。0 禁用、期限前离散／连续活动、设置变更规则保留。
独立的 opaque close attempt 在后端关闭等待期间防重复；已发布关闭后
重开进入新期限，旧请求迟到的剪贴板完成不能解除新一次关闭的防重。
手动／焦点／敏感复制的既有 lockVault 合同、后端和 IPC 未变。

原样 idle-deadline-confirmed-probe.mjs 的 SHA-256 比对未变，同一探针
修复前 5 failed / 3 passed，修复后 8 passed。仓库实际服务图新增 18
回归，加上菜单 9 项共 27 passed。新增用例覆盖七种活动／恢复事件、
期限前行为、隐藏恢复、设置、监听器／保存订阅清理、排队重开以及
合成剪贴板 IPC 的旧／新关闭交错，不使用共享 OS 剪贴板或用户库。
首次新增集合因设置 debounce 句柄跨 mock 时钟重置而失败；fixture
在重置前 flush 合成保存队列后完整通过，该搭建问题不计产品缺陷。

verify-idle-deadline.log exit 0：格式、零错误警告 check、150-key 设置
契约、生产构建、359 frontend、673 Rust、ACL 2、KeePass 25、
QuickXML 1363（4 ignored）及 clippy 全部通过。Rust 根门串行、
debug0、无过滤，live clipboard unset；四个原生剪贴板测试体与
ignored 不计执行覆盖。TODO 更新后的完整 format:check 也已通过。
QuickXML 临时专项副本的 unused patch 提示与此前一致，不等于应用
依赖失效；两套来源门均通过。补丁和源码／探针哈希见
idle-deadline-fix.patch 与 idle-deadline-green-evidence.json。

文档 currency gate 同步 security-model 的实际期限和 pending 生命周期，
TODO 只检查精确 renderer 范围。已有 SKILL 路由、模块归属、IPC／schema
和主题合同仍有效，不额外修改维护引用。报告／strict JSON／79 张卡片
计数与原始 15 项／6.1 基线另行复核。整体目标继续 active；本项不
代表原生 Android 后台锁定、Keystore、手机另存为新 APK／真机 provider、
renderer 停止运行时的后端期限约束或完整系统／竞品／UI 工作完成。

### Finding: SP-S69 损坏配置的解析诊断回显凭据字段输入

- Severity: Medium
- Confidence: High
- Category: Security / Privacy
- Status: Confirmed（已修复，2912963）
- Affected area: Native startup config recovery diagnostics
- Evidence:
  - File: src-tauri/src/config/store.rs:48–70; src-tauri/src/config/settings.rs:669–720; src-tauri/src/lib.rs:350 (c9d3f1b)
  - Function / Module: ConfigStore::load -> read_config -> serde_json::from_str<AppConfig> error logging
  - Relevant behavior: startup recovery prints the complete serde_json error with Display. Wrong-typed accessKey/secretKey values can appear in that error even though their decoding fails. The corrupt file is backed up and defaults load, so successful recovery does not prove diagnostic privacy.
  - Reproduction: config-diagnostic-red-pinned.json executes the real production config modules with root-matching serde 1.0.229 / serde_json 1.0.151. Two numeric credential-field cases appear in stderr; correctly typed string control does not. All three file/recovery controls pass. Captured evidence redacts the synthetic credential token.
  - Independent verification: config-diagnostic-native-red.log runs the actual root ConfigStore in isolated child processes. One parent regression fails with disclosure count 2 rather than 0 after all three child controls succeed, including exact backup, defaults and valid masked snapshot. 673 root tests are filtered; this is not a full gate. Diagnostic test source/patch hashes are saved and the original tests.rs bytes restored exactly.
- Problem: a malformed config can place a credential-field input value into process diagnostics through a raw deserialization error.
- Why it matters: a manually edited legacy/plaintext WebDAV credential with missing JSON quotes may be copied into a diagnostic capture or support log. Medium reflects the wrong-typed local-config condition and stderr scope; no real credential, normal DPAPI/keyring value, remote disclosure or Android logcat execution is proved.
- Realistic failure scenario: a numeric WebDAV password is manually entered without quotes in a legacy config; the app starts on defaults and preserves the corrupt file, but stderr includes the entire input number.
- Minimal fix: emit only the parser's non-content category, line and column plus existing recovery/backup status. Never format the error's Display/Debug text or source JSON into the diagnostic. Keep config recovery and schema behavior unchanged.
- Better long-term fix: review other secret-bearing parse boundaries and verify native mobile logging/storage separately. Keystore and background lock gaps are independent and remain unresolved.
- Regression test suggestion: capture actual root recovery stderr in child processes for wrong-typed accessKey/secretKey and a valid control; require absent input values, exact corrupt-file backup, defaults and unchanged valid/masked values. Corroborate with the immutable module probe.
- Estimated effort: 1–2 hours for a safe diagnostic formatter, native regression and complete verification.

确认阶段尚未修改应用行为，工作区测试文件已恢复。原生红测 0 passed /
1 failed / 673 filtered，三项子进程控制全部成功才到达回显计数断言。
独立探针首版 serde_json 1.0.154 是初步证据；最终依据已对齐 root lock
的 1.0.151 版本，源码／manifest／lock 哈希见 config-diagnostic-red-pinned.json。
不把新版本辅助结果混称生产版本，也未访问用户配置、密码库、剪贴板
或网络。原生和独立探针使用的凭据均为公开合成标记。

移动端复核另确认 remote_secret::store 仍经非 Windows DPAPI passthrough，
Android Keystore 和原生 onPause／onStop 锁定未接；这里只记录已有源码
缺口，不声称设备泄漏复现或在本项完成其实现。PITFALLS 中把已接系统
keyring 的桌面 macOS/Linux 仍写为未保护，需与现有 platform 源码同步。

累计 80 Confirmed（Critical 1 / High 19 / Medium 49 / Low 11），包含
已修复历史；原始 b7c5d9f／15 项／6.1 分不变。手机另存为新 APK／
真机 provider、移动端安全语义、其余系统／竞品／UI 仍待闭环，目标 active。

修复与验证（2026-11-11）：2912963 只修改配置解析诊断，保留错误类别／
行列及原来的备份／默认启动反馈，不再插入完整 serde 错误。实际原生
子进程回归 1 passed / 0 failed / 673 filtered，三个子测试均实际执行；
坏文件逐字节备份、默认恢复和合法／masked 控制保持。原样锁定依赖
模块探针 3 passed / 0 failed，源码／manifest／lock 哈希与红测一致。

verify-config-diagnostic.log 最终 exit 0：359 前端、674 Rust、ACL 2、
KeePass 专项 25、QuickXML 1363（4 ignored）；format、Svelte check／
build、IPC contract 和 clippy 均通过。Rust 单线程、debug0、无过滤，
live clipboard 未启用；四项原生剪贴板测试体及 ignored 项不算实际覆盖。
config-diagnostic-final-proof.json 保存源码、修复 patch 和完整日志哈希。

文档 currency gate 已同步 data-contracts／security-model／PITFALLS 和
精确 TODO，且纠正桌面 macOS/Linux 已接 OS keyring 的旧说明。未改变
schema、IPC、CSS、依赖或 vendor。无关 .trae/ 保留。此项只关闭本地
配置错误日志回显；手机另存为新 APK／真机 provider、移动端 Keystore
及原生后台锁、其余系统／竞品／UI 仍需继续验证，整体目标 active。

### Finding: SP-S70 WebDAV 列表键名再次编码后指向不同文件

- Severity: Medium
- Confidence: High
- Category: Stability / Data integrity
- Status: Confirmed（已修复，a314835）
- Affected area: WebDAV list-to-open and shared object URL construction
- Evidence:
  - File: src-tauri/src/remote/webdav.rs:73–88, 161–169, 254–340, 355–366; src/lib/services/vault.ts:873–898; src-tauri/src/commands/remote.rs:28–80 (b6565fb)
  - Function / Module: parse_multistatus -> href_to_key -> RemoteObject.key -> open_remote_vault -> WebDavStorage::url_for
  - Relevant behavior: href_to_key retains percent-encoded href path text as the logical key; url_for then encodes each percent sign again. The selected key travels unchanged through the service and native open path. GET and PUT share this URL builder.
  - Reproduction: webdav-path-native-red.log executes actual production parsing and URL construction for four tiny synthetic paths. Plain ASCII retains its target; encoded space, UTF-8 Chinese and literal percent cases differ. A separate native GET captures `GET /dav/vaults/a%2520b.kdbx HTTP/1.1` for a listed `/dav/vaults/a%20b.kdbx`. Both regressions fail, with 674 unrelated tests filtered; no live provider or KDBX load was exercised.
  - Independent verification: Python stdlib ElementTree and urllib decode/re-encode all four XML href references to their expected original resource paths. webdav-path-independent-reference.json records four controls. Diagnostic source/patch and source hashes are saved; the production tests.rs is restored byte-for-byte before this report.
- Problem: WebDAV list results use URI-encoded strings where the transport expects a logical decoded key, so a selected filename can resolve to a different resource.
- Why it matters: normal spaces, Chinese filenames and percent signs can produce a download error despite the file being visible. If a differently named encoded-literal file exists, the request can select that sibling instead. Medium reflects normal filename usability and target identity; no real vault loss, overwrite or cloud-provider execution is proved.
- Realistic failure scenario: a provider lists `a b.kdbx` as `a%20b.kdbx`; selecting it sends `a%2520b.kdbx`, which refers to a file literally named `a%20b.kdbx`. A typical provider returns 404 because that sibling does not exist.
- Minimal fix: establish a single URI-to-logical-key boundary with exactly one segment-wise UTF-8 percent decode, then use the existing one-time request encoding. Validate malformed escapes, separator and dot-segment ambiguity explicitly rather than globally unquoting into structural separators. Preserve ordinary direct create/open keys and S3 semantics.
- Better long-term fix: cover absolute/path-relative hrefs, endpoint path boundaries, query/fragment handling and native provider behavior with an explicit WebDAV resource identity contract.
- Regression test suggestion: assert actual list-to-GET targets for ASCII, spaces, Unicode, literal percent and already-percent-looking logical filenames; add controlled write/readback and rejection controls for malformed encoding, encoded separators and traversal before publishing any fix.
- Estimated effort: 2–4 hours for the scoped path contract, bounded parser/wire regressions and full verification; real mobile/cloud provider verification remains separate.

本项确认阶段未修改应用行为；临时原生回归 0 passed / 2 failed /
674 filtered，普通路径控制通过，独立标准库参考 4 passed。只用合成
XML／路径和一次无凭据的 loopback GET；未读取用户库／配置、未访问
云端／共享剪贴板、未写远程文件或复现高资源输入。之前配置诊断修复
2912963 的完整门仍有效，但不作为本项修复验证。当时尚未修复，后续修复见下。

累计 81 Confirmed（Critical 1 / High 19 / Medium 50 / Low 11），含
历史已修复项；原始 b7c5d9f／15 项／6.1 分不变。整体目标 active，
手机另存为新 APK／真机 provider、移动端安全、其余系统／竞品／UI
仍未全部闭环。

修复与验证（2026-11-11）：a314835 在 href 与逻辑键名之间逐段解码
一次，再由既有 request encoder 编码。编码端点根路径与字面 plus／
percent-looking 文件名保持；错误百分号／UTF-8、编码斜杠／点／控制
字符、内部空路径段及绝对路径越出根目录均返回无 href 内容的错误。
raw path 校验先于 URL 的点路径归一化。请求仍使用配置的 endpoint，
不采用响应中的 authority；直接创建键和 S3 语义保持。

原样确认诊断源码 SHA256 与红测相同：两项从失败变为通过，676
无关测试 filtered。最终远程模块 28 项 harness 通过，其中 S3 live
test 因未启用而跳过，不能计真实服务验证。新原生 HTTP 回归使用
七类文件名／编码根路径、22 次实际 PROPFIND／GET／PUT／回读，
精确检查请求路径、回读字节和名称相似旁文件未修改。13 类非法
路径在已有有效记录之后拒绝整份列表，错误不回显 href。

verify-webdav-path.log 最终 exit 0：359 前端、676 Rust、ACL 2、
KeePass 25、QuickXML 1363（4 ignored）；format／check／build／
contract／clippy 均通过。单线程、debug0、无过滤，live clipboard／
S3 均未启用；四项原生剪贴板体、S3 live 与 ignored 不算实际覆盖。
webdav-path-final-proof.json 保存源码、原样诊断、fix patch、完整日志
及独立参考哈希；临时确认测试源码已逐字节恢复为正式回归版本。

官方 [RFC 3986 §2.4](https://www.rfc-editor.org/rfc/rfc3986#section-2.4)
明确禁止同一字符串重复编码／解码，[RFC 4918 §8.3](https://www.rfc-editor.org/rfc/rfc4918#section-8.3)
定义 Multi-Status href 的 URI 处理。公开文本和哈希保存于
webdav-uri-official-reference.json；这是规范旁证，不是竞品／提供商
运行证据。辅助脚本两次摘录定位失败已修正，不计产品缺陷。

文档 currency gate 同步 data-contracts／security-model／PITFALLS 和
精确 TODO；未改 IPC／schema／依赖／vendor／CSS。Legacy base-relative
键仍按旧约定处理，不声称实现全部 RFC reference resolution／XML
属性／提供商语义。手机另存为新 APK／真机 provider、移动端安全
与其余系统／竞品／UI 仍未全部验证，整体目标 active，累计 81
Confirmed 含历史已修复项；原始 b7c5d9f／15 项／6.1 分保持。

### Finding: SP-S71 WebDAV XML 实体和分段文本截断文件名与元数据

- Severity: Medium
- Confidence: High
- Category: Stability / Data integrity
- Status: Confirmed（已修复，82aea43）
- Affected area: WebDAV PROPFIND multistatus property text decoding
- Evidence:
  - File: src-tauri/src/remote/webdav.rs:229–245, 254–339; src-tauri/src/remote/s3.rs:385–404 (5a0edbc)
  - Function / Module: parse_multistatus -> record_prop -> RemoteObject.key/size/modified
  - Relevant behavior: each Text/CDATA event replaces the previous property value, with per-event trimming; GeneralRef is ignored. A valid XML property can contain multiple text chunks and references, so the final chunk is not the whole value. The S3 value reader already accumulates text/CDATA/reference events; this finding is specific to WebDAV.
  - Reproduction: actual native root diagnostic webdav-xml-native-red.log reports six filename mismatches after a plain control passes: named/decimal references, split CDATA, comment-separated text, referenced percent and literal entity text. Split size12 becomes2 and the modified date becomes only00:00:00 GMT. A separate real no-auth loopback GET for listed a&amp;b.kdbx targets /dav/b.kdbx rather than /dav/vaults/a&b.kdbx. Three tests fail,676 unrelated tests are filtered.
  - Independent verification: Python stdlib ElementTree joins text and resolves standard XML references for the same eight tiny documents; all expected key/size/date controls pass. Fixture/source/test hashes and exact root source restoration are recorded in webdav-xml-red-provenance.json. Official RFC4918 §8.3.1 explicitly notes legal URI ampersands still require XML escaping; its downloaded text/hash/excerpt is recorded separately.
- Problem: a successful WebDAV list can publish a truncated logical filename or incorrect metadata from valid XML.
- Why it matters: normal XML serialization of a filename containing & can make a visible item open a different resource or fail. Metadata hints also become incorrect. Medium reflects the real parser/request identity failure; no genuine cloud-provider, vault overwrite, data loss or credential disclosure is proved.
- Realistic failure scenario: a provider returns /dav/vaults/a&amp;b.kdbx in its href; the client displays b.kdbx and requests /dav/b.kdbx. XML writers that divide values around CDATA or comments produce the same suffix-only behavior.
- Minimal fix: collect one complete simple property value through Text/CDATA and standard named/numeric references, then trim once where appropriate. Preserve literal entity text without recursive unescape. Reject unknown references, invalid decoding or nested/truncated property content with content-free errors rather than returning guessed/partial list keys; retain optional size and collection behavior.
- Better long-term fix: separately audit complete XML structure, namespace/propstat semantics and real-provider interoperability. This patch should not silently imply a complete WebDAV parser rewrite or external entity support.
- Regression test suggestion: actual production parser and HTTP list-to-GET/PUT/readback for XML-escaped ampersand, character references, adjacent text/CDATA/comments, literal entity text and split metadata; independent standard XML controls; invalid entity/nested/truncated properties reject the entire list; previous URI encoding/root and collection tests remain green.
- Estimated effort: 1–2 hours for the scoped value reader, native wire regressions and complete verification; provider/device evidence remains separate.

确认阶段未修改应用行为。临时 root 测试已逐字节恢复；原生红测
0 passed /3 failed /676 filtered，标准库 XML 控制8 passed。仅少量
合成 XML／路径和一次无凭据 loopback GET，无用户配置／密码库、
云端／写远程文件、移动设备／APK、共享剪贴板、外部实体解析或
高资源复现。上一轮路径解码修复 a314835 不修复本项 XML 事件组合。

累计82 Confirmed（Critical1 /High19 /Medium51 /Low11），包含
历史已修复项；原始 b7c5d9f／15项／6.1分保持。手机另存为新 APK
和真机 provider、移动端安全、其余系统／竞品／UI 仍待完成，目标 active。

修复与验证（2026-11-11）：82aea43 将已识别的简单 XML 字段读至
完整结束，再解释 href／size／modified。Text／CDATA／标准 named 和
numeric reference 累加，引用仅解码一次；内部空白与字面实体文本
保留，注释／PI 不截断。未知引用、解码失败、嵌套／截断字段以无
内容错误拒绝整份列表。缺失／空的大小日期提示及原 collection
处理保持，URI 解码仍独立执行。

原样三个确认探针从红变绿（678 无关测试 filtered），fixture 和
诊断源码 SHA256 未变，临时追加源码精确恢复。严格 JSON 的八项
独立标准 XML／key／size／date 控制通过。远程模块30项 harness
通过；13类文件名的实际40次 loopback PROPFIND／GET／PUT／回读
精确核对目标、字节、元数据及名称相似旁文件未变。三种字段共15
类未知引用／嵌套／截断控制均拒绝整份列表。

首次完整门 verify-webdav-xml.log 在最终 Clippy collapsible_if 规则
处 exit1；合并条件后重播原样探针并完整重跑。最终
verify-webdav-xml-final.log exit0：359前端／678 Rust／ACL2／
KeePass25／QuickXML1363（4 ignored）；format／check／contract／
build／clippy 全通过。Rust单线程、debug0、无过滤，live clipboard／
S3／配置诊断fixture开关未启用。四个原生剪贴板体、S3 live体和
ignored 不计实际运行覆盖。webdav-xml-final-proof.json 保存源码／
fixture／原样诊断／fix patch／完整日志和参考哈希。

文档 currency gate 同步 data-contracts／security-model／PITFALLS 和
精确 TODO，未改 IPC／schema／依赖／vendor／CSS。RFC4918 XML转义
规范只是旁证；完整XML结构／namespace／propstat与真实提供商／
客户端兼容尚待独立审查。手机另存为新APK／真机provider、移动端
安全、其余系统／竞品／UI 仍待完成；累计82 Confirmed 含历史已
修复项，原始 b7c5d9f／15项／6.1分保持，整体目标 active。

### Finding: SP-S72 WebDAV 非列表页面与不完整文档被当成成功列表

- Severity: Medium
- Confidence: High
- Category: Stability / Testing
- Status: Confirmed（已修复，520a69c）
- Affected area: WebDAV list document envelope validation
- Evidence:
  - File: src-tauri/src/remote/webdav.rs:134–166, 254–338; src-tauri/src/remote/mod.rs:180–204 (28fa586)
  - Function / Module: WebDavStorage::list -> parse_multistatus -> list_objects_async
  - Relevant behavior: a case-insensitive substring anywhere in the body qualifies as multistatus; EOF ends parsing without requiring a single completed multistatus root. The transport and async command return this result as a successful list.
  - Reproduction: actual production parser accepts three HTML pages whose text/comment/attribute contains multistatus as successful empty lists; an unclosed root returns one object, two roots return two objects and trailing non-whitespace text returns one object. Valid empty and one-file controls pass. webdav-structure-native-red.log:1 passed/2 failed/678 filtered.
  - Independent verification: Python stdlib ElementTree full-document parsing plus DAV root identity rejects the six invalid cases and accepts the two controls. Eight tiny fixtures, diagnostic/source hashes and exact root test restoration are recorded in webdav-structure-red-provenance.json; no network or real provider is involved.
- Problem: provider errors and malformed document envelopes are converted to successful empty or partial object lists.
- Why it matters: a misconfigured gateway can appear to contain no files, while malformed provider output hides missing results instead of presenting a useful failure. Medium reflects missing error handling; no vault write, credential exposure, permanent data loss or real cloud incident is demonstrated.
- Realistic failure scenario: an HTTP-success gateway error page mentions multistatus, or a provider's XML document ends after a complete response but before closing its root. SecPivot shows an empty or partial list with no parsing error.
- Minimal fix: require exactly one completed multistatus document root before publishing any list; reject extra roots, truncated document envelopes and non-whitespace outside the root with content-free errors. Preserve existing valid empty lists, case-compatible tag handling and simple-property error contracts.
- Better long-term fix: separately audit namespace identity, response/propstat status semantics, XML scalar validity and real-provider interoperability; document-envelope validation alone does not establish all XML or WebDAV semantics.
- Regression test suggestion: replay the unchanged six wrong-root/boundary fixtures and two valid controls against the production parser; native loopback list responses must reject the same invalid documents; legal comments/PI/whitespace and existing uppercase/property/URI controls stay green.
- Estimated effort: 1–2 hours for a scoped document envelope gate, regressions and full verification.

确认阶段未改应用行为，临时测试逐字节恢复；两项红测与合法控制
区分失败边界。仅8个小型合成XML、实际解析函数，无网络／用户配置／
密码库／共享剪贴板／远程写入／外部实体／高资源复现。完整namespace／
propstat／真实provider仍待审，手机另存为仍缺新APK／真机验证。
累计83 Confirmed（Critical1／High19／Medium52／Low11）含历史修复；
原始 b7c5d9f／15项／6.1分保持，整体目标 active。

修复与验证（2026-11-11）：520a69c 在发布列表前以独立流式深度
计数验证单个完整 multistatus 根。错误根、根未闭合、第二个根及
根外非XML空白拒绝列表；合法空根、前后空白／注释／PI 保持。
完整属性读取仍先执行，原有字段错误契约不被提前的文档错误替代；
命名空间、response／propstat语义和完整XML scalar／语法不在此门内。

原样诊断源码与八项确认fixture的SHA256未变：3项全部通过，680
无关测试filtered；独立标准XML8项及严格JSON值一致性通过，临时
测试精确恢复。原生loopback实际8次无凭据HTTP200列表响应覆盖同
一组无效6项与合法2项，六项无效响应全部报错；另有合法前后PI／
注释／空白及7项非法边界控制。远程32项harness通过，其中S3 live
体未启用，不计提供商验证；此前属性／URI请求回归继续通过。

初次编译在 BytesText.as_ref 类型推断处E0282终止，修为直接slice
迭代后，targeted／clippy／原样重播／完整门通过，不计新增产品缺陷。
verify-webdav-structure-final.log exit0：359前端／680 Rust／ACL2／
KeePass25／QuickXML1363（4 ignored），format／check／contract／
build／clippy全通过。单线程、debug0、无过滤，live clipboard／S3／
配置fixture开关未启用；四个原生剪贴板体、S3 live及ignored不计
实际运行覆盖。webdav-structure-final-proof.json保存源码、原样诊断、
fixture、fix patch和完整日志哈希。

currency gate同步data-contracts／security-model／PITFALLS与精确TODO；
无IPC／schema／依赖／vendor／CSS改动。无用户密码库、真实provider、
设备／APK、共享剪贴板、远程文件写入或高资源复现。累计83
Confirmed含历史已修复项，原始b7c5d9f／15项／6.1分保持；手机
另存为新APK／真机、完整审计／竞品／UI仍待完成，目标active。

### Finding: SP-S73 WebDAV 请求未询问集合类型，按需属性响应把目录列成文件

- Severity: Low
- Confidence: High
- Category: Stability / Testing
- Status: Confirmed（已修复，fee4565）
- Affected area: WebDAV PROPFIND request and collection classification contract
- Evidence:
  - File: src-tauri/src/remote/webdav.rs:17–25, 124–164, 299–383; src-tauri/src/remote/tests.rs:624–646, 764–814 (abe8106)
  - Function / Module: PROPFIND_BODY -> WebDavStorage::list -> parse_multistatus
  - Relevant behavior: the named-property request includes size and last-modified only, while the parser needs resourcetype/collection to reliably exclude directories. Existing mocks return this unrequested property, hiding the request/response mismatch; file names no longer require a kdbx extension and optional size cannot identify resource type.
  - Reproduction: a bounded no-auth loopback server parses the actual request and returns resourcetype only when requested. The request contains only getcontentlength/getlastmodified; a collection href without its optional trailing slash is published as vaults/archive alongside a.kdbx and the extensionless backup file. webdav-collection-native-red.log reports1 passed/1 failed/680 filtered; explicit-type parser control already succeeds.
  - Independent verification: Python stdlib XML parsing of the captured actual request confirms exactly two DAV property names; the response contains four member records and no type property. Request/response/diagnostic/root-source SHA256 and exact source restoration are recorded in webdav-collection-red-provenance.json. RFC4918 §5.2 recommends resourcetype as more reliable than the URI and only SHOULD requires a collection trailing slash; §9.1 defines named-property selection.
- Problem: the transport never requests the property on which its collection filter depends.
- Why it matters: a named-property server can show a directory as a selectable database file, followed by an open failure. Low reflects a constrained enumeration/usability defect; no actual provider incident, file overwrite, credential disclosure or data loss is demonstrated.
- Realistic failure scenario: a cloud gateway returns only the two requested metadata properties and represents a directory without a trailing slash. SecPivot shows that directory in its file list, although the parser would correctly skip it if its type were requested and returned.
- Minimal fix: request DAV:resourcetype alongside size/date, then retain the existing optional metadata and filename behavior. Add a wire fixture whose response follows the actual named-property request instead of always returning extra properties.
- Better long-term fix: separately audit namespace and successful propstat/type semantics, unavailable properties and real-provider compatibility; never guess that extensionless or size-zero objects are directories.
- Regression test suggestion: capture the real PROPFIND property names, verify a request-driven response excludes non-slashed collections and retains kdbx/extensionless files and metadata; missing type/size/date compatibility and previous URI/XML/empty-list controls remain green.
- Estimated effort: 30–60 minutes for the request property, bounded wire regression and full verification.

竞品复核：KeeWeb固定公开commit ad7891de92cf666400a6b4c859eeb731b0f68761、
package版本1.18.9的StorageWebDav提供直接URL GET／HEAD与PUT／临时
文件MOVE，未实现PROPFIND列表，不能据此验证集合识别。其默认临时
写后MOVE需要额外提供商方法与事务语义，不直接搬入本项修复；其
fileOptsToStoreOpts的XOR／Base64也不替代SecPivot原生凭据存储。
上述为源码比较，无竞品运行／安全事件判断；固定源码和package
哈希保存于keeweb-webdav-pinned-reference.json。首次旧路径404已通过
官方仓库树定位impl路径，不计产品缺陷。

确认阶段未修改应用行为，临时测试逐字节恢复。仅一次小型无凭据
loopback请求和合成成员，无用户配置／密码库、真实云端、真机／APK、
共享剪贴板、远程文件写入或高资源复现。累计84 Confirmed含历史
已修复项（Critical1／High19／Medium52／Low12），原始b7c5d9f／15项／
6.1分保持；其余审计／竞品／UI与手机另存为真机验证仍待完成。

修复与验证（2026-11-11）：fee4565 在命名属性PROPFIND请求中加入
DAV:resourcetype，原有集合过滤无需重写；大小／日期仍为提示，
类型不可用时不猜测普通／无扩展名文件。原样2项诊断全部通过，
682无关测试filtered，诊断与红测request／response原件SHA256未变；
临时源码精确恢复。独立标准XML对实际新请求确认三个DAV属性，
对实际新响应确认四个成员中的两个普通文件。

正式原生HTTP回归校验实际请求的DAV namespace、精确三项属性、
Depth1、无凭据和请求驱动响应：非斜杠目录排除，kdbx／无扩展名
文件及大小日期保留。空／缺失／404类型提示对照保留普通文件。
早期targeted34／clippy通过；提示fixture改为合法propstat结构后，
原样诊断及最终完整Rust门内34项remote harness再次通过。

verify-webdav-collection-final.log exit0：359前端／682 Rust／ACL2／
KeePass25／QuickXML1363（4 ignored）；format／check／contract／
build／clippy全通过。单线程、debug0、无过滤，live clipboard／S3／
配置fixture开关未启用；四个原生剪贴板体、S3 live和ignored不计
实际运行覆盖。webdav-collection-final-proof.json保存源码、原样诊断、
fix patch、完整日志、实际XML和固定竞品参考哈希。

currency gate同步data-contracts／security-model／PITFALLS和精确TODO；
无IPC／schema／依赖／vendor／CSS变化。真实provider、namespace／
propstat仍待独立复核；手机另存为仍待替换APK／真机验证。累计84
Confirmed含历史已修复项，原始b7c5d9f／15项／6.1分保持；完整
审计／竞品／UI目标仍active。

当前库存复核（fee4565）：已提交第一方文本文件368个、
119066行；相较初始路径集合74个既有路径哈希变化、116个
新覆盖路径，后者包含初始库存未纳入的文档／技能等，不等同新增
应用源文件。vendor的109个跟踪文件在该库存之外，补丁字节与来源
另由完整门验证；二进制／锁文件和无关未跟踪编辑器数据排除。
first-party-inventory-webdav-collection.json保存路径／哈希／排除／工作树
状态，仅证明库存覆盖，不把368个文件或119066行称为逐行深读；
原始252个文件／82147行的基线库存记录保持。

### Finding: SP-S74 WebDAV 忽略命名空间与层级，扩展字段改变真实路径和集合判断

- Severity: Medium
- Confidence: High
- Category: Stability / Data integrity
- Status: Confirmed（已修复，7532f1f）
- Affected area: WebDAV XML expanded-name and parent-container interpretation
- Evidence:
  - File: src-tauri/src/remote/webdav.rs:252–402 (fb6601b)
  - Function / Module: parse_multistatus / validate_multistatus_document / local_name
  - Relevant behavior: local names are lowercased without resolving namespaces, and any href/response/collection inside a response updates the current record regardless of its parent. Unknown extension elements and their children are not skipped. Root namespace and unbound prefixes are unchecked.
  - Reproduction: webdav-namespace-native-red.log runs actual production parsing for16 tiny documents. Five prefix/default namespace/uppercase/fully unqualified legacy controls pass; eleven foreign/same-name/rebound-prefix/unqualified-in-DAV/unknown-container cases mismatch expected resource or rejection. Two tests fail/one passes,682 unrelated tests filtered. A real no-auth loopback GET after parsing a foreign href targets /dav/other.kdbx instead of /dav/actual.kdbx; no actual cloud vault is loaded or written.
  - Independent verification: stdlib ElementTree expanded names and direct-child path lookup reproduce all16 expected outcomes. The two non-DAV/unbound-root documents reject, while unrelated extension subtrees leave the actual file, size and date intact. Compatibility rules for uppercase/entirely unqualified legacy XML are explicitly separated from namespace identity. Fixture/diagnostic/source hashes and exact test restoration are recorded in webdav-namespace-red-provenance.json. RFC4918 §17 requires processing as if unrecognized elements and their children were absent.
- Problem: valid extension data can overwrite the response href/metadata, create an unrelated response or misclassify a regular file as a collection.
- Why it matters: a server returning ordinary dead/extension properties can make a listed file request another resource on the configured endpoint or disappear. Medium reflects verified resource identity and enumeration failure; no real provider incident, remote write, credential disclosure or data loss is demonstrated.
- Realistic failure scenario: a PROPFIND response includes its real DAV:href and a separate application namespace's href property. The latter replaces the real path; an unrelated collection or nested response can also alter the visible list.
- Minimal fix: resolve expanded names, recognize fields only under their expected DAV containers, and ignore unknown/unexpected subtrees including recognized-looking descendants. Require a DAV root, with an explicitly separate entirely unqualified legacy mode; reject unbound prefixes without echoing XML. Preserve prior URI, simple text, empty-list and uppercase compatibility contracts.
- Better long-term fix: separately audit successful propstat/status policies, duplicate known properties, full XML validity and actual provider/device interoperability; namespace/context correctness does not establish those boundaries.
- Regression test suggestion: replay immutable16-case expanded-name/context fixture and original GET probe; bounded list-to-GET/PUT/readback must preserve the real file and untouched sibling despite extension href/type/metadata/response fields. Prefix aliases/default/rebinding/uppercase/legacy and previous XML/URI controls remain green; old malformed-property tests must target actual namespaced property locations.
- Estimated effort: 2–4 hours for scoped namespace/context interpretation, wire regressions, original-probe replay and full verification.

确认阶段未修改应用行为，临时测试逐字节恢复；标准库16控制通过，
原生两项红测及合法控制通过。仅小型合成XML和一次无凭据loopback
GET，无用户配置／密码库、云端／写入、共享剪贴板、真机／APK、
外部实体或高资源复现。累计85 Confirmed（Critical1／High19／Medium53／
Low12）含历史已修复项，原始b7c5d9f／15项／6.1分保持；整个审计／
竞品／UI和手机另存为真机验证仍待完成，目标active。

修复与验证（2026-11-11）：7532f1f 使用NsReader按DAV命名空间和
预期父容器读取字段，忽略完整未知／错误位置子树；prefix别名／default
namespace／uppercase与全无namespace旧格式保持，非DAV根／未绑定
prefix以无内容错误拒绝。原样namespace三项探针与先前SP-S71文本
三项探针分别3 passed／684 filtered；诊断／十六项fixture原件哈希未变，
共享strict JSON值一致，临时测试逐字节恢复。

正式十六项namespace／层级fixture及未知子树复杂后代回归通过；实际
无凭据loopback列表／GET／PUT／回读四次请求均保持actual.kdbx与大小／
日期，other.kdbx原字节未变。原十五类错误字段fixture仅修正DAV资格
及propstat／prop位置，错误值和内容无关断言保持；原样SP-S71另行重播。

verify-webdav-namespace-final.log exit0：359前端／684 Rust／ACL2／
KeePass25／QuickXML1363（4 ignored），其中remote harness36；
format／check／build／补丁来源／clippy全通过。完整门后仅澄清相关文档
并再次format:check。单线程、debug0、无过滤且live clipboard／S3／
配置fixture未启用；跳过的原生剪贴板／S3与ignored体不计运行覆盖。
webdav-namespace-final-proof.json保存原样诊断、最终源码、fix patch
和完整日志哈希；保留fee4565历史库存及其artifact，不将库存称逐行深读。

currency gate同步data-contracts／security-model／PITFALLS及精确TODO；
无IPC／schema／依赖／vendor／CSS变化。propstat状态、重复属性、
完整XML／真实provider仍待独立审计；手机另存为仍待新APK／真机选择器
验证。累计85 Confirmed含历史已修复项（Critical1／High19／Medium53／
Low12），原始b7c5d9f／15项／6.1分保持；整体审计／竞品／UI目标active。

追加真实性复核（2026-11-11，94985ce）：propstat状态候选暂不确认。
RFC4918 §9.1规定同一资源的每个属性只出现一次；用重复成功／失败属性
或失败块中的非空伪值作主要证据，不能证明合规provider的正常路径失败。
本次六个合规控制让成功属性与不可用属性集合互不重叠：200元数据／
404空类型、反序、成功collection／403空元数据、全不可用、status先于
prop及foreign status扩展。独立ElementTree按200块读取均得到预期值；
实际生产解析器1 test passed／684 filtered覆盖六例，字段／集合结果
全部一致，临时源码逐字节恢复，不新增issue card或修复。

首轮harness在第四例比较时把缺省modified错误写成null，而DTO使用
skip_serializing_if省略该字段；最后两例当时未执行。已纠正fixture／
oracle格式并完整重跑六例，保留webdav-status-initial-harness-error.log，
不把这一测试错误归为产品缺陷。webdav-status-candidate-proof.json
保存fixture／diagnostic／原生日志／源码／RFC哈希及此差异说明。
本轮无网络请求、真实provider／配置／密码库／剪贴板／真机／APK或
高资源复现；非空失败值、重复属性、response级／缺失／畸形status与
完整provider语义仍待独立判断，六例通过不代表全面状态验证。
累计85历史Confirmed、原始15项／6.1分不变，整体目标仍active。

### Finding: SP-S75 远程密码被自动去掉两端空白，正确凭据在配置保存时改写

- Severity: Medium
- Confidence: High
- Category: Stability / Data integrity
- Status: Confirmed（已修复，ff9efb9）
- Affected area: Remote secret normalization and settings persistence
- Evidence:
  - File: src/lib/services/settings.ts:377–418, 1221–1229; src-tauri/src/config/settings.rs:1352–1359; src-tauri/src/config/store.rs:57–86, 210–243; src-tauri/src/remote/s3.rs:58, 93–102 (858f947)
  - Function / Module: normalizeRemoteSettings -> schedulePersist -> ConfigStore::set/read_config -> normalize_remote_settings
  - Relevant behavior: the frontend string helper trims secretKey before queued set_config; native normalization independently trims secret_key after decoding and after encrypted storage reload. WebDAV forwards the resulting changed password unchanged, while S3 construction additionally trims its signing key.
  - Reproduction: fourteen actual frontend normalization cases across both kinds change ten exact values; seven actual appSettings.updateRemote/flush cases send five changed values to the synthetic set_config seam. Native production normalization changes ten of fourteen values; ConfigStore::set and actual Windows DPAPI TempDir reopen each change five supplied boundary-whitespace values. Two native tests fail with counts only,684 unrelated tests filtered; no credential values logged. Leading/trailing/both/space-only/Unicode edge whitespace fail; plain/empty and endpoint trim controls retain their intended behavior.
  - Independent verification: RFC7617 section2 specifies concatenating user-id, colon and password, excluding control characters without whitespace trimming. Pinned KeeWeb commit ad7891de92cf666400a6b4c859eeb731b0f68761 uses config.password directly in Basic construction at storage-webdav.js:367–370; this is source evidence only. Diagnostic/log/source/RFC hashes and exact restoration are in remote-password-whitespace-red-proof.json and remote-password-whitespace-independent-reference.json.
- Problem: password content is treated as surrounding form whitespace rather than an exact credential. A whitespace-only replacement can also become the empty placeholder that retains an older stored password.
- Why it matters: a valid WebDAV credential cannot be submitted or stored faithfully; repeated authentication failures look like server or ACL problems. Medium reflects verified configuration fidelity and expected login failure, with no actual provider incident, credential disclosure, vault loss or privilege bypass demonstrated.
- Realistic failure scenario: a WebDAV account has a password beginning or ending in spaces. The user enters the correct password; the queued settings payload and stored configuration silently contain a different value, so subsequent Basic authentication cannot match the account.
- Minimal fix: preserve secretKey/secret_key strings exactly through normalization and transport construction. Keep endpoint/region/bucket/prefix/name normalization and exact-empty placeholder semantics. Add actual queued IPC, encrypted reload and bounded synthetic Basic request regressions.
- Better long-term fix: treat secret fields as opaque values across all configuration boundaries; distinguish exact empty from whitespace without silently sanitizing credentials. Mobile secret-store and real provider/device compatibility remain independent work.
- Regression test suggestion: replay immutable frontend/native probes; verify masked responses, exact-byte encrypted reload and blank-keep after whitespace-only replacement, and a bounded loopback Basic header matching the original synthetic password. S3 signing-key preservation and empty-key rejection need scoped coverage without live cloud credentials.
- Estimated effort: 1–2 hours for the three normalization/construction changes, independent replay, native wire and full verification.

确认阶段未修改应用行为，临时测试逐字节恢复；前端为真实模块与合成
masked IPC边界，原生配置仅临时目录及Windows DPAPI。没有真实用户
配置／密码库／云端认证／剪贴板／设备／APK或高资源工作。竞品仅取
密码原样传递契约，不采用其XOR持久化；不以源码比较证明竞品运行。
累计86 Confirmed含历史已修复项（Critical1／High19／Medium54／Low12），
原始b7c5d9f／15项／6.1分保持；完整审计／竞品／UI目标仍active。

修复与验证（2026-11-11）：ff9efb9 在前端／原生配置归一化及S3
构造中原样保留secretKey／secret_key；仅exact empty保持旧凭据，
纯空白作为新非空值保存。其他字段的归一化和masked IPC保持。
原样前端14项归一化／7项排队提交均0 mismatch；原样原生2项通过，
正式配置／transport原生3项与前端2项通过，所有临时源码逐字节恢复。
Windows DPAPI临时目录保存／重开及blank-keep覆盖合成凭据。

独立Python stdlib base64／UTF8／hashlib／hmac检查实际12次loopback
GET（WebDAV6／S3 6）均匹配完整输入，不导入项目crypto，不保存
认证header或原始凭据。证明这些请求的凭据字节，不泛化为完整SigV4
或真实provider兼容性。red／green／independent诊断与日志哈希均核对。

完整verify-remote-password-whitespace-final.log exit0：361前端／687
Rust／ACL2／KeePass25／QuickXML1363（4 ignored），format／check／
build／补丁来源／clippy通过。首次完整门仅在新增测试date切片写法
触发Clippy；改用byte slice后窄Clippy、原样与独立wire及完整门重新
通过，首次失败日志仍保留。单线程、debug0，live clipboard／S3／
config fixture未启用，跳过body和ignored不计运行覆盖。

remote-password-whitespace-final-proof.json保存fix patch、源码、原样
诊断与完整日志哈希；data-contracts／security-model／PITFALLS／TODO
已在修复提交同步，本次仅收尾证据。无真实配置／密码库／云端／设备
／APK／共享剪贴板／高资源执行；不能恢复此前已trim的凭据。
累计86 Confirmed含历史已修复项（Critical1／High19／Medium54／Low12），
原始b7c5d9f／15项／6.1分及历史库存artifact保持；完整审计／竞品／UI
目标仍active，手机另存为仍待替换APK／真机provider验证。

### Finding: SP-S76 远程设置输入框缺少可访问名称

- Severity: Medium
- Confidence: High
- Category: Accessibility
- Status: Confirmed（已修复，7d86e04）
- Affected area: S3 and WebDAV settings form fields
- Evidence:
  - File: src/lib/components/settings/RemoteSettingsPanel.svelte:124–385; src/lib/components/templates/form/TextField.svelte:26, 114 (d9c2af4)
  - Function / Module: RemoteSettingsPanel -> TextField input
  - Relevant behavior: visible headings are strong elements adjacent to the fields, with no label association. TextField supports ariaLabel but the panel does not pass it for any of its eight text/password fields. The profile Select and backup range already have names.
  - Reproduction: isolated localhost UI renders eight unnamed S3 and six unnamed WebDAV text/password controls. DOM checks independently exclude label, aria-label, aria-labelledby and title; native browser accessibility tree lacks their field descriptions. Only names/types/headings were inspected, without credential values or user settings. remote-form-name-confirmed-before.json SHA256 13219e10913032bc7544a54f5cf4306d80cba772daab143ff74465e8a7745d97.
- Problem: assistive technology receives generic inputs or placeholder examples instead of stable localized field names.
- Why it matters: users navigating the form through a screen reader cannot reliably distinguish endpoint, username/key, password, prefix and backup template, especially when placeholders are empty or become the same saved-credential hint. The whole connection form is affected; no credential disclosure or provider incident is claimed.
- Realistic failure scenario: a screen-reader user tabs through WebDAV settings to configure a connection; the password and username controls have no programmatic names even though headings are visible to sighted users.
- Minimal fix: supply TextField.ariaLabel from the same existing localized keys as visible headings, including protocol-specific endpoint and credential titles. Preserve shared primitives and theme tokens.
- Better long-term fix: check stable localized accessible names when reviewing every settings form; shared controls cannot infer nearby headings safely.
- Regression test suggestion: inspect rendered S3/WebDAV in both locales, verify all text/password fields have names matching visible headings, and confirm name-based locators plus Tab focus. Check dark/light and narrow layout without entering credentials.
- Estimated effort: 30–60 minutes including localized props and rendered verification.

本项先确认并记录再实施；仅独立localhost UI／源码证据，未访问用户配置／
密码库／真实凭据／系统剪贴板。累计87 Confirmed含历史已修复项
（Critical1／High19／Medium55／Low12），原始15项／6.1分保持。
手机另存为的新版APK／真机provider验证和整体审计／竞品目标仍未完成。

修复与验证（2026-11-11）：7d86e04 为8处TextField传入与可见标题相同
的本地化ariaLabel，包含S3／WebDAV端点和凭据差异。生产build独立
localhost中，中英文S3 8项／WebDAV6项名称均与标题一致；深浅色
1280px桌面／390×844窄屏保持30px控件高度，无页面横向溢出。
按名称定位与WebDAV端点Tab到用户名、solid焦点outline实际通过。
预览仅切换隔离origin的主题／语言，恢复中文深色及viewport默认。

check／build／format:check exit0；未为纯prop修复增加实现镜像测试。
原开发预览在构建目录watch发生EBUSY退出，生产预览启动后HTTP200，
新浏览器tab成功完成检查；预览进程已停止。remote-form-name-final-proof.json
保存源码hash及名称／布局结果；桌面和深浅窄屏截图保留。
theme-token gate使用既有TextField能力，无CSS／token／布局变化；
currency gate同步settings-panels.md，无IPC／schema／秘密处理变化。
没有真机读屏／provider／APK证明，不将此局部修复认定全站无障碍完成；
累计87历史Confirmed及原始15项／6.1分保持，整体目标仍active。

### Finding: SP-S77 无效远程配置重命名清空凭据存在提示并触发误报

- Severity: Low
- Confidence: High
- Category: Frontend state / Stability
- Status: Confirmed
- Affected area: Remote profile rename service and saved-credential placeholders
- Evidence:
  - File: src/lib/services/settings.ts:1372–1417; src/lib/components/settings/RemoteSettingsPanel.svelte:57–64, 128–129 (362524f)
  - Function / Module: appSettings.renameRemoteProfile -> secretPresence / showTip / schedulePersist
  - Relevant behavior: removes stored-credential presence and shows the rename-reset error before validating the profile, normalized name or collision. Same-name also executes the rename path; every call queues a config write.
  - Reproduction: actual service/presence/tips/queue with a masked synthetic IPC seam runs same, normalized-same, empty, whitespace-only and collision names for both S3 and WebDAV. All ten keep the settings content identical, yet lose secret presence, emit one error and cause one extra set_config. remote-profile-rename-red.json and immutable diagnostic hashes are saved in remote-profile-rename-red-provenance.json; no raw values logged.
  - Independent verification: the UI derives saved-credential placeholders directly from presenceMap[activePath]. The service's name validation occurs after clearing that state. Neither backend credential loss nor real-provider failure is claimed.
- Problem: an operation rejected or having no identity change nevertheless publishes the side effects of a successful rename.
- Why it matters: the user sees that credentials need re-entry and loses the saved placeholder despite retaining the original profile. Low reflects proven misleading UI state and redundant persistence, without proof of stored-secret destruction.
- Realistic failure scenario: the user clears the live profile-name input or enters a colliding name. The original profile remains selected but the UI falsely says it was renamed and credentials were reset.
- Minimal fix: validate profile existence, normalized name, unchanged identity and collisions before changing presence, displaying reset feedback or scheduling persistence. Keep successful rename behavior unchanged.
- Better long-term fix: perform validation before side effects for other profile operations too; removeRemoteProfile remains a separate unconfirmed scope.
- Regression test suggestion: execute both protocols' unchanged/invalid/collision cases against actual stores and queue, asserting unchanged presence/settings, no error tips and no IPC writes. Verify a valid rename changes identity and retains the existing reset warning only when stored credentials were present.
- Estimated effort: 30–60 minutes for validation ordering, service regressions and frontend verification.

累计88历史Confirmed（Critical1／High19／Medium55／Low13），原始15项／
6.1分保持。本项仅合成IPC与直接UI派生证据，无实际后台凭据丢失证明。
手机真机／竞品／全项目审计目标仍active。
