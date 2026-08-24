# Fuck My Shit Mountain 审计报告

**项目：** SecPivot
**审计模式：** full（全维度）
**日期：** 2026-08-24
**Reviewer:** ox-alpha (opencode)
**审计基线提交：** `f9aa08683a847b4b4e963f5af6577587e6693dc9`（2026-08-21）

---

## 1. 执行摘要 Executive Summary

SecPivot 是一个工程质量明显高于平均水平的项目。Rust 后端表现出罕见的纪律性：机密数据系统性地使用 zeroize 生命周期管理、本地写入采用 fsync + 原子重命名、两个网络服务均仅绑定回环地址且带请求体积上限、互斥锁中毒有防护、网络分发处理器用 `catch_unwind` 包裹以防锁污染。前端隐私卫生近乎完美：零遥测、零 console 输出、剪贴板带所有权校验的定时擦除。测试侧约 392 个 Rust 测试覆盖了核心路径（NIST/RFC 标准向量、错误密码拒绝、并发保存、合并冲突语义），远超同类个人项目的水平。

主要风险集中在三处：（1）KeePassHttp 桥接服务对本地任意进程/网页开放且关联审批无速率限制，配合 `Access-Control-Allow-Origin: *` 构成本地攻击面；（2）发布链完整性缺口——Windows 产物未签名、无 checksum/SBOM，CI 中所有 GitHub Action 仅按可变 tag 固定，无 Rust 工具链锁定文件；（3）前端 `+page.svelte` 已膨胀至 3338 行、91 个顶层函数，是最大的可维护性债务。另有少量静默 fallback（未知远程存储类型映射为 S3、WebDAV 吞 match 分支）和文档过期声明需要清理。

总体评价：**A（良好）**。没有发现高危或严重问题；所列问题均为局部可修，不需要重写。建议在下一个稳定版发布前优先处理发布链完整性与本地桥接面加固。

### 评分仪表盘

```
安全 Security     ████████░░  8.0  A   本地协议面 CORS * + 无关联速率限制；其余密钥/加密/输入边界卫生极佳
稳定 Stability    █████████░  8.5  A   原子写入+fsync、锁中毒防护、catch_unwind；个别 expect 在资源耗尽时转 abort
性能 Performance  ████████▌░  8.5  A   未发现热点问题；依赖极轻；god 组件或影响 UI 渲染但无实测证据
测试 Testing      ███████▌░░  7.5  A   392 个 Rust 测试含标准向量；IPC 胶水层与组件仅有源码正则级测试
可维护 Maintain   ██████▌░░░  6.5  B   +page.svelte 3338 行/91 函数；vault.ts JSDoc 约 31%；命名一致性优秀
设计 Design       ███████▌░░  7.5  A   SRP/原子性/fail-fast 执行到位；少量静默 fallback 与字符串哨兵契约
发布 Release      ██████▌░░░  6.5  B   版本校验门禁扎实；Windows 未签名、无 checksum/SBOM、Action 未按 SHA 固定
─────────────────────────────────────
综合 Overall      ███████▌░░  7.6  A
```

每个维度评分 0.0–10.0。**分数越高越好（10 = 干净，0 = 粪山）。** 评分为判断性评分而非机械扣分。

### 发现统计 Finding Statistics

| 严重度   | 数量   | 已确认 | 存疑  |
| -------- | ------ | ------ | ----- |
| Critical | 0      | 0      | 0     |
| High     | 0      | 0      | 0     |
| Medium   | 6      | 6      | 0     |
| Low      | 10     | 10     | 0     |
| Info     | 0      | 0      | 0     |
| **合计** | **16** | **16** | **0** |

> 另有 5 条补充观察项（L-1、L-2、L-3、I-1、I-2）列于第 4 节末尾简表，属低危/信息级，为保持与详细发现卡计数一致不计入上表。

---

## 2. 项目地图 Project Map

**架构分层：**

- **Rust 后端（src-tauri/src）**：`vault/`（库模型、会话、持久化、合并）、`crypto/`（AES/HMAC/TOTP/KDF 原语）、`bridge/`（KeePassHttp 兼容 HTTP 服务，端口 19455）、`rpc/`（KeePassRPC WebSocket + SRP-6a，端口 12546）、`commands/`（Tauri IPC 命令层）、`remote/`（WebDAV/S3 远程同步与备份）、`platform/`（Win32 FFI：剪贴板、DPAPI、自动键入、焦点、护盾）、`config/`。
- **前端（src/lib, src/routes）**：Svelte 5 runes，服务层 `services/vault.ts` 集中承载约 80 处 invoke 调用；单一 `.svelte.ts` store；`+page.svelte` 为应用壳。
- **入口/初始化**：`main.rs` → Tauri Builder 注册命令与状态 → 前端 SvelteKit adapter-static SPA。
- **状态归属**：库会话状态归 Rust 侧 `sessions.rs`（PersistencePermit 信号量串行化持久化）；前端仅持有 UI 态。
- **持久化**：本地 kdbx 文件经 `.tmp` 同步写 + rename 原子替换；远程保存带 SHA-256 冲突检测与修订守卫；`remote/backup.rs` 时间戳备份带保留修剪。
- **外部接口**：回环 HTTP（bridge）、回环 WS+SRP（rpc）、Tauri IPC、HIBP HTTPS（k-匿名，可选）、favicon 抓取、WebDAV/S3。
- **安全边界**：主密钥/密码仅在内存会话内、zeroize 收尾；新关联需用户显式批准（120 秒一次性令牌）；CSP 严格且区分 dev/prod；capabilities 最小授权。
- **AI 面**：产品代码无 LLM/RAG 表面（`.opencode/` 仅为工具链配置，不在产品边界内）→ AI-Safety 不适用。
- **成本驱动**：仅 HIBP/favicon 外部请求，量级可忽略。
- **高风险区域**：`bridge/server.rs`（本地明文协议）、`rpc/`（自定义加密协议）、`vault/persist.rs`（数据落盘）、发布链（release.yml / scripts）。

### 覆盖矩阵 Coverage Matrix

| 维度                            | 覆盖度       | 检查证据                                                                                                  | 排除 / 限制                          |
| ------------------------------- | ------------ | --------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| 架构 Architecture               | High         | 全目录树、模块依赖方向、invoke 调用点分布（~120 处）、状态归属分析                                        | 未运行时追踪                         |
| 安全 Security                   | High         | unwrap/panic 全量扫描、zeroize 点位清单、两网络服务认证模型、unsafe 全部 ~36 处逐一核对、CSP/capabilities | 未做动态渗透/模糊测试                |
| 稳定 Stability                  | High         | 错误传播路径、锁序、poison 处理、原子写实现（util.rs:24-56）、fallback 扫描                               | 无长时间压力运行                     |
| 性能 Performance                | Medium       | Cargo profile 策略、依赖重量、线程模型审查                                                                | 无性能基准/剖析数据                  |
| 测试 Testing                    | High         | 14 个前端测试文件逐个阅读、392 个 Rust 测试分布统计、runner 配置                                          | 未测覆盖率百分比                     |
| 可维护 Maintainability          | High         | 文件行数排行、JSDoc 覆盖率统计、命名/导入一致性抽查                                                       | —                                    |
| 设计 Design                     | High         | principles 对照（SRP/DRY/fail-fast/CQS 等）、fallback 分类扫描                                            | —                                    |
| 发布 Release                    | High         | ci.yml/release.yml 逐行、scripts/*.mjs                                                                    | ps1、tauri.conf.json、Cargo profiles | 未实际跑一次发布 |
| 文档 Documentation              | High         | README/AGENTS/docs/* 与 package.json、release.yml 交叉验证                                                | —                                    |
| 配置 Configuration              | High         | tauri.conf.json、capabilities、env var 处理、默认值审查                                                   | Android 构建未实机验证               |
| 可观测 Observability            | Medium       | 日志语句审查（eprintln 内容检查）、错误分类函数                                                           | 桌面应用无 metrics/alerting 面       |
| 数据完整性 Data-Integrity       | High         | 原子写、合并不变量、备份策略、三阶段 RPC 写、冲突标记                                                     | 备份恢复未实测演练                   |
| 隐私 Privacy                    | High         | 遥测/console/network 全量搜索（零命中）、剪贴板/HIBP/日志脱敏审查                                         | —                                    |
| 无障碍 Accessibility            | Medium       | aria 属性统计（146 处）、ModalShell 焦点行为、键盘处理                                                    | 未用屏幕阅读器实测                   |
| 供应链 Supply-Chain             | High         | workflow 权限/action 固定方式、lockfile 提交状态、工具链固定、签名/checksum/SBOM                          | 依赖 CVE 数据库未全量比对            |
| 成本 Cost                       | Medium       | 外部调用清单、后台任务、存储增长点                                                                        | 单机桌面应用，成本低风险             |
| AI 安全 AI-Safety               | Not assessed | 产品代码无 LLM/prompt/tool 调用表面（rg 零命中）                                                          | 不适用                               |
| Fallback                        | High         | unwrap_or_default / 空 match 分支全量扫描并逐个定性                                                       | —                                    |
| 测试真实性 Testing-Authenticity | High         | 逐文件评估 mock 策略与断言对象（行为 vs 实现细节）                                                        | —                                    |
| 类型安全 Type-Safety            | High         | as any/@ts-ignore 零命中验证、tsconfig strict、Rust Result 一致性                                         | —                                    |
| 前端状态 Frontend-State         | High         | $effect 清单、store 数量、组件行数排行、invoke 分散度                                                     | —                                    |
| 后端 API Backend-API            | Medium       | commands 层错误契约（Result<T,String>）、输入校验、sentinel 契约                                          | IPC 面非传统 REST API                |
| 依赖重量 Dependency-Weight      | High         | package.json（3 个运行时依赖）、Cargo.toml feature 纪律                                                   | 未逐包统计传递体积                   |
| 代码一致性 Code-Consistency     | Medium       | 命名约定、导入组织、错误处理模式抽样                                                                      | 未逐文件比对风格                     |
| 注释覆盖 Comment-Coverage       | Medium       | vault.ts 导出符号 vs JSDoc 计数、过期注释搜索                                                             | —                                    |

**覆盖率说明**：检查范围为一方源码（69 个 Rust、49 个 Svelte、32 个 TS 文件）、14 个测试文件、2 个 CI workflow、10 个脚本、全部文档。排除项：`.git`、`node_modules`、`.venv`、`build/`、`src-tauri/target`、`src-tauri/gen`、图标二进制与 lockfile 内容。所有结论基于静态证据，未执行动态模糊测试或性能剖析。

---

## 3. 重点风险 Top Risks

1. **[M-1] KeePassHttp 本地协议面过宽** — Medium — 回环绑定下任意本地进程/网页可访问 19455 端口，`Access-Control-Allow-Origin: *` 且关联审批无速率限制，可被反复弹出批准框骚扰。
2. **[R-1] Windows 发布产物未签名、无 checksum/SBOM** — Medium — 用户无法验证安装包来源完整性，供应链信任链断裂于最后一公里。
3. **[R-2] CI Actions 未按 commit SHA 固定** — Medium — `dtolnay/rust-toolchain@stable`、`tauri-apps/tauri-action@v0` 等均为可变引用，上游被攻破即直接注入构建。
4. **[R-3] 无 Rust 工具链锁定文件** — Medium — 缺 `rust-toolchain.toml`，构建在 `@stable` 漂移下不可重现。
5. **[MA-1] `+page.svelte` god-component** — Medium — 3338 行、91 个顶层函数、混合导入导出编排/窗口管理/面板状态机。
6. **[A-1] ModalShell 无焦点陷阱** — Medium — 键盘用户 Tab 可进入模态背景内容，密码管理器场景下是真实可用性缺陷。
7. **[F-1] WebDAV 空 match 分支吞错** — Low-Medium — `webdav.rs:210` `_ => {}` 可能掩盖服务器端异常。
8. **[F-2] 未知远程存储类型静默映射为 S3** — Low — `remote/mod.rs:81` 配置拼写错误不报错反而建 S3 客户端。
9. **[T-1] IPC 胶水层仅有正则级测试** — Low — `services/vault.ts` 运行时零测试，靠 `component-contracts.test.mjs` 的源码正则维持契约。
10. **[S-1] 剪贴板自动清除仅由前端定时器保证** — Low — webview 进程被杀则秘密滞留剪贴板。
11. **[ST-1] 资源耗尽路径的 expect 转 abort** — Low — `remote/mod.rs:55-59`、`rpc/server/mod.rs:146`。
12. **[D-1] 过期文档四处** — Low — RELEASE.md:56 四 ABI universal 声明、TODO.md:50 rust-s3 引用、android.md 自相矛盾、SKILL.md:148 错误基路径。

其余 Low/Info 发现详见第 4 节。

---

## 4. 详细发现 Detailed Findings

### Finding: KeePassHttp 本地协议面对任意本地进程/网页开放且无速率限制

- Severity: Medium
- Confidence: High
- Category: Security
- Status: Confirmed
- Affected area: bridge/server.rs（KeePassHttp 兼容服务）
- Evidence:
  - File: src-tauri/src/bridge/server.rs:94, :27-28, :452, :346-371
  - Function / Module: `accept_loop` / 关联审批（ApprovalBoard）
  - Relevant behavior: 服务仅绑定 `127.0.0.1:19455`，响应头 `Access-Control-Allow-Origin: *`；新关联需用户批准（120s 一次性令牌），但对同一客户端反复发起未关联请求无任何速率限制；头部上限 16 KiB、body 上限 1 MiB、IO 超时 10s。
- Problem: 浏览器在多数配置下允许向 localhost 发起 fetch，因此本机上运行的任意网页和恶意进程均可直接 POST 到该端口。虽然解密响应需要已建立的关联密钥，但未认证请求仍会进入 dispatch，且可以不断触发批准弹窗（approval spam），构成对用户的社工骚扰面与轻度 DoS。
- Why it matters: 密码管理器的威胁模型中，本机浏览器是半可信环境。协议本身继承自 KeePassHttp（KeePassXC 行为一致），但"与竞品同样暴露"不等于"无可收敛空间"。
- Realistic failure scenario: 用户浏览被植入脚本的页面 → 页面循环 POST `test-associate` 到 19455 → SecPivot 反复置顶弹出"是否允许新程序关联"对话框 → 用户疲劳后误点允许，恶意本地进程获得查询条目能力的入口。
- Minimal fix: 对同一 peer 的未认证/失败关联请求加每秒计数限流（如 5 次/分钟）；将 `Access-Control-Allow-Origin: *` 收紧为回显已知浏览器扩展 origin 或移除该头。
- Better long-term fix: 为未关联请求引入预检令牌（preflight token），仅在设置面板主动展示时放行。
- Regression test suggestion: 在 `bridge/tests.rs` 增加"N 秒内超过 N 次未知 key 的 test-associate 请求返回限流响应"的单测。
- Estimated effort: 2–4 小时

### Finding: Windows 发布产物未签名，且无 checksum/SBOM/updater 签名

- Severity: Medium
- Confidence: High
- Category: Release / Supply-Chain
- Status: Confirmed
- Affected area: .github/workflows/release.yml
- Evidence:
  - File: .github/workflows/release.yml:111-150（tauri-action 上传，无 signing 步骤）
  - Function / Module: build job
  - Relevant behavior: Android APK 经 keystore 签名并用 apksigner verify 校验（L250-266）；Windows NSIS 安装包既无 Authenticode 签名，也无 SHA256SUMS、SBOM 或 tauri updater 签名生成步骤；无 updater 插件配置。
- Problem: Windows 用户下载安装包后没有任何来源完整性校验手段；镜像投毒或传输篡改无法被发现。对一个密码管理器而言这是发布信任链的实质缺口。
- Why it matters: 供应链攻击最经济的落点是最终产物分发环节；前面所有构建期加固（严格 CSP、最小 capabilities）都被未签名的产物稀释。
- Realistic failure scenario: 攻击者劫持某下载镜像/短链 → 替换未签名的 `SecPivot_x64-setup.exe` → 用户安装后凭据尽失且无任何告警线索。
- Minimal fix: 在 release job 增加 SHA256SUMS 文件生成并随 release 上传（约 30 分钟）；README 发布页公布指纹校验方法。
- Better long-term fix: 申请代码签名证书接入 Authenticode；启用 tauri updater + minisign 公钥。
- Regression test suggestion: `release-version.test.mjs` 风格新增脚本测试：断言发布产物清单必须包含 SHA256SUMS。
- Estimated effort: checksum 半天；签名证书流程数天

### Finding: CI Actions 以可变 tag/branch 引用，未按 SHA 固定

- Severity: Medium
- Confidence: High
- Category: Supply-Chain
- Status: Confirmed
- Affected area: .github/workflows/ci.yml, release.yml
- Evidence:
  - File: .github/workflows/ci.yml:14,16,40,42,44,55; release.yml:111,140,167
  - Function / Module: steps uses
  - Relevant behavior: `actions/checkout@v7`、`actions/setup-node@v7`、`dtolnay/rust-toolchain@stable`（分支引用！）、`mozilla-actions/sccache-action@v0.0.11`、`Swatinem/rust-cache@v2`、`taiki-e/install-action@v2`、`tauri-apps/tauri-action@v0`——全部为可变引用，无一使用 commit SHA。
- Problem: 任一 action 仓库被攻破或其 major tag 被移动，构建环境即被注入恶意代码，且 release.yml 持有 `contents: write` 和 Android 签名 secrets，爆炸半径直达发布产物。
- Why it matters: 这是 GitHub Actions 供应链的标准加固项；本项目 release 工作流权限较高，风险不成比例地放大。
- Realistic failure scenario: `tj-actions` 类事件重演 → `tauri-action@v0` 指向恶意 commit → 下一个 tag push 时 release job 泄露 ANDROID_KEYSTORE_* secrets 并在产物中植入后门。
- Minimal fix: 将全部 action 固定到具体 commit SHA（Dependabot/renovate 可自动维护注释版本号）。
- Better long-term fix: 同时给 release.yml 各 job 显式收窄 permissions（当前 top-level contents:write 覆盖 verify/android job）。
- Regression test suggestion: 在 `release-script.test.mjs` 增加正则审计：workflow 内 `uses:` 必须匹配 `@<40位hex>`。
- Estimated effort: 1–2 小时

### Finding: 无 Rust 工具链锁定文件，构建不可精确重现

- Severity: Medium
- Confidence: High
- Category: Release / Reproducibility
- Status: Confirmed
- Affected area: 仓库根目录 / CI
- Evidence:
  - File: .github/workflows/ci.yml:40（`dtolnay/rust-toolchain@stable`）；仓库无 rust-toolchain.toml、无 .nvmrc（package.json engines 也未声明 Node 版本约束）
  - Function / Module: toolchain resolution
  - Relevant behavior: Rust 编译器随 stable 通道漂移；Node 用 `"24"` 主版本浮动。
- Problem: 相同 tag 在不同时间构建可能产生不同 codegen 甚至编译失败；发布产物的可重现性无从谈起。
- Why it matters: lockfile（package-lock.json、Cargo.lock）已正确提交，唯独工具链这一层缺失，属于低成本高收益补齐项。
- Realistic failure scenario: stable 某日引入破坏性 lint/clippy 变更 → 当晚打 hotfix tag → release job 的 verify 门禁意外失败，发布中断。
- Minimal fix: 添加 `rust-toolchain.toml`（pin 具体小版本）与 `.nvmrc`，CI 的 dtolnay action 会自动读取前者。
- Better long-term fix: 记录发布构建的完整工具链指纹进 RELEASE 元数据。
- Regression test suggestion: CI 步骤断言 `rustc --version` 与 rust-toolchain.toml 一致。
- Estimated effort: 30 分钟

### Finding: +page.svelte 膨胀为 3338 行 god-component

- Severity: Medium
- Confidence: High
- Category: Maintainability / Frontend-State
- Status: Confirmed
- Affected area: src/routes/+page.svelte
- Evidence:
  - File: src/routes/+page.svelte:1-3338
  - Function / Module: 应用根组件（91 个顶层函数、7 个 `$effect`、9 处直接 `invoke()`）
  - Relevant behavior: 同时承担壳布局、CSV/XML 导出编排（:979/:1026/:1070）、导入解析（:1387/:1425）、autotype 触发（:2834）、窗口尺寸管理（:374）、面板宽度拖拽状态机（:616-784）、TCato 覆盖层逻辑（:2108）。
- Problem: 单一组件聚合了至少四类互不相干的职责，任何功能改动都在同一文件碰撞；面板宽度 effect 间存在文档化的顺序耦合（:485-489, :780-784 注释自证脆弱性）。
- Why it matters: 这是前端唯一系统性可维护性债务——其余组件均通过 `services/vault.ts` 走服务层，模式健康。
- Realistic failure scenario: 新增一种导入格式 → 修改 :1387 区域 → 与面板宽度状态机的变量作用域意外冲突（同名局部变量）→ 回归仅在特定布局状态下显现。
- Minimal fix: 将导入/导出编排抽为 `lib/services/io.ts`，窗口管理与面板宽度状态机抽为 composable，+page.svelte 目标降到 <1000 行。
- Better long-term fix: 按"shell / io / layout-state"三模块拆分并各自补运行时测试。
- Regression test suggestion: 先为抽取的 io 编排逻辑写 node:test 单测（复用现有 fake-invoke 模式），再重构。
- Estimated effort: 2–3 天

### Finding: ModalShell 无焦点陷阱与初始焦点管理

- Severity: Medium
- Confidence: High
- Category: Accessibility
- Status: Confirmed
- Affected area: src/lib/components/ModalShell.svelte
- Evidence:
  - File: src/lib/components/ModalShell.svelte:40-56
  - Function / Module: 模态容器
  - Relevant behavior: 有 `role="dialog"`、`aria-modal`、Escape 关闭（svelte:window 监听），但无 focus trap、无打开时初始焦点设置、关闭后焦点不归还触发元素。
- Problem: 键盘用户在模态打开时仍可 Tab 进入背景内容，屏幕阅读器的 aria-modal 声明与真实焦点行为不一致。
- Why it matters: 密码管理器的目标用户群包含大量键盘重度用户；146 处 aria 使用说明项目重视 a11y，此处是最薄弱一环。
- Realistic failure scenario: 用户键盘操作打开"编辑条目"对话框 → Tab 若干次后焦点落入背后表格 → Enter 触发了删除按钮而用户以为自己在操作对话框。
- Minimal fix: 打开时聚焦对话框容器（tabindex="-1"），Tab 循环限制在对话框 DOM 子树内（约 30 行实现），关闭时归还焦点。
- Better long-term fix: 封装 `<dialog>` 元素原生 showModal() 的焦点语义。
- Regression test suggestion: 组件测试断言 modal 打开后 document.activeElement 位于对话框内、Tab 循环不逃逸。
- Estimated effort: 2–4 小时

### Finding: WebDAV 客户端空 match 分支可能吞掉服务器错误

- Severity: Low
- Confidence: Medium
- Category: Stability / Fallback
- Status: Confirmed（分支存在）/ Suspected（是否实际掩盖错误待确认）
- Affected area: src-tauri/src/remote/webdav.rs
- Evidence:
  - File: src-tauri/src/remote/webdav.rs:210
  - Function / Module: PROPFIND/status 处理
  - Relevant behavior: `_ => {}` 空 arm，未记录也未上报。
- Problem: 非 2xx/预期状态码的响应被静默忽略，远程同步异常可能表现为"无事发生"而非明确报错。
- Why it matters: 远程同步是数据完整性关键路径，静默分支违背 fail-fast。
- Realistic failure scenario: 自建 WebDAV 返回 423 Locked → 空 arm 吞掉 → 用户以为同步成功，实际两端数据开始分叉，直到下次哈希冲突检测才暴露。
- Minimal fix: 将 `_ => {}` 改为记录状态码的警告日志或返回 Err。
- Better long-term fix: 为 remote 层建立统一的状态码→错误分类表。
- Regression test suggestion: `remote/tests.rs` 增加"非常规状态码产生可见错误/日志"用例。
- Estimated effort: 30 分钟

### Finding: 未知远程存储 kind 静默回退为 S3

- Severity: Low
- Confidence: High
- Category: Configuration / Fallback
- Status: Confirmed
- Affected area: src-tauri/src/remote/mod.rs
- Evidence:
  - File: src-tauri/src/remote/mod.rs:81
  - Function / Module: 存储工厂
  - Relevant behavior: `_ => Ok(Arc::new(S3Storage::new(cfg)?))` —— 未识别的 kind 字符串一律构造 S3 客户端。
- Problem: 配置值拼写错误（如 "webdav " 带尾随空格）不会报错，而是以错误的协议连接失败，报错信息指向网络而非配置。
- Why it matters: 防御性猜测掩盖配置错误，延长排障时间；正确行为是 fail-fast。
- Realistic failure scenario: 用户手改配置把 "webdav" 写成 "WebDav" → 连接怪异失败 → 排障一小时后发现是大小写。
- Minimal fix: 显式匹配 `"s3"`，其余返回 Err("unknown storage kind")。
- Better long-term fix: 配置反序列化时用 enum 而非 String。
- Regression test suggestion: `config/tests.rs` 增加"非法 kind 必须报错"用例。
- Estimated effort: 15 分钟

### Finding: IPC 胶水层缺少运行时测试，契约靠源码正则维持

- Severity: Low
- Confidence: High
- Category: Testing / Testing-Authenticity
- Status: Confirmed
- Affected area: tests/component-contracts.test.mjs; src/lib/services/vault.ts
- Evidence:
  - File: tests/component-contracts.test.mjs:264-285, :336-348; package.json:18
  - Function / Module: 源码正则断言
  - Relevant behavior: 该文件读取 .svelte/.rs 源文本做 regex 断言（事件名对齐、sessionView.capture 门控等）；vault.ts 的 invoke 封装无任何运行时执行测试；无组件/DOM 测试框架。
- Problem: 正则契约测试对重排/重命名极度脆弱（改名即假红，真 bug 却可能漏），且无法捕获序列化/参数类型等运行时错误。Rust 侧 392 个测试弥补了领域逻辑，TS 侧 glue 成为盲区。
- Why it matters: 属于真实的置信度缺口而非测试数量问题——但 glue 层薄、类型严格（strict + 零 as any），实际逃逸风险有限。
- Realistic failure scenario: 重构 vault.ts 参数名 → Rust 端命令期待旧名 → invoke 运行时失败 → 正则测试全绿（两边字符串都同步改了注释除外）。
- Minimal fix: 为 vault.ts 最高频的 5 个封装写 node:test 单测（mock @tauri-apps/api invoke），验证参数透传与错误包装。
- Better long-term fix: 引入轻量组件测试（如 svelte + happy-dom）覆盖 LockScreen 与 EntryTable 关键交互。
- Regression test suggestion: 见 minimal fix 本身。
- Estimated effort: 1 天

### Finding: 剪贴板定时擦除完全依赖前端存活

- Severity: Low
- Confidence: High
- Category: Privacy / Security
- Status: Confirmed
- Affected area: src/lib/utils/clipboard.ts; src-tauri/src/platform/clipboard.rs
- Evidence:
  - File: src/lib/utils/clipboard.ts:38-46, :58-72; src-tauri/src/commands/clipboard.rs:11-21
  - Function / Module: 定时清除调度
  - Relevant behavior: 后端只提供 `clipboard_read_text`/`clipboard_clear`，超时清除由前端 setTimeout 实现（带内容所有权校验，设计良好）；但 webview 进程崩溃/被杀则定时器消失。
- Problem: 秘密可能在剪贴板中无限期滞留，违背最小暴露原则。
- Why it matters: 概率低（桌面应用正常退出会走清理路径），但属密码管理器应兜底的边界。
- Realistic failure scenario: 用户复制密码 → 系统 OOM 杀掉 webview → 密码留在剪贴板直到下次复制。
- Minimal fix: 在 Rust 侧 spawn 一个接收 secret 副本的一次性计时清除任务（zeroized 副本 + 所有权比对），前端调用改为触发后端计时。
- Better long-term fix: 后端统一管理剪贴板秘密生命周期（含锁定/退出钩子）。
- Regression test suggestion: `platform/clipboard.rs` 单测模拟"清除请求注册后即使无前端心跳也按时清空"。
- Estimated effort: 3–5 小时

### Finding: 资源耗尽路径上的 expect 将可恢复失败转为进程 abort

- Severity: Low
- Confidence: High
- Category: Stability
- Status: Confirmed
- Affected area: src-tauri/src/remote/mod.rs; src-tauri/src/rpc/server/mod.rs
- Evidence:
  - File: src-tauri/src/remote/mod.rs:55-59; rpc/server/mod.rs:146
  - Function / Module: OnceLock 初始化 / `try_clone`
  - Relevant behavior: 初始化线程 spawn 失败或 socket clone 失败（fd 耗尽）直接 expect abort。
- Problem: 这两处失败理论上可恢复（降级"远程同步不可用"/拒绝新连接），abort 使整个密码库会话丢失。
- Why it matters: 触发条件罕见（fd 耗尽需本机大量连接），故评 Low；但修复廉价。
- Realistic failure scenario: 恶意本地进程耗尽句柄 → 用户此时解锁远程同步 → 整个应用 abort，正在编辑的未保存修改丢失。
- Minimal fix: 将 expect 改为记录错误并把远程功能标记为不可用；try_clone 失败则关闭该连接并继续 accept。
- Better long-term fix: 为两个 server 增加 accept 错误退避循环。
- Regression test suggestion: 难以单测；代码评审级修复即可，注明理由。
- Estimated effort: 1–2 小时

### Finding: 会话切换存在双查找后 unwrap 的脆弱模式

- Severity: Low
- Confidence: High
- Category: Stability / Concurrency
- Status: Confirmed
- Affected area: src-tauri/src/vault/sessions.rs
- Evidence:
  - File: src-tauri/src/vault/sessions.rs:382-390
  - Function / Module: switch_active / parked 队列
  - Relevant behavior: :382 `parked.get_mut(...).ok_or_else(...)` 校验存在，:390 `parked.remove(session_id).unwrap()` 依赖同一锁区间内不变量。
- Problem: 当前正确（同一 Mutex 区间），但不变量跨 8 行成立，重构时极易破坏且 unwrap 会以 panic 形式爆发。
- Why it matters: catch_unwind 保护的是网络 handler，这里 panic 发生在会话管理层，代价更高。
- Realistic failure scenario: 后续贡献者在 :385 与 :390 之间插入提前 return → 不变量断裂 → 生产 panic。
- Minimal fix: 合并为一次 `let entry = parked.remove(id).ok_or_else(...)?` 再使用。
- Better long-term fix: 无需更大改动。
- Regression test suggestion: 现有 sessions.rs 测试已覆盖切换路径；补充"切换中途会话已被移除"的防御性用例。
- Estimated effort: 15 分钟

### Finding: REMOTE_CHANGED 字符串哨兵作为跨层错误契约

- Severity: Low
- Confidence: High
- Category: Type-Safety / Backend-API
- Status: Confirmed
- Affected area: src-tauri/src/vault/persist.rs ↔ src/lib/services/vault.ts
- Evidence:
  - File: src-tauri/src/vault/persist.rs:30-32（`\n` 结尾 sentinel 前缀）；前端以前缀匹配分支
  - Function / Module: 远程保存冲突信号
  - Relevant behavior: 错误通道统一为 `Result<T, String>`，冲突靠约定字符串前缀传递，是全代码唯一的程序化错误匹配点。
- Problem: 字符串哨兵脆弱——任一侧措辞调整即静默失效，冲突检测退化为普通错误提示。
- Why it matters: 冲突检测保护用户数据不被覆盖，是该项目数据完整性设计的支柱之一。
- Realistic failure scenario: 重构时把 sentinel 改为中文消息 → 前缀不再匹配 → 远程变更检测失效 → 用户强制覆盖丢掉远端新数据。
- Minimal fix: 定义常量并在两侧引用同一份（如通过生成的 TS 常量或 component-contracts 正则已有 IPC 名对齐机制扩展到该 sentinel）。
- Better long-term fix: 错误升级为结构化 `{code, message}` JSON。
- Regression test suggestion: component-contracts.test.mjs 增加断言：persist.rs 中 sentinel 字符串与前端匹配串逐字节一致。
- Estimated effort: 1 小时

### Finding: HIBP 校验用的密码副本未 zeroize

- Severity: Low
- Confidence: High
- Category: Privacy
- Status: Confirmed
- Affected area: src-tauri/src/vault/breach.rs
- Evidence:
  - File: src-tauri/src/vault/breach.rs:125-130
  - Function / Module: 批量泄露检查
  - Relevant behavior: 为计算 SHA-1 克隆的密码 Vec<u8> 用完后直接丢弃，未 wipe（对比 helpers.rs:27 的 wipe_secret_bytes 纪律）。
- Problem: 与项目自身的机密卫生标准不一致；内存残留窗口虽短但违背统一纪律。
- Why it matters: 项目其他所有密文路径都 zeroize，此处的例外会成为后续复制粘贴的坏样板。
- Realistic failure scenario: 信息影响有限（内存快照攻击场景），主要是纪律漂移风险。
- Minimal fix: 使用后调用现有 `wipe_secret_bytes`。
- Better long-term fix: 无。
- Regression test suggestion: 无法直接断言内存擦除；以 clippy/lint 规则或评审清单覆盖。
- Estimated effort: 15 分钟

### Finding: 两台回环服务 thread-per-connection 无连接数上限

- Severity: Low
- Confidence: High
- Category: Performance / Stability
- Status: Confirmed
- Affected area: bridge/server.rs; rpc/server/mod.rs
- Evidence:
  - File: src-tauri/src/bridge/server.rs:132-148; rpc/server/mod.rs:255-270
  - Function / Module: accept_loop
  - Relevant behavior: 每连接 spawn 线程，无并发上限；有 1 MiB body/frame 上限与 IO 超时缓解。
- Problem: 本地进程可通过连接洪水消耗线程资源。loopback-only + 上限缓解使其仅为理论 DoS。
- Why it matters: 修复简单（Semaphore 上限），收益是把"理论"变成"不可能"。
- Realistic failure scenario: 恶意软件在本机开数千连接 → 内存增长 → 应用变慢。已有更直接的恶意软件威胁，此项边际风险低。
- Minimal fix: accept 循环外包 `Arc<Semaphore>` 限流（如 64）。
- Better long-term fix: 迁移到 tokio 异步 accept。
- Regression test suggestion: 集成测试并发 200 连接全部得到响应或明确拒绝。
- Estimated effort: 2 小时

### Finding: 文档存在四处过期/矛盾声明

- Severity: Low
- Confidence: High
- Category: Documentation
- Status: Confirmed
- Affected area: RELEASE.md, TODO.md, docs/android.md, skills/version-release/SKILL.md, skills/secpivot-dev/SKILL.md
- Evidence:
  - File: RELEASE.md:56（"四 ABI universal release APK"，实际 release.yml:243-247 为 aarch64+x86_64 split-per-abi）；TODO.md:50（引用已移除的 rust-s3 0.34）；docs/android.md:26 vs :64（TARGET_RANLIB 要求前后矛盾）；skills/version-release/SKILL.md:164-170（同四 ABI + openssl-sys 陈述）；skills/secpivot-dev/SKILL.md:148（基路径写成 B:\Program\Project\SecPivot，缺 Open 目录段）
  - Function / Module: 文档
  - Relevant behavior: 与 PITFALLS.md:58-59/69 的现状描述及 release.yml 实际行为不符。
- Problem: 维护工作流的人若依据 SKILL.md 的 CI 描述决策会被误导；新贡献者按 android.md 前半部分配置会做无用功。
- Why it matters: 该项目文档密度高、通常准确，这四处是少数漂移点，趁热修复成本最低。
- Realistic failure scenario: 下次 Android 发布排障时按 SKILL.md 检查 openssl-sys/TARGET_RANLIB，浪费半天。
- Minimal fix: 修正上述五处文本（各 1-2 行）。
- Better long-term fix: 在 release 脚本测试中加入"文档不得提及 universal APK/rust-s3"的守卫（可选，略过度）。
- Regression test suggestion: 无需；纯文本修正。
- Estimated effort: 30 分钟

### 其余 Low / Info 补充观察项（简表，不重复计入上方统计）

以下为低危/信息级观察，未单独出具发现卡：

| ID  | 严重度 | 类别            | 摘要                                                         | 证据                                                            |
| --- | ------ | --------------- | ------------------------------------------------------------ | --------------------------------------------------------------- |
| L-1 | Low    | Maintainability | vault.ts 约 90 个导出仅 28 个有 JSDoc（31%）                 | src/lib/services/vault.ts                                       |
| L-2 | Low    | Frontend-State  | `void invoke(...)` fire-and-forget 三处可能吞 rejection      | LockScreen.svelte:36, TcatoOverlay.svelte:41, +page.svelte:2834 |
| L-3 | Low    | Configuration   | version.mjs 中 prettier 失败视为非致命                       | scripts/version.mjs:38-40                                       |
| I-1 | Info   | Supply-Chain    | ci.yml 未声明显式 permissions 块（继承默认）                 | .github/workflows/ci.yml                                        |
| I-2 | Info   | Maintainability | 中文 UI 字符串硬编码内联，未来 i18n 受阻（项目语境下可接受） | ModalShell.svelte:73-74 等                                      |

---

## 5. 架构分析 Architecture Analysis

- Coverage: High
- Inspected evidence: 模块目录结构、依赖方向梳理、invoke 调用点分布统计、状态归属追踪
- Exclusions / limits: 未做运行时依赖图分析

架构整体清晰：Rust 侧 vault/crypto/bridge/rpc/commands/remote/platform 边界分明，依赖方向单向（commands → vault/remote，network → dispatch → session），前端服务层集中了 80/~120 处 invoke。状态归属明确（库状态独占于 Rust sessions，前端仅 UI 态）。

### 架构摘要

| 子类                | 数量 | 影响区域                               | 建议动作                                                   |
| ------------------- | ---- | -------------------------------------- | ---------------------------------------------------------- |
| ModuleBoundary      | 1    | +page.svelte                           | 拆分导入导出/窗口管理/布局状态机（见 MA-1）                |
| DependencyDirection | 0    | —                                      | —                                                          |
| StateOwnership      | 0    | —                                      | —                                                          |
| BoundaryContract    | 1    | persist.rs↔vault.ts                    | REMOTE_CHANGED 哨兵结构化（D-1/Low 表外，见第 4 节独立卡） |
| EvolutionRisk       | 1    | settings 面板绕过 services 直接 invoke | 收敛到 services 层保持统一                                 |

## 6. 安全分析 Security Analysis

- Coverage: High
- Inspected evidence: 全量 unwrap/panic 扫描（生产路径约 40 处逐一核验）、zeroize 点位清单、双网络服务认证与限流模型、36 处 unsafe 逐一定性（全部 Win32 FFI）、CSP/capabilities 审查、日志脱敏检查
- Exclusions / limits: 未执行动态渗透测试、模糊测试或依赖 CVE 全量比对

亮点值得记录：错误分类函数 `classify_open_error`（helpers.rs:608-623）刻意将错密码/MAC 失败折叠为统一消息，杜绝 oracle；HIBP 采用规范 k-匿名（仅 5 hex 前缀离机）；RPC Conn Drop 时清零 key_secret/session_key（rpc/server/mod.rs:295-304）。无 Critical/High 安全发现。

主要遗留即 M-1（本地协议面）与 L 级剪贴板兜底（S-1）、HIBP 副本未擦除。

## 7. 稳定分析 Stability Analysis

- Coverage: High
- Inspected evidence: 错误传播链、锁序与 poison 处理（sessions.rs:25,119,137 等）、catch_unwind 包裹点（bridge/server.rs:237-250, rpc/server/mod.rs:448-460）、原子写实现（util.rs:24-56）、阻塞调用隔离（remote/mod.rs:43-60,87-89）
- Exclusions / limits: 无长时间压力/浸泡测试

稳定性设计成熟：PersistencePermit 串行化持久化、只读降级仅在真实持久化失败时触发（persist.rs:92-99）、120s 关联等待置于 vault 互斥锁之外且有专项测试（bridge/server.rs:682-721）。遗留问题均为低危（ST-1 expect abort、F-1 吞分支、L-4 双查找 unwrap）。

## 8. 性能分析 Performance Analysis

- Coverage: Medium
- Inspected evidence: Cargo profile 分层策略（dev/ci 快迭代、release 由 CI env 覆盖开启 fat LTO）、依赖重量（前端运行时仅 3 个包）、线程模型、EntryTable 渲染路径粗查
- Exclusions / limits: 无基准测试与剖析数据，性能结论置信度受此限制

未发现现实瓶颈：解析器均有字节上限，远程 IO 全部带超时并跳转阻塞池。潜在关注点：+page.svelte 的巨型响应式粒度在大库（万级条目）下的重渲染行为未验证；thread-per-connection 见 L-4。

## 9. 测试分析 Testing Analysis（含测试真实性 Testing Authenticity）

- Coverage: High
- Inspected evidence: 14 个前端测试逐文件评估、392 个 Rust 测试分布表、CI runner 配置（cargo-nextest + node --test）
- Exclusions / limits: 未统计行覆盖率百分比

质量高于数量：NIST/RFC 标准向量（AES、HMAC、TOTP）、SRP 与 Kee 4.0.7 兼容往返、并发编辑保存竞态、合并冲突属性测试、脚本注入探针（changelog-script.test.mjs:33-44）、PowerShell 签名脚本对真实临时 Gradle 工程执行（android-signing-script.test.mjs:76-104）。弱区集中在 TS glue 与组件运行时（T-1）。component-contracts.test.mjs 是刻意的实现细节测试，作为跨层契约的唯一守护者可接受，但应在文档中明示其为权宜之计。

### 测试真实性评估 Testing Authenticity Assessment

| 测试域                                     | 真实置信度  | 风险               | 动作                     |
| ------------------------------------------ | ----------- | ------------------ | ------------------------ |
| 解锁/keyfile/错密码（vault/tests.rs）      | High        | —                  | Keep                     |
| 保存/只读降级/并发（vault/tests.rs）       | High        | —                  | Keep                     |
| 加密原语（crypto/*, NIST/RFC 向量）        | High        | —                  | Keep                     |
| bridge/rpc 协议矩阵                        | High        | —                  | Keep                     |
| 合并/同步语义                              | High        | —                  | Keep                     |
| 配置规范化（40 测）                        | High        | —                  | Keep                     |
| 前端纯逻辑 utils（9 文件，注入式协作对象） | Medium-High | UI 集成回归        | Keep                     |
| 发布/签名脚本（含注入探针）                | Medium-High | —                  | Keep                     |
| vault.ts IPC glue                          | Low         | 参数/序列化逃逸    | Add（runtime mock 测试） |
| component-contracts 正则测试               | Low-Medium  | 脆弱但守护跨层契约 | Keep 并标注              |

## 10. 可维护性分析 Maintainability Analysis

- Coverage: High
- Inspected evidence: 行数排行、$effect 清单、JSDoc 覆盖率、命名/导入一致性抽查
- Exclusions / limits: —

除 MA-1（god-component）外前端结构健康：单一 store、runes 局部态、服务层集中。Rust 侧模块划分与文件尺寸控制良好。L-1（vault.ts JSDoc 31%）与 L-2（void invoke）为低成本修补项。

## 11. 设计原则分析 Design / Principles Analysis

### 违反的原则

| 原则           | 违反数 | 严重度 | 影响区域                                                                                    |
| -------------- | ------ | ------ | ------------------------------------------------------------------------------------------- |
| Fail-Fast      | 3      | Low    | remote/mod.rs:81（未知 kind 回退 S3）、webdav.rs:210（吞分支）、version.mjs prettier 非致命 |
| SRP            | 1      | Medium | +page.svelte（四职责聚合）                                                                  |
| CQS / 显式契约 | 1      | Low    | REMOTE_CHANGED 字符串哨兵                                                                   |
| DRY            | 0      | —      | —                                                                                           |

### 遵守良好的原则

- **Fail-fast（主流）**：发布脚本全部 execFileSync + exit(1)；Android 签名脚本 $ErrorActionPreference=Stop 且缺失 env 即 throw；validate-release-version.mjs fail-closed 身份门禁。
- **KISS/YAGNI**：前端运行时依赖仅 3 个包；无多余抽象层。
- **边界防御**：所有网络字节入口有尺寸上限；catch_unwind 防锁污染是有意识的设计。
- **数据不变量**：原子写 + fsync、三阶段 RPC 写、修订守卫，均有对应测试。

## 12. 发布分析 Release Analysis

- Coverage: High
- Inspected evidence: 两条 workflow 逐行、release.mjs 两段式流、版本一致性门禁、Cargo profile 策略
- Exclusions / limits: 未实际执行一次端到端发布演练

发布工程是强项：RELEASE_TAG 强制交叉校验四个版本源、非发布文件脏树拒绝提交、--regenerate 模式限定 force-with-lease、draft release + prerelease 自动识别。缺口集中在产物信任链（R-1 签名/checksum/SBOM）与环境固定（R-2/R-3）。AGENTS.md 所述"本地 release 故意 opt-level=0、CI env 覆盖极端优化"在 workflows 中得到证实，策略自洽。

## 13. 文档分析 Documentation Analysis

### 文档摘要

| 子类           | 数量 | 涉及文档                                                                                                             | 建议动作       |
| -------------- | ---- | -------------------------------------------------------------------------------------------------------------------- | -------------- |
| StaleDocs      | 5    | RELEASE.md:56, TODO.md:50, docs/android.md:26/64, skills/version-release/SKILL.md:164-170, secpivot-dev/SKILL.md:148 | 修正（见 D-1） |
| UserDocs       | 1    | README.md 缺前置依赖（Node/Rust 版本、VS Build Tools/WebView2）与安全报告渠道                                        | 补充           |
| OperatorDocs   | 0    | —                                                                                                                    | —              |
| DeveloperDocs  | 0    | AGENTS.md 与 SKILL.md 命令集和 package.json 完全一致                                                                 | —              |
| ApiDocs        | 0    | browser-integration.md 与实现一致                                                                                    | —              |
| DecisionRecord | 0    | PITFALLS.md 实质承担了决策记录职能且质量高                                                                           | —              |

## 14. 配置安全分析 Configuration Safety Analysis

- Coverage: High
- Inspected evidence: tauri.conf.json CSP dev/prod 分离、capabilities 最小授权（default.json/android.json）、env var fail-closed 处理、feature flags 审查
- Exclusions / limits: Android 实机构建未验证

CSP 严格（default-src 'self'、frame-ancestors 'none'、prod 无远程源），dev CSP 独立且仅 localhost。capabilities 桌面/安卓分别最小化。唯一配置类缺陷为 F-2（kind 静默回退）。无硬编码秘密（Android 签名全部走 env，keystore 仅落 gen/android/ 构建目录且不入库）。

## 15. 可观测性分析 Observability / Operability Analysis

- Coverage: Medium
- Inspected evidence: eprintln 内容审计（仅布尔 presence，无值泄漏）、错误分类消息策略、前端零日志确认
- Exclusions / limits: 桌面单机应用无 metrics/tracing/alerting 面，属形态使然而非缺陷

对桌面产品而言观测需求有限；当前"少日志 + 严格脱敏"的取舍与威胁模型一致。若未来增加远程同步诊断，建议先建统一的 remote 层错误分类（同时解决 F-1）。

## 16. 数据完整性分析 Data Integrity Analysis

- Coverage: High
- Inspected evidence: util.rs 原子写逐行核对、merge 属性测试（tests.rs:8438-8587）、备份保留修剪（backup.rs:50,104）、REMOTE_CHANGED 冲突路径、三阶段写
- Exclusions / limits: 未实测备份恢复演练（建议列入发布前 checklist）

这是项目最强的维度之一：本地写教科书级原子性；远程冲突检测 + 强制覆盖双路径显式化；损坏文件探测（helpers.rs:638-663）非破坏性分类。唯一结构性弱点是 D-1 哨兵契约的脆弱性。

## 17. 隐私 / 数据治理分析 Privacy / Data Governance Analysis

- Coverage: High
- Inspected evidence: 全仓 telemetry/analytics/fetch/console 零命中验证、剪贴板所有权校验擦除、HIBP k-匿名、日志脱敏、零知识边界（主密钥不出会话）
- Exclusions / limits: —

隐私姿态堪称范本：无遥测、无第三方网络调用（除用户主动触发的 HIBP/favicon/WebDAV）、剪贴板清除前校验内容归属（clipboard.ts:38-46）、锁定时清空内存副本。遗留仅 S-1（后端兜底清除）与 breach.rs 副本未擦除。

## 18. 无障碍 / UX 正确性分析 Accessibility / UX Correctness Analysis

- Coverage: Medium
- Inspected evidence: 146 处 aria 属性分布、ModalShell 焦点行为、Escape 处理、错误/加载态抽查
- Exclusions / limits: 未用屏幕阅读器/纯键盘实测走查

aria 覆盖广（EntryDetail 40 处、EntryEditorDialog 37 处），Escape 关闭齐全；核心缺口是 A-1 焦点陷阱。次要：加载态防重复点击未见系统性模式，建议随组件测试一并补。

## 19. 供应链 / 可重现性分析 Supply Chain / Reproducibility Analysis

- Coverage: High
- Inspected evidence: workflow 权限与引用方式、lockfile 提交状态（git ls-files 确认）、工具链固定、产物签名/SBOM、registry 卫生
- Exclusions / limits: 未做 cargo-audit/cargo-deny 全量 CVE 比对（建议纳入 CI）

正面：npm ci 锁定安装、Cargo.lock 入库、无第三方 secrets、release 仅 tag 触发。缺口即 R-1/R-2/R-3 三项。建议追加 cargo-deny 步骤作为持续防线。

## 20. 成本分析 Cost / Resource Economics Analysis

- Coverage: Medium
- Inspected evidence: 外部调用清单（HIBP 可选、favicon 带 20s 超时、WebDAV/S3 用户自有凭据）、后台任务审查
- Exclusions / limits: —

单机桌面应用，无外部付费 API、无遥测存储、无 LLM 开销。favicon 抓取有超时与缓存边界，无失控成本驱动。无发现问题。

## 21. AI / LLM 安全分析 AI / LLM Safety Analysis

- Coverage: Not assessed
- Inspected evidence: 产品代码 rg 检索 prompt/model/llm/completion 等关键词零命中；`.opencode/` 为开发工具链配置，不在产品边界
- Exclusions / limits: 不适用

## 22. Fallback / 防御性代码分析

### Fallback 摘要

| 子类                | 数量 | 保留并告警 | Fail-Fast | 移除 |
| ------------------- | ---- | ---------- | --------- | ---- |
| SilentFallback      | 1    | 0          | 1         | 0    |
| EmptyCatch/EmptyArm | 2    | 1          | 1         | 0    |
| CompatibilityBranch | 0    | —          | —         | —    |
| SilentCorrection    | 1    | 1          | 0         | 0    |
| DefensiveGuess      | 0    | —          | —         | —    |

明细：remote/mod.rs:81（DefensiveGuess→应 fail-fast，见 F-2）；webdav.rs:210（EmptyArm→保留但加日志，见 F-1）；lib.rs:212 `_ => {}`（keepalive 场景，保留合理）；otp.rs:256 counter 默认 0（语义可接受，保留）；serialize.rs/hosts.rs 的 unwrap_or_default 均为可选字段正确语义，非问题。整体 fallback 纪律良好。

## 23. 类型安全分析 Type Safety Analysis

### 摘要

| 子类          | 数量                                                         | Critical | High | Medium | Low |
| ------------- | ------------------------------------------------------------ | -------- | ---- | ------ | --- |
| UnsafeBlock   | ~36（全 Win32 FFI）                                          | 0        | 0    | 0      | 0   |
| TypeAssertion | 0（as any/@ts-ignore/as unknown as 零命中，tsconfig strict） | 0        | 0    | 0      | 0   |
| InputBoundary | 0（网络解析全部返回 Result，字节上限齐备）                   | 0        | 0    | 0      | 0   |
| OutputLeak    | 0（classify_open_error 统一折叠敏感错误）                    | 0        | 0    | 0      | 0   |
| BooleanTrap   | 0                                                            | 0        | 0    | 0      | 0   |
| StringlyTyped | 1（REMOTE_CHANGED 哨兵 + Result<T,String> IPC 契约）         | 0        | 0    | 0      | 1   |
| ErrorType     | 1（同上，String 错误阻碍程序化匹配）                         | 0        | 0    | 0      | 1   |

unsafe 逐处核验：GMEM_ZEROINIT 剪贴板副本、DPAPI 边界由返回长度约束、RegCloseKey 全路径清理——FFI 卫生优秀。

## 24. 前端状态分析 Frontend State Analysis

### 摘要

| 子类               | 数量 | 影响组件                                                       |
| ------------------ | ---- | -------------------------------------------------------------- |
| ComponentSize      | 3    | +page.svelte(3338), EntryDetail(1719), EntryEditorDialog(1719) |
| StateDuplication   | 0    | —                                                              |
| PropDrilling       | 0    | —                                                              |
| EffectChain        | 1    | +page.svelte:485-489/:780-784 面板宽度顺序耦合（有注释自证）   |
| UIBusinessCoupling | 1    | +page.svelte 导入导出编排内联（MA-1 的一部分）                 |
| DOMasState         | 0    | —                                                              |
| RequestState       | 1    | void invoke fire-and-forget 三处（L-2）                        |
| RenderPerf         | 0    | 大库下未验证（见性能分析限制）                                 |

## 25. 后端 API 分析 Backend API Analysis

- Coverage: Medium（Tauri IPC + 两个本地协议，非 REST）
- Inspected evidence: commands 层错误契约、输入校验点、sentinel 匹配、settings 面板直连 invoke 分布
- Exclusions / limits: —

IPC 面一致性好：统一 `Result<T, String>`、命令白名单对齐由 component-contracts 正则守护（:343-348）。改进点：StringlyTyped 错误（D-1）与 settings 面板绕过 services 层（AboutSettingsPanel:26, BridgeSettingsPanel:46-65, RpcSettingsPanel:54-67）宜收敛。

## 26. 依赖重量分析 Dependency Weight Analysis

### 依赖记分板

| 依赖                                                                | 状态    | 用途                                                 | 建议动作                 |
| ------------------------------------------------------------------- | ------- | ---------------------------------------------------- | ------------------------ |
| @tauri-apps/api, plugin-dialog, plugin-opener                       | Healthy | 运行时 IPC/dialog/opener                             | Keep                     |
| image =0.25.8（no default features, png only）                      | Healthy | 图标解码                                             | Keep（feature 纪律范本） |
| reqwest（rustls+blocking, no OpenSSL）                              | Healthy | HIBP/favicon/WebDAV/S3                               | Keep                     |
| 桌面专属 crate（enigo/global-shortcut/keyring/tungstenite/aes-cbc） | Healthy | cfg(any(windows,macos,linux)) 隔离，Android 保持精瘦 | Keep                     |
| windows-sys（显式 feature 列表）                                    | Healthy | Win32 FFI                                            | Keep                     |

无 overweight/unused 依赖。rust-s3 已移除（PITFALLS.md:69）但 TODO.md:50 仍引用（D-1）。

---

## 27. 代码一致性分析 Code Consistency Analysis

- Coverage: Medium
- Inspected evidence: 命名约定抽样（组件 PascalCase / utils camelCase / util 文件 kebab-case）、`$lib/...` 导入组织、错误处理模式（Rust 统一 Result<T,String>、前端 try/catch 分布）
- Exclusions / limits: 未逐文件全量风格比对

一致性整体优秀：无命名漂移、导入路径统一走 `$lib` 别名、Rust 错误处理单一模式贯穿 IPC 边界。轻微不一致：settings 面板绕过 services 层直接 invoke（AboutSettingsPanel:26、BridgeSettingsPanel:46-65、RpcSettingsPanel:54-67），与主体模式偏离；中英文混用（UI 字符串中文、注释英文）为项目有意选择。

## 28. 注释覆盖分析 Comment Coverage Analysis

- Coverage: Medium
- Inspected evidence: vault.ts 导出符号 vs JSDoc 计数（约 90 导出 / 28 JSDoc，31%）、过期注释搜索、高质量注释样本核验
- Exclusions / limits: Rust 侧 rustdoc 覆盖率未逐项统计

现有注释质量高且无过期误导（+page.svelte:486-489、:780-784 对 effect 顺序耦合的自证注释是范本；PITFALLS.md 承担决策记录职能）。主要缺口：vault.ts 作为最大服务层公共 API 文档稀疏（L-1）。Rust 侧模块级 `//!` 文档在 bridge/rpc 协议模块上存在且准确（含协议继承弱点的坦诚标注 rpc/server/mod.rs:13-14）。

## 29. 建议修复顺序 Recommended Fix Order

### 立即修复

无 Critical/High 问题。以下两项因触及用户信任链建议尽快：

| ID  | 问题                                                | 工作量   |
| --- | --------------------------------------------------- | -------- |
| R-2 | CI Actions 固定到 commit SHA                        | 1–2 小时 |
| F-2 | 未知存储 kind fail-fast（一行级修复，防数据误路由） | 15 分钟  |

### 稳定版发布前修复

| ID  | 问题                                     | 工作量   |
| --- | ---------------------------------------- | -------- |
| R-1 | Windows 产物 SHA256SUMS（+评估签名证书） | 半天起   |
| R-3 | rust-toolchain.toml + .nvmrc             | 30 分钟  |
| M-1 | bridge 关联请求限流 + CORS 收敛          | 2–4 小时 |
| A-1 | ModalShell 焦点陷阱                      | 2–4 小时 |
| D-1 | 五处过期文档修正                         | 30 分钟  |

### 稍后安排

| ID              | 问题                                                | 工作量   |
| --------------- | --------------------------------------------------- | -------- |
| MA-1            | +page.svelte 拆分                                   | 2–3 天   |
| T-1             | vault.ts 运行时测试                                 | 1 天     |
| S-1             | 后端剪贴板兜底清除                                  | 3–5 小时 |
| F-1             | webdav 空 arm 加告警                                | 30 分钟  |
| ST-1 / L-4      | expect 降级 / 双查找合并                            | 2 小时   |
| L-1 / L-2 / L-3 | JSDoc 补齐 / void invoke 收口 / prettier 非致命复查 | 半天     |

### 暂时忽略

I-1（ci permissions，低风险）、I-2（中文硬编码，项目语境合理）、thread-per-connection 上限（L-4，loopback 下理论风险）、breach.rs 副本擦除（低价值，顺手修即可）、RenderPerf（无证据前不动）。

## 30. Quick Wins 快赢修复（1–2 小时内移除真实风险）

| 修复                              | 移除的风险           | 工作量  |
| --------------------------------- | -------------------- | ------- |
| rust-toolchain.toml + .nvmrc      | 构建漂移/发布中断    | 30 分钟 |
| 未知 kind 返回 Err                | 配置错误静默连错协议 | 15 分钟 |
| sessions.rs remove-and-check 合并 | 重构引爆点           | 15 分钟 |
| breach.rs 副本 wipe               | 机密卫生例外         | 15 分钟 |
| SHA256SUMS 生成步骤               | 产物完整性不可验证   | 1 小时  |
| webdav.rs:210 加日志              | 同步错误静默         | 30 分钟 |
| 五处文档修正                      | 维护误导             | 30 分钟 |

## 31. 长期重构计划 Long-term Refactor Plan

| 项                   | 动机                                                | 方案                                                          | 风险                                   | 测试策略                                                               |
| -------------------- | --------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------- | ---------------------------------------------------------------------- |
| +page.svelte 模块化  | 唯一系统性前端债务                                  | 抽 io.ts 服务 + layout composable + shell 瘦身，目标 <1000 行 | 回归风险集中于面板宽度 effect 顺序耦合 | 先为 io 编排写 node:test（fake-invoke 模式），重构后契约正则测试保持绿 |
| 错误契约结构化       | REMOTE_CHANGED 哨兵脆弱 + String 错误无法程序化匹配 | IPC 错误升级为 {code,message}，保留字符串兼容期               | 跨层同步改动量大                       | component-contracts 扩展守护 code 枚举；Rust 侧新增错误枚举单测        |
| 剪贴板生命周期后端化 | 前端定时器不可靠                                    | 后端计时清除 + 锁定/退出钩子                                  | 平台差异（Win32 剪贴板所有权）         | platform/clipboard.rs 集成测试                                         |
| CI 增加 cargo-deny   | 供应链持续防线                                      | deny.toml + advisory DB 检查入 ci.yml                         | CI 时长小幅增加                        | cargo-deny 自身即验证                                                  |

---

_本报告由 fuck-my-shit-mountain 技能流程生成。所有严重度/置信度标注遵循技能 rubric；每项发现的证据均来自静态代码审读，未执行动态渗透或性能剖析。_
