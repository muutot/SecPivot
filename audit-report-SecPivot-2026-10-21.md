# Fuck My Shit Mountain Audit Report

**Project:** SecPivot
**Audit mode:** full（全维度）
**Date:** 2026-10-21
**Reviewer:** opencode（minimax/minimax-m3.1-flash）
**Baseline:** `4ab13e4`（tag `v1.6.4`）+ 当前工作区未提交修改（设置页安全区、移动端长按菜单定位）

---

## 1. Executive Summary / 执行摘要

SecPivot 是一个工程质量明显高于同类个人项目的桌面密码管理器：Rust 后端坚持零ize 化密钥生命周期、原子写 + fsync、本地协议仅绑定回环、IPC 凭据掩码、视图纪元（view-epoch）防陈旧回写；前端零遥测、零 console 输出；455 个 Rust 测试 + 178 个前端测试覆盖核心路径。与 2026-08-24 的上一次审计相比，此前报告的高危项**基本都已修复并复核**：GitHub Actions 全部改为 40 位 SHA 固定、`rust-toolchain.toml` 锁定 1.96.0、SHA256SUMS 已随产物发布、bridge 的 `associate` 已加滑动窗口限流、`ModalShell` 已有焦点陷阱与初始焦点管理。

本次全量审计的价值集中在**三条此前未被覆盖的链路上**：① 发布流程的“手动 dispatch 覆盖已发布 release”以及“Android 签名私钥在编译期以明文落盘并注入到所有构建脚本的环境里”；② 移动端 SAF 链路的“原地截断写入导致失败即毁库”“持久授权失败被静默吞掉”“远程对象 key 可逃逸本地镜像目录”；③ 构建脚本的“Gradle 仓库补丁脚本的窗口判定跨块误判”——该脚本在真实模板形态下**自证为无效**，也就是它本来要防的 Android 构建失败仍可能复发。共 20 条发现（3 High / 10 Medium / 7 Low），其中 19 条为逐行确认（Confirmed），1 条为依赖 provider 行为的存疑项（Suspected）。

**真实性复检**同样产出了价值：候选问题中 3 条被证伪并排除（重命名远程配置丢失凭据——实为前端显式提示的设计行为；锁屏未显示可读文件名——实为 subtitle 已显示、路径行有意保留原路径；`config.json` 权限缺失——`store.rs:100` 已有 `set_permissions(0o600)`），另有 2 条确认为上一轮已修复项。所有保留项均给出精确 `file:line` 与可复现的触发路径。

总体评价：**A（良好，7.1/10）**。没有 Critical；3 条 High 全部集中在发布与签名链路，均可用局部改动修复，不需要重构。

### Score Dashboard / 评分仪表

```
Security        ███████░░░  7.0  A   签名私钥在编译期明文注入构建环境；Windows 产物未签名；远程 key 可逃逸镜像目录
Stability       ███████░░░  7.0  A   SAF 原地截断写入失败即毁库；下载/本地读取无上限；OS RNG 失败即 panic
Performance     ████████░░  8.0  A   未发现热点瓶颈；依赖极轻；仅存在无上限读取这一类边界问题
Testing         ████████░░  7.5  A   455 Rust + 178 前端测试；前端 IPC 胶水层仍缺运行时测试，主 CI 无 Android 编译
Maintainability ███████░░░  7.0  A   +page.svelte 仍 2620 行 / 68 个顶层函数；脚本与文档多处口径漂移
Design          ████████░░  7.5  A   fail-fast、原子写、视图纪元执行到位；SRP/DRY 在前端巨型组件上失守
Release         ██████░░░░  5.5  B   手动 dispatch 可覆盖已发布产物；verify 不阻塞 Windows draft；regenerate 流程死锁
─────────────────────────────────────
Overall         ███████░░░  7.1  A
```

每个维度评分 0.0–10.0。**分数越高越好**（10 = 干净，0 = 粪山）。评分为判断性评分，非机械扣分；Release 维度的低分主要由 3 条 High 级发布完整性问题拉低。

### Finding Statistics / 发现统计

| Severity  | Count  | Confirmed | Suspected |
| --------- | ------ | --------- | --------- |
| Critical  | 0      | 0         | 0         |
| High      | 3      | 3         | 0         |
| Medium    | 10     | 9         | 1         |
| Low       | 7      | 7         | 0         |
| Info      | 0      | 0         | 0         |
| **Total** | **20** | **19**    | **1**     |

## 2. Project Map / 项目地图

**架构分层**

- **Rust 后端 `src-tauri/src/`**：`vault/`（会话、持久化、合并、条目、附件临时区）、`crypto/`（AES/HMAC/TOTP/KDF 原语）、`bridge/`（KeePassHttp 本地 HTTP 服务）、`rpc/`（KeePassRPC WebSocket + SRP-6a）、`commands/`（Tauri IPC 层）、`remote/`（S3 / WebDAV 传输 + 本地镜像与备份轮转）、`platform/`（Win32 FFI：剪贴板 / DPAPI / auto-type / 护盾）、`config/`（配置 schema 与归一化）、`net.rs`（reqwest TLS 信任根统一入口）。
- **Android 插件 `src-tauri/plugins/saf/`**：本地 Tauri 插件，Kotlin 侧实现 `ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT` + `takePersistableUriPermission`，Rust 侧 `open_document` / `create_document`。
- **前端 `src/lib` + `src/routes`**：Svelte 5 runes；`services/vault.ts`（约 1700 行，集中承载 IPC 调用）、`services/settings.ts`（约 1360 行）、`routes/+page.svelte`（应用壳，2620 行 / 68 个顶层函数）、`components/settings/*`（设置面板）。
- **入口与初始化**：`src-tauri/src/main.rs` → Tauri Builder 注册命令、插件与 `setup` 钩子；前端 SvelteKit adapter-static SPA，路由 `/`（主界面）与 `/settings`（设置页）。
- **状态归属**：数据库与密钥仅存在于 Rust `VaultSessions`；前端只持有 UI 态与脱敏配置快照（`get_masked`）。
- **持久化**：本地 `.kdbx` 走 `util::atomic_write`（tmp + fsync + rename）；远程走 S3/WebDAV + 本地镜像与时间戳备份轮转；`config.json` 单独 `set_permissions(0o600)`。
- **外部接口**：回环 HTTP（bridge）、回环 WS（rpc）、Tauri IPC、HIBP HTTPS、favicon 抓取、S3/WebDAV、Android SAF。
- **安全边界**：主密钥/密码只在内存会话内并 zeroize；关联审批需用户显式确认且已有限流；CSP 严格；capabilities 按平台最小授权。
- **AI / LLM 表面**：产品代码无 LLM / prompt / tool 调用面（`.opencode/` 仅为工具链配置）→ AI-Safety 维度不适用。
- **高风险区（本次重点）**：`release.yml` 的发布身份与产物链路、`configure-android-signing.ps1` + `tauri android build` 的签名注入、`vault/file_io.rs` 的 SAF 写入、`remote/backup.rs` 的本地镜像命名、`scripts/patch-android-gradle-repos.ps1` 的仓库补丁。

### Coverage Matrix / 覆盖矩阵

| Dimension            | Coverage     | Evidence inspected                                                                              | Exclusions / limits                                  |
| -------------------- | ------------ | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| Architecture         | High         | 全目录树、`invoke` 调用分布、状态归属、插件注册（`lib.rs` / `plugins/saf`）                     | 未做运行时调用图追踪                                 |
| Security             | High         | 密钥生命周期、IPC 掩码、bridge/rpc 认证与限流、TLS 根、SAF 授权、capabilities、release 产物签名 | 未做动态渗透测试                                     |
| Stability            | High         | 原子写实现、SAF 写入路径、下载/读取边界、panic 路径、rebase/发布脚本状态机                      | 未做长时间压力运行                                   |
| Performance          | Medium       | 依赖重量、虚拟列表、下载与解析路径、限流与并发模型                                              | 无性能基准或 profiling 数据                          |
| Testing              | High         | 26 个前端测试文件 + Rust 测试分布、CI 工作流、契约脚本                                          | 未统计覆盖率百分比                                   |
| Maintainability      | High         | 文件行数排行、顶层函数计数、脚本与文档一致性抽查                                                | 未逐文件比对全部命名                                 |
| Design               | High         | principles 逐条对照、失败即失败原则、原子性、视图纪元                                           | —                                                    |
| Release              | High         | `release.yml` 逐行、6 个 `scripts/*.mjs`、2 个 `*.ps1`、实际 release run 结论与产物清单         | 未执行一次完整发布演练                               |
| Documentation        | High         | `docs/*`、README、AGENTS、skills references、TODO 与代码交叉验证                                | 未审查历史 CHANGELOG 全文                            |
| Configuration        | High         | `tauri.conf.json`、capabilities、Rust/TS 双端 defaults 与归一化、版本源                         | Android 真机行为未验证                               |
| Observability        | Medium       | 日志语句与错误分类审查                                                                          | 桌面应用无 metrics / alerting                        |
| Data Integrity       | High         | 原子写、备份轮转、合并不变量、SAF 写入、镜像落盘                                                | 备份恢复未实测演练                                   |
| Privacy              | High         | 临时附件、配置权限、遥测搜索、日志脱敏、IPC 掩码                                                | —                                                    |
| Accessibility        | Medium       | 语义标签、焦点管理、窄屏布局、错误态                                                            | 未用屏幕阅读器实测                                   |
| Supply Chain         | High         | action 固定、工具链锁定、签名与校验和、密钥注入路径                                             | 依赖 CVE 未全量比对                                  |
| Cost                 | Medium       | 外部请求清单、后台任务、存储增长点                                                              | 单机桌面应用，成本风险低                             |
| AI-Safety            | Not assessed | 产品代码无 LLM / prompt / tool 面                                                               | 不适用                                               |
| Fallback             | High         | `unwrap_or_default` / 静默 match 分支 / 吞错点全量扫描                                          | —                                                    |
| Testing Authenticity | High         | 逐文件评估断言对象（行为 vs 实现细节 vs 正则）                                                  | —                                                    |
| Type Safety          | High         | `as any` / `@ts-ignore` 零命中、tsconfig strict、Rust `Result` 一致性                           | —                                                    |
| Frontend State       | Medium       | 组件行数排行、store 数量、`$effect` 清单、上下文菜单所有者模型                                  | 前端全量 diff 审计因工具端点故障未完成，仅精读改动面 |
| Backend API          | Medium       | IPC 命令错误契约、输入校验、sentinel 契约                                                       | IPC 非传统 REST API                                  |
| Dependency Weight    | High         | `package.json` / `Cargo.toml` 依赖清单与 feature 纪律                                           | 未逐包统计传递体                                     |
| Code Consistency     | Medium       | 命名约定、导入组织、脚本风格抽查                                                                | 未逐文件比对                                         |
| Comment Coverage     | Medium       | 关键模块注释密度、过期注释抽查                                                                  | —                                                    |

**覆盖说明**：本次审计覆盖一方源码（Rust 76 / Svelte 60 / TS 49 / JS 40 / 文档 69 个文件）、26 个前端测试文件、2 个工作流与 9 个脚本；排除 `.git`、`node_modules`、`src-tauri/target`、`src-tauri/gen`、图标二进制与 lockfile 全文。所有结论基于静态证据与一次真实 release run 的 job 结论，未执行动态渗透、模糊测试或性能剖析。审计期间 2 个并行审计子代理因工具端点不可用失败，相应维度（前端状态、Android/网络）改为主代理精读改动面 + 关键路径复核，覆盖度记为 Medium 并在此明示。

### 问题真实性复检 / Re-verification of candidate issues

审计过程中提出的候选问题在写入报告前逐条回到源码复核，以下候选**被证伪或降级并排除**：

| 候选问题                                                        | 复核结论                                    | 依据                                                                                                                                                                                      |
| --------------------------------------------------------------- | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 重命名远程配置会丢失已存凭据                                    | **排除（设计行为）**                        | `src/lib/services/settings.ts` 的 `renameRemoteProfile` 显式清除 `secretPresence` 并提示 `remote.renamedSecretsReset`；后端 `store.rs:201-215` 的 placeholder-keep 只按路径匹配是既定语义 |
| 锁屏未显示 `content://` 可读文件名                              | **排除（误报）**                            | `LockScreen.svelte:106` 已把 `remembered.fileName` 作为 subtitle 渲染；`:138-140` 显示原始路径是有意设计（路径即身份，且 tooltip 保留完整 URI）                                           |
| `config.json` 缺少所有者权限限制                                | **排除（已修复）**                          | `config/store.rs:97-102` 写入后显式 `set_permissions(0o600)`，`config/tests.rs:1173` 有断言                                                                                               |
| CI action 未按 SHA 固定 / 缺少 Rust 工具链锁定 / 缺少产物校验和 | **排除（已修复）**                          | 全部 action 为 40 位 SHA；`rust-toolchain.toml` 锁定 1.96.0；`release.yml:201-226` 生成并上传 SHA256SUMS                                                                                  |
| bridge `Access-Control-Allow-Origin: *` 且关联请求无速率限制    | **降级（限流已修复，CORS 保留为设计取舍）** | `bridge/server.rs:74-95` 存在 `AssociateRateLimiter`，`:249` 对 `associate` 生效                                                                                                          |
| `ModalShell` 无焦点陷阱                                         | **排除（已修复）**                          | `ModalShell.svelte:66-109` 实现焦点捕获、初始焦点与关闭后焦点归还                                                                                                                         |
| 主 CI 缺少 Android 编译门禁                                     | **记录为建议（Low）**                       | `ci.yml` 仅在 push/PR 跑 Windows/Linux/前端；Android 只在 tag 触发的 release 中编译（见“建议修复顺序 · Schedule Later”）                                                                  |

## 3. Top Risks / 重点风险

1. **[F-1] 手动 dispatch 可用非标签提交的产物覆盖已发布 release** — High / Release — `validate` 只比对版本字符串，不校验 HEAD 是否等于 `v<version>` 所指提交；dispatch 后 `tauri-action` 按 `tagName` 命中既有 release，`gh release upload --clobber` 直接覆盖 NSIS、便携 ZIP 与 SHA256SUMS。
2. **[F-2] Android 签名私钥在编译期明文落盘并注入全部构建脚本环境** — High / Security — `configure-android-signing.ps1:48` 把 JKS 写进工作区，`release.yml:344-353` 让 `npm`/Gradle/`cargo build.rs` 在持有口令的环境里运行。
3. **[F-5] Android SAF 原地截断写入，失败即毁库** — High / Stability — `vault/file_io.rs:82-98` 以 `truncate(true)` + `write_all` 直写目标文档，无备份、无原子替换。
4. **[F-6] 远程对象 key 可在 Windows 上逃逸本地镜像目录** — Medium / Security — `remote/backup.rs:18-20,89` 的 basename 拆分不处理反斜杠与绝对路径。
5. **[F-12] Gradle 仓库补丁脚本在真实模板形态下自证无效** — Medium / Release — 1200 字符窗口跨越到下一个 `repositories` 块，误判“已包含 google()/mavenCentral()”（已本地复现）。
6. **[F-9] `release.mjs --regenerate` 在 RELEASE.md 门禁之前改写历史** — Medium / Release — rebase + 删标签先于门禁执行，门禁必然失败，留下改写过的分支与被删的本地标签。
7. **[F-18] `verify` 不阻塞 Windows 产物进入 draft release** — Medium / Release — `build` 只 `needs: validate`，红测试仍会产出可发布的安装包。
8. **[F-7] 解密后的附件临时文件按默认权限落盘** — Medium / Privacy — `vault/temp_attachments.rs:40,57` 未设置 0700/0600。
9. **[F-17] 持有 `contents: write` 的构建 job 保留并导出写权限令牌** — Medium / Security — `release.yml:101-116,158-160`。
10. **[F-8] 远程下载与本地读取均无体积上限** — Medium / Stability — `s3.rs:266`、`webdav.rs:178` 直接 `.bytes()`；`file_io.rs:35-40` 直接 `fs::read`。
11. **[F-3] 正式 Windows 产物要求 AVX2** — Medium / Release — `release.yml:110` 的 `target-cpu=x86-64-v3`。
12. **[F-4] Windows 产物未签名且无来源证明** — Medium / Release — 无 `signtool` / attestation 步骤。
13. **[F-19] SAF 持久授权失败被静默吞掉，重启后重开必然失败** — Medium / Stability（Suspected）— `SafPlugin.kt:90-92` 捕获 `SecurityException` 后不回报状态。
14. **[F-13] `package-lock.json` 版本号长期停留在 1.4.0** — Low / Configuration。
15. **[F-20] `+page.svelte` 仍是 2620 行 / 68 个顶层函数的巨型组件** — Low / Maintainability。

## 4. Detailed Findings / 详细发现

### Finding: 手动 dispatch 可用非标签提交的产物覆盖已发布 release

- Severity: High
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `.github/workflows/release.yml`、`scripts/validate-release-version.mjs`
- Evidence:
  - File: `.github/workflows/release.yml:7-12,33,39-51,114-116,158-166,195,222`
  - File: `scripts/validate-release-version.mjs:29-62`
  - Function / Module: `validate` job / `validateReleaseIdentity` / `tauri-action` / `gh release upload --clobber`
  - Relevant behavior: `workflow_dispatch` 接受任意 `version` 字符串；`validate` 只比较 `package.json` / `tauri.conf.json` / `Cargo.toml` 三处版本并要求 `RELEASE_TAG == v<version>`，**从不校验当前 HEAD 是否就是该标签指向的提交**；`build` job 以 dispatch 选中的 ref（默认 `main`）checkout，`tauri-action` 用 `tagName: v<version>` 解析 release，`gh release upload ... --clobber` 覆盖同名资产。
  - Trigger: v1.6.4 已发布后，`main` 的 `package.json` 仍是 1.6.4；此时在 Actions 手动 dispatch Release，version 填 `1.6.4`、ref 选 `main`，校验通过，构建出的二进制来自 `main` 上未发布的提交。
- Problem: 发布身份校验缺少“提交即产物来源”的约束，同名资产的覆盖也没有任何一致性检查。
- Why it matters: 密码管理器的分发链路一旦允许“同名不同物”，用户下载到的安装包与标签、`RELEASE.md`、校验和说明全部失去对应关系；SHA256SUMS 反而为替换后的字节背书。
- Realistic failure scenario: 1.6.5 开发期间误触发 1.6.4 的重跑 → 1.6.4 的 NSIS 与便携 ZIP 被 1.6.5 开发态二进制覆盖，校验和同步更新，外部用户无任何提示地收到错误产物。
- Minimal fix: 在 `validate` 中比对 `git rev-parse HEAD` 与 `git rev-list -n 1 v$VERSION`（或强制 `github.ref_type == 'tag'`），并在 `tauri-action` 显式设置 `targetCommitish` 为该标签提交。
- Better long-term fix: 把“版本 + 标签 + 提交 + 产物哈希”做成一次性的 release manifest 并在发布后校验，dispatch 只允许重跑同一提交的构建。
- Regression test suggestion: 构造“HEAD 比标签多一个提交”的临时仓库，断言 `validateReleaseIdentity` / `validate` 失败退出。
- Estimated effort: 1–2 小时

### Finding: Android 签名私钥在编译期明文落盘并注入全部构建脚本环境

- Severity: High
- Confidence: High
- Category: Security
- Status: Confirmed
- Affected area: `scripts/configure-android-signing.ps1`、`.github/workflows/release.yml` 的 `android` job
- Evidence:
  - File: `scripts/configure-android-signing.ps1:31-32,48`
  - File: `.github/workflows/release.yml:335-353`
  - Function / Module: `Configure Android release signing` / `Build Android APKs (release, per-ABI)`
  - Relevant behavior: 脚本把 base64 解码后的 JKS 直接写到工作区内的 `src-tauri/gen/android/secpivot-release.jks`；随后的 `npx tauri android build` 在 `ANDROID_KEYSTORE_PASSWORD` / `ANDROID_KEY_PASSWORD` / `ANDROID_KEY_ALIAS` 三个环境变量存在的环境里执行，该命令会运行 `npm` 生命周期脚本、Gradle 插件与所有 crate 的 `build.rs`。
  - Trigger: 任意一个被引入的 npm 依赖、Gradle 插件或 crate 的 `build.rs` 读取工作区文件或环境变量。
- Problem: 发布签名身份与第三方构建代码处在同一信任域，且以明文形式暴露。
- Why it matters: 泄露的是**永久性**签名身份：攻击者可用它签出设备同样信任的更新包，用户侧不会出现任何异常提示；这比单个版本产物被替换严重一个量级。
- Realistic failure scenario: 新引入的某个 Gradle 插件在配置阶段读取 `secpivot-release.jks` 与 `ANDROID_KEYSTORE_PASSWORD` 环境变量并外传 → 攻击者随后发布带后门的“更新”，覆盖安装后获得用户全部凭据。
- Minimal fix: 改为“先构建未签名 APK，再在独立 job 中用 `apksigner` 签名”，签名 job 只做签名与校验，不执行任何第三方构建代码。
- Better long-term fix: 使用 Android App Signing（Google 托管签名密钥），仓库内永不出现私钥；或使用短时���凭据 + 硬件密钥。
- Regression test suggestion: 在 CI 的构建 job 中放置一个探针 `build.rs` / Gradle 脚本，断言其无法读取 `gen/android/*.jks` 且环境中不存在 `ANDROID_KEYSTORE_PASSWORD`。
- Estimated effort: 半天

### Finding: Android SAF 原地截断写入，失败即毁库

- Severity: High
- Confidence: High
- Category: Stability
- Status: Confirmed
- Affected area: `src-tauri/src/vault/file_io.rs`
- Evidence:
  - File: `src-tauri/src/vault/file_io.rs:82-98`（`write_uri`），对照 `:44-53`（`write` 的桌面分支走 `util::atomic_write`）
  - Function / Module: `write_uri` / `write`
  - Relevant behavior: 移动端以 `OpenOptions::write(true).truncate(true)` 打开 `content://` 文档并 `write_all(bytes)`，失败仅返回错误字符串；文档被截断后没有回滚、没有备份、SAF 也不提供 rename。
  - Trigger: 写入过程中 provider 抛错、存储空间不足、应用被杀、用户同时通过其他应用改写该文档。
- Problem: 桌面路径有“临时文件 + fsync + rename”的原子替换，SAF 路径没有等价的原子性，却复用了同一套“保存成功/失败”的 UI 语义。
- Why it matters: 移动端被删改的文档往往是用户唯一的库副本；一次写失败就把密文库变成空文件或半截文件，而 UI 只提示“写入数据库失败”。
- Realistic failure scenario: 用户在手机端保存修改，provider 因空间不足返回错误 → 文档已被截断 → 库文件损坏 → 用户重开时解析失败，原始数据不可恢复（且没有本地镜像兜底）。
- Minimal fix: 写前先在本地镜像目录保留一份滚动备份；或先写入 `ACTION_CREATE_DOCUMENT` 创建的新文档、校验后再覆盖原文档。
- Better long-term fix: 抽象出“文档写入器”接口，桌面用原子替换、移动用“新文档 + 校验 + 覆盖”两阶段策略，并在 UI 上区分“已写入但未提交”与“保存失败”。
- Regression test suggestion: 为 `write_uri` 增加可注入的失败注入（写入中途报错），断言原文档字节未被破坏，或断言已生成可回滚的本地副本。
- Estimated effort: 1–2 天

### Finding: 远程对象 key 可在 Windows 上逃逸本地镜像目录

- Severity: Medium
- Confidence: High
- Category: Security
- Status: Confirmed
- Affected area: `src-tauri/src/remote/backup.rs`、`src-tauri/src/vault/persist.rs`
- Evidence:
  - File: `src-tauri/src/remote/backup.rs:9-17`（`validate_remote_key` 只 trim 并去前导 `/`）、`:18-21`（`remote_key_basename` 仅按 `/` 切分）、`:89`（`dir.join(name)`）
  - File: `src-tauri/src/vault/persist.rs:192-198,235-241,308-311,371-374`（`write_local_copy(local_dir, &remote_key_basename(&key), …)`）
  - Function / Module: `validate_remote_key` / `remote_key_basename` / `write_local_copy`
  - Relevant behavior: 服务器列出的对象 key 直接决定本地镜像文件名；在 Windows 上 `\` 是路径分隔符，`dir.join("..\\..\\evil.kdbx")` 会解析到镜像目录之外，`join` 遇到绝对路径还会整体替换目标路径。
  - Trigger: 被入侵或恶意的 S3/WebDAV 端点返回形如 `x/..\..\..\Users\Public\evil.kdbx` 的 key，用户以“SaveLocal”模式打开。
- Problem: 远端可控字符串未经净化即参与本地路径构造，构成目录穿越写。
- Why it matters: 攻击者可用自己控制的内容在用户机器上任意位置创建文件（内容是下载到的 KDBX 字节）；在 Windows 上还可能覆盖既有文件。
- Realistic failure scenario: 用户的 S3 桶被他人写权限滥用，列表中出现 `a/..\..\Startup\evil.exe`；App 下载并把该字节写到用户 Startup 目录，重启即执行。
- Minimal fix: 对 basename 做白名单净化（拒绝 `\`、`..`、盘符、绝对路径、分隔符），并对最终路径做 `starts_with(local_dir)` 断言后再写。
- Better long-term fix: 镜像文件名统一用“key 的 SHA-256 摘要 + 扩展名”，显示名与文件名解耦。
- Regression test suggestion: 单元测试 `remote_key_basename`/`write_local_copy` 传入 `x/..\..\evil` 与 `x/C:\evil`，断言最终路径仍在 `local_dir` 内。
- Estimated effort: 2–4 小时

### Finding: Gradle 仓库补丁脚本在真实模板形态下自证无效

- Severity: Medium
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `scripts/patch-android-gradle-repos.ps1`
- Evidence:
  - File: `scripts/patch-android-gradle-repos.ps1:41-43`（`$windowLength = Min(1200, …)`；`$missing` 在块之后 1200 字符窗口内检索）
  - File: `scripts/patch-android-gradle-repos.ps1:88-95`（`pluginManagement` 与 `dependencyResolutionManagement` 分别匹配）
  - Function / Module: `Add-MissingReposToBlock` / `Ensure-ReposInFile`
  - Relevant behavior: “某块是否已包含 google()/mavenCentral()”的判定使用块之后的固定字符窗口，不做括号配对；Tauri 生成的 `settings.gradle.kts` 恰好是 `pluginManagement { repositories { gradlePluginPortal() } }` 紧跟 `dependencyResolutionManagement { repositories { google(); mavenCentral() } }` 的形态。
  - Trigger: 在该真实形态上运行脚本（本次已用临时 fixture 复现）。
- Problem: 窗口跨界取到了下一个块里的 `google()` / `mavenCentral()`，于是判定“无需修改”并原样保留 `pluginManagement` 块。
- Why it matters: 该脚本存在的唯一目的就是防止 “Kotlin 插件只能从 plugins.gradle.org 解析、父 BOM 缺失” 的 Android 构建失败；它在这个失败场景下恰好不生效，CI 会以 Gradle 的晦涩错误失败，发布流程被拖垮。
- Realistic failure scenario: 插件镜像再次出现父 BOM 不同步 → 脚本报告 “No change needed for settings pluginManagement” → `tauri android build` 在 `:buildSrc` 配置阶段失败，Android release 中断。
- Minimal fix: 改为按大括号配对提取块体内容（逐字符计数），只在块内判定缺失项。
- Better long-term fix: 不再对生成文件做字符串补丁，改为在 `tauri android init` 后写入仓库自带的 `settings.gradle.kts` 模板（版本化、可测试）。
- Regression test suggestion: 把本次复现用的双块 fixture 固化为脚本测试，断言 `pluginManagement` 块内被补入 `google()` 与 `mavenCentral()`。
- Estimated effort: 2–3 小时

### Finding: `release.mjs --regenerate` 在 RELEASE.md 门禁之前改写历史

- Severity: Medium
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `scripts/release.mjs`
- Evidence:
  - File: `scripts/release.mjs:126-179`（`if (isRegenerate) { … }`，其中 `:171` 执行 `git rebase --onto`、`:172` 执行 `git tag -d`）
  - File: `scripts/release.mjs:227-236`（`if (!checkReleaseMd(currentVersion)) { … exit(2) }`）
  - Function / Module: 主流程 / regenerate 分支
  - Relevant behavior: rebase 与删标签发生在门禁之前。工作树干净时，rebase 会把旧发布提交从历史里摘掉并删掉本地标签，随后步骤 1 重新改版本号、步骤 3 检查 `RELEASE.md` 首个标题——此时 `RELEASE.md` 已随 rebase 回到上一版本，门禁必然失败并 `exit(2)`；若事先已按流程写好 `RELEASE.md`（工作树变脏），`git rebase` 自身会因“存在未暂存改动”直接失败。
- Problem: regenerate 路径在两种工作树状态下都走不通，且失败后没有文档化的恢复动作。
- Why it matters: 这是修复“已发布版本发布说明写错”的唯一官方路径；它失败意味着用户要么强推历史，要么手改标签，风险不可控。
- Realistic failure scenario: 用户执行 `node scripts/release.mjs --regenerate 1.6.4` 修正发布说明 → 脚本删掉本地 `v1.6.4` 标签并 rebase 分支 → 门禁失败退出 → 本地处于“标签已删、版本号已改、RELEASE.md 是旧版”的中间态，用户不知道该 `git reset --hard origin/main` 还是继续。
- Minimal fix: 把 `RELEASE.md` 标题校验前移到 rebase 之前；rebase 时用 `git stash` 暂存被编辑的 `RELEASE.md` 再恢复。
- Better long-term fix: regenerate 改为“新建发布提交 + 移动标签”的非历史改写模型（`git reset --soft` + 重新 commit），彻底避免 rebase。
- Regression test suggestion: 在 `tests/release-script.test.mjs` 中断言“标题校验调用出现在 `git rebase` 之前”，并构造脏工作树的 regenerate 用例。
- Estimated effort: 3–5 小时

### Finding: `verify` 不阻塞 Windows 产物进入 draft release

- Severity: Medium
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `.github/workflows/release.yml`
- Evidence:
  - File: `.github/workflows/release.yml:96-98`（`build:` 的 `needs: validate`）
  - File: `.github/workflows/release.yml:452-464`（`publish-android` 的 `needs: [verify, build, android]`）
  - Function / Module: job 依赖图
  - Relevant behavior: `build` job 只依赖 `validate`，因此测试全红时 NSIS 安装包、便携 ZIP 与 SHA256SUMS 仍会产出并进入 draft release；只有 Android 上传链路受 `verify` 约束。
- Problem: 发布产物的验证门禁只覆盖了 Android 一侧。
- Why it matters: 流程依赖“人工发布 draft”这一最后动作；人工通常只看界面状态，容易在红 run 的情况下把草稿发布出去。
- Realistic failure scenario: `npm run verify` 因新增 Rust 测试失败而红，但 draft release 里已有完整安装包；维护者看到 job 失败却在 Releases 页面看到“资产齐全”，直接发布 → 用户拿到未通过测试的二进制。
- Minimal fix: `build` 改为 `needs: [validate, verify]`，或给上传步骤加 `if: needs.verify.result == 'success'`。
- Better long-term fix: 让 draft release 的创建本身成为 `verify` 的后置动作，使“产物存在”与“验证通过”成为同一状态。
- Regression test suggestion: 增加工作流结构测试，断言 `build.needs` 包含 `verify`。
- Estimated effort: 30 分钟

### Finding: 持有 `contents: write` 的构建 job 保留并导出写权限令牌

- Severity: Medium
- Confidence: High
- Category: Security
- Status: Confirmed
- Affected area: `.github/workflows/release.yml` 的 `build` job
- Evidence:
  - File: `.github/workflows/release.yml:101-102`（job 级 `permissions: contents: write`）、`:114-116`（`actions/checkout` 未设 `persist-credentials: false`）、`:158-160`（`GITHUB_TOKEN` 注入 `tauri-action` 步骤）
  - Function / Module: build job 权限与 checkout 配置
  - Relevant behavior: checkout 默认把令牌持久化进 `.git/config`；同一 job 内还会执行 `npm ci`、Rust 编译与第三方构建脚本，令牌同时通过环境变量暴露给 `tauri-action` 步骤。
- Problem: 写权限令牌与不可信构建代码共存于同一 job。
- Why it matters: 任意被投毒的构建依赖都能读取令牌、创建标签或覆盖 release 资产，令攻击者可直接接管分发渠道。
- Realistic failure scenario: 某次 `npm ci` 拉到的传递依赖被投毒 → `postinstall` 读取 `.git/config` 中的令牌 → 推送伪造标签或覆盖正式 release 资产。
- Minimal fix: `actions/checkout` 增加 `persist-credentials: false`；构建 job 降为 `contents: read`，发布动作单独放到 `contents: write` 的发布 job。
- Better long-term fix: 全面采用“只读构建 → 上传产物 → 独立发布 job”的三段式（Android 已是此模式，Windows 应对齐）。
- Regression test suggestion: 工作流静态测试，断言所有 `contents: write` job 均无 `npm ci` 且 checkout 关闭了凭据持久化。
- Estimated effort: 1 小时

### Finding: 解密后的附件临时文件按默认权限落盘

- Severity: Medium
- Confidence: High
- Category: Privacy
- Status: Confirmed
- Affected area: `src-tauri/src/vault/temp_attachments.rs`
- Evidence:
  - File: `src-tauri/src/vault/temp_attachments.rs:40`（`std::fs::create_dir_all(&dir)`）、`:57`（`std::fs::write(&path, data)`）
  - Function / Module: `AttachmentTempStore::create`
  - Relevant behavior: 目录与文件均以进程默认 umask 创建（Linux/macOS 上通常是 0755 / 0644），而这里写的是**已解密的附件明文**；清理只在 discard / 锁 / 关闭 / 陈旧清扫时发生。
- Problem: 明文附件在共享临时目录中以其他本地用户可读的方式存在。
- Why it matters: 密码库本体是加密的，但导出的附件是明文；同机其他账号或进程可以在清扫窗口内读取。
- Realistic failure scenario: 用户在 Linux 桌面版点击“打开附件”预览 PDF → `/tmp/secpivot-attachments/<uuid>/invoice.pdf` 以 0644 存在数分钟，同机低权限账号可读取该发票。
- Minimal fix: 目录用 `set_permissions(0o700)`、文件用 `0o600`（Unix 分支），Windows 保持现状。
- Better long-term fix: 使用 `mkdtemp` 语义的私有临时根目录，并把解密内容限制在内存中按需流式写出。
- Regression test suggestion: Unix 下的单元测试断言目录权限为 0700、文件权限为 0600。
- Estimated effort: 1–2 小时

### Finding: 远程下载与本地读取均无体积上限

- Severity: Medium
- Confidence: High
- Category: Stability
- Status: Confirmed
- Affected area: `src-tauri/src/remote/s3.rs`、`src-tauri/src/remote/webdav.rs`、`src-tauri/src/vault/file_io.rs`
- Evidence:
  - File: `src-tauri/src/remote/s3.rs:256-268`（`get_with_etag` → `response.bytes()`）
  - File: `src-tauri/src/remote/webdav.rs:161-180`（同样的 `.bytes()`）
  - File: `src-tauri/src/vault/file_io.rs:35-40`（`std::fs::read`）、`src-tauri/src/vault/persist.rs:138`
  - Function / Module: `S3Storage::get_with_etag` / `WebDavStorage::get_with_etag` / `file_io::read`
  - Relevant behavior: 只有 120 秒总超时（`remote/mod.rs:42`）与 15 秒连接超时，没有体积上限；本地读取同样一次性读入内存。
  - Trigger: 远端返回超大 `Content-Length` 或持续流式输出的端点；本地指向异常巨大的文件。
- Problem: 超时只限制时间，不限制内存占用。
- Why it matters: 一次恶意或失控的响应就能让进程 OOM 崩溃，用户侧表现为应用无提示退出（可能发生在保存路径上）。
- Realistic failure scenario: 用户指向一个被篡改的 S3 兼容端点，它在 120 秒内持续推送数据 → Rust 侧 `bytes()` 持续扩容 → 进程被 OOM killer 终止，数据库会话丢失。
- Minimal fix: 先读 `Content-Length` 并与上限比较，超限直接报错；流式读取时按上限截断。
- Better long-term fix: 统一 `read_capped(path, max)` 工具函数，密钥文件（已实现 10 MiB 上限）与数据库文件共用同一上限策略。
- Regression test suggestion: 用本地测试服务器返回超限 `Content-Length`，断言返回“超过上限”错误而不是分配内存。
- Estimated effort: 3–5 小时

### Finding: SAF 持久授权失败被静默吞掉，重启后重开必然失败

- Severity: Medium
- Confidence: Medium
- Category: Stability
- Status: Suspected
- Affected area: `src-tauri/plugins/saf/android/.../SafPlugin.kt`、`src/lib/services/vault.ts`
- Evidence:
  - File: `src-tauri/plugins/saf/android/src/main/java/com/secpivot/saf/SafPlugin.kt:88-93`（`takePersistableUriPermission` 失败仅 `catch (_: SecurityException)`）
  - File: `src/lib/services/vault.ts:483-491`（`rememberRecent` 在 Android 上把 `content://` 路径写入最近文件）
  - Function / Module: `finishPick` / `rememberRecent`
  - Relevant behavior: 授权失败时插件仍返回 URI，前端照常把该 URI 写入最近文件；下次启动时该 URI 的读授权已失效，`file_io::read_uri` 只能拿到 provider 的原始错误。
  - Trigger: 只授予会话级授权的 provider，或 provider 在应用重启后撤销授权。
- Problem: “能否跨重启重开”这一关键属性被当作成功处理，失败信号没有回传给调用方。
- Why it matters: 这正是最初“重启后打不开最近库”现象的一类成因；用户看到的是一条 provider 原始错误，而不是“该文档的授权已失效，请重新选择”。
- Realistic failure scenario: 用户在 Android 打开某库并长按记住 → 重启应用 → 点击最近记录 → 弹出“无法读取数据库文件: SecurityException”这类无法理解的报错。
- Minimal fix: 插件在 `takePersistableUriPermission` 失败时回传 `persisted: false`，前端据此不把该 URI 写入最近文件（或写入但标注“需重新授权”）。
- Better long-term fix: 打开时先做一次可读性探测（读 1 字节），失败即从最近列表移除并提示。
- Regression test suggestion: 为 Rust 侧的 `PickedDocument` 增加 `persisted` 字段的序列化测试；Kotlin 侧对 `SecurityException` 分支增加单测（mock ContentResolver）。
- Estimated effort: 3–5 小时

### Finding: 正式 Windows 产物要求 AVX2

- Severity: Medium
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `.github/workflows/release.yml` 的 `build` job
- Evidence:
  - File: `.github/workflows/release.yml:104-110`（job 级 `RUSTFLAGS: -C target-cpu=x86-64-v3`）
  - File: `src-tauri/Cargo.toml`（本地 profile 不含该 flag）
  - Function / Module: build job 环境变量
  - Relevant behavior: 只有 release job 设置 `x86-64-v3`（Haswell 级别指令集，要求 AVX2）；本地构建与 CI 都不带该 flag，因此本地“能跑”不能证明产物能跑。
- Problem: 面向全体 Windows 用户的产物被隐式限定到 2013 年之后的 CPU。
- Why it matters: 受影响机器上进程在显示窗口前即触发非法指令（0xC000001D），表现为“安装后打不开”，且只在真实用户机器上暴露。
- Realistic failure scenario: 2012 年的商务本或默认虚拟机的用户下载 1.6.4 安装包 → 双击无反应 → 被判定为“安装损坏”。
- Minimal fix: 改用 `x86-64-v2`（不要求 AVX2），或在 release job 增加一次最低指令集的启动冒烟测试。
- Better long-term fix: 放弃单一 `target-cpu`，用运行时特性检测分发两个安装包并在安装器里探测。
- Regression test suggestion: CI 中以 `qemu -cpu qemu64`（无 AVX2）运行构建产物的 `--version` 冒烟命令。
- Estimated effort: 30–60 分钟

### Finding: Windows 产物未签名且无来源证明

- Severity: Medium
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `.github/workflows/release.yml`、`src-tauri/tauri.conf.json`
- Evidence:
  - File: `.github/workflows/release.yml:157-226`（`tauri-action` 构建并上传，无 `signtool` / attestation 步骤）
  - File: `src-tauri/tauri.conf.json`（无签名身份配置、无 updater 公钥）
  - Function / Module: build job 产物阶段
  - Relevant behavior: 安装包与便携 ZIP 直接上传；SHA256SUMS 与产物同处一个 release、同一个信任域。
- Problem: 产物缺少来源证明与代码签名，校验和只能发现传输损坏，不能发现替换。
- Why it matters: 密码管理器的最终安装环节是供应链攻击最经济的落点；SmartScreen 的“未知发布者”提示也会劝退用户。
- Realistic failure scenario: 下载镜像被劫持或短链被替换，安装包被改写；用户无从察觉，因为校验和也在同一页面。
- Minimal fix: 引入 `actions/attest-build-provenance` 或等价机制，并在 README 公布指纹获取方式。
- Better long-term fix: 采购代码签名证书接入 Authenticode，并启用 tauri updater + minisign 公钥校验。
- Regression test suggestion: 工作流静态测试断言上传步骤之前存在签名或证明步骤。
- Estimated effort: 1 天（证书流程另计）

### Finding: `release.mjs` 复用可能已过期的本地标签

- Severity: Low
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `scripts/release.mjs`
- Evidence:
  - File: `scripts/release.mjs:255-266`（`const exists = git tag -l …`；`else` 分支只打印 “Tag … already exists”）
  - File: `scripts/release.mjs:268-282`（随后仍然推送分支与标签）
  - Function / Module: 步骤 5「Tagging」与步骤 6「Pushing」
  - Relevant behavior: 标签已存在时既不校验它指向哪次提交，也不新建；脚本照样推送分支并打印“Release complete”。
- Problem: “标签已存在”被当作“已发布”的同义词，而两者并不等价。
- Why it matters: 标签是发布产物的真实来源；标签停在旧提交时，分支与发布产物已经分叉，而脚本报告成功。
- Realistic failure scenario: 发布后又在分支上提交若干改动，再次运行脚本（此时版本号未变，步骤 4 无变更可提交）→ 标签仍指向旧的发布提交 → 脚本推送分支并报成功，但“最新代码”从未进入任何 release。
- Minimal fix: 标签已存在时断言其解析到本次发布提交（或 `HEAD`），否则报错退出。
- Better long-term fix: 用 `git tag -f` 之前强制“标签必须等于发布提交”的不变量校验，并在脚本层禁止静默复用。
- Regression test suggestion: `tests/release-script.test.mjs` 增加“标签指向父提交时必须拒绝”的用例。
- Estimated effort: 1–2 小时

### Finding: changelog 基线取“按创建时间最新的标签”

- Severity: Low
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `scripts/changelog.mjs`
- Evidence:
  - File: `scripts/changelog.mjs:59-71`（`git tag --sort=-creatordate` 取第一个）
  - File: `scripts/changelog.mjs:146`（`fromRef` 默认取该标签）、`scripts/release.mjs`（调用时不传 `--from`）
  - Function / Module: `getLatestTag`
  - Relevant behavior: 基线是“时间上最新”的标签，而不是“当前 HEAD 的祖先链上最新”的标签。
- Problem: 跨分支标签或回溯重发会让变更区间算错。
- Why it matters: `CHANGELOG.md` 与 `RELEASE.md` 的提交列表直接来自该区间，算错就会静默丢条目或重复条目。
- Realistic failure scenario: 在维护分支上打出 `v1.6.4-hotfix` 标签后回到 `main` 发版 → 变更区间从 hotfix 标签开始 → `main` 上若干提交从发布说明里消失。
- Minimal fix: 改用 `git tag --merged HEAD --sort=-v:refname` 取基线，并在目标版本早于最新合并标签时拒绝发布。
- Better long-term fix: 变更日志从合并历史（`--first-parent`）生成，避免分支拓扑带来的歧义。
- Regression test suggestion: 在临时仓库里构造“另一分支上更新的标签”，断言基线仍为当前 HEAD 的祖先标签。
- Estimated effort: 2–3 小时

### Finding: `package-lock.json` 版本号长期停留在 1.4.0

- Severity: Low
- Confidence: High
- Category: Configuration
- Status: Confirmed
- Affected area: `scripts/version.mjs`、`package-lock.json`
- Evidence:
  - File: `package-lock.json:3,9`（`"version": "1.4.0"`，而 `package.json` 为 1.6.4）
  - File: `scripts/version.mjs:21-25`（`VERSION_FILES` 不含 lockfile）、`scripts/release-files.mjs`（发布文件白名单同样不含 lockfile）
  - Function / Module: `setVersion` / `ensureOnlyReleaseFilesChanged`
  - Relevant behavior: 版本提升只改三个版本源，lockfile 根版本不动。
- Problem: 版本源之间出现无声漂移，且漂移文件会阻断发布流程。
- Why it matters: `npm install` 会把 lockfile 根版本改回当前版本，产生一个“非发布文件”的已跟踪改动，发布前置检查会以“Commit non-release tracked changes before releasing”直接失败。
- Realistic failure scenario: 开发者本地执行一次 `npm install`（哪怕只是加一个开发依赖）→ 下一次发版在第 0 步就被拦下，排查成本高。
- Minimal fix: `VERSION_FILES` 增加 `package-lock.json` 的两个版本字段，并把该文件加入发布白名单。
- Better long-term fix: 引入单一版本源（由 `package.json` 派生其余文件）并在 CI 中断言一致性。
- Regression test suggestion: `tests/release-version.test.mjs` 断言 `package-lock.json` 根版本与 `package.json` 一致。
- Estimated effort: 1 小时

### Finding: 标签过滤规则无法匹配预发布版本

- Severity: Low
- Confidence: High
- Category: Release
- Status: Confirmed
- Affected area: `.github/workflows/release.yml`、`skills/version-release/SKILL.md`
- Evidence:
  - File: `.github/workflows/release.yml:4-6`（`tags: - "v[0-9]+.[0-9]+.[0-9]+*"`）
  - File: `skills/version-release/SKILL.md`（声明“带可选预发布后缀的语义化标签会触发 release 工作流”）
  - Function / Module: 触发器过滤器
  - Relevant behavior: GitHub 的过滤器语法里 `+` 是量词而非字面量，该模式只能匹配 `v<数字>.<数字>.<数字>`；`v1.7.0-rc.1` 不匹配。
- Problem: 文档承诺的预发布触发方式在过滤器层面不成立。
- Why it matters: `release.mjs` 能正常创建并推送 `v1.7.0-rc.1` 标签，脚本报告“Release complete”，但不会有任何 Release 工作流运行——版本看上去已发布，实际没有产物。
- Realistic failure scenario: 团队计划用 `1.7.0-rc.1` 做预发布验证 → 标签推送成功 → CI 无反应 → 直到用户反馈才发现没有安装包。
- Minimal fix: 过滤器改为 `- "v*"`（版本合法性已由 `validate` 的 semver 校验保证），或追加 `- "*"` 模式。
- Better long-term fix: 在 `release.mjs` 推送标签后校验对应的工作流运行是否被触发（通过 GitHub API），未触发即报错。
- Regression test suggestion: 用一个过滤规则测试工具（或简单的正则断言）覆盖 `v1.7.0-rc.1`。
- Estimated effort: 30 分钟

### Finding: `atomic_write` 用默认权限重建文件，丢弃原有的 0600

- Severity: Low
- Confidence: High
- Category: Privacy
- Status: Confirmed
- Affected area: `src-tauri/src/util.rs`
- Evidence:
  - File: `src-tauri/src/util.rs:31-67`（`std::fs::write(&tmp, bytes)` → rename 覆盖目标）
  - File: `src-tauri/src/config/store.rs:97-102`（对照：配置文件显式 `set_permissions(0o600)`）
  - Function / Module: `atomic_write`
  - Relevant behavior: 临时文件以 umask 默认权限（通常 0644）创建并直接 rename 覆盖目标，目标文件原有的严格权限被丢弃。
- Problem: 库文件与远程镜像的每次保存都会把权限放宽到 umask 默认值。
- Why it matters: 影响有限（KDBX 本身是加密的，密钥文件从不重写），但这与 `config.json` 已有的收紧策略不一致，Unix 多用户机器上密文库与备份副本变成同机可读。
- Realistic failure scenario: 用户手动把 `vault.kdbx` 设为 0600 → 保存一次 → 权限回到 0644 → 同机其他账号可读取密文库（可离线爆破主密码）。
- Minimal fix: 写临时文件时使用 `OpenOptions::new().mode(0o600)`（Unix）或在 rename 前 `set_permissions` 复原目标原有权限。
- Better long-term fix: 统一封装“受限原子写”工具函数，配置、库文件、镜像共用。
- Regression test suggestion: Unix 单测断言 `atomic_write` 之后目标文件权限为 0600。
- Estimated effort: 2–3 小时

### Finding: `random_bytes` 在 OS 熵源失败时 panic

- Severity: Low
- Confidence: High
- Category: Stability
- Status: Confirmed
- Affected area: `src-tauri/src/crypto/primitives.rs`
- Evidence:
  - File: `src-tauri/src/crypto/primitives.rs:33-35`（`getrandom::getrandom(&mut buf).expect("OS RNG must be available")`）
  - Function / Module: `random_bytes`
  - Relevant behavior: 熵源不可用时直接 panic（Rust 侧为 abort 级中断），调用方无法降级。
- Problem: 唯一的熵源失败路径是硬崩溃。
- Why it matters: 该函数用于生成会话随机数与 SRP nonce，崩溃点可能在保存流程中间，留下不一致的中间状态。
- Realistic failure scenario: 容器/受限环境（早期启动、熵池未初始化）调用 `random_bytes` → 进程 abort → 用户看到应用无提示消失。
- Minimal fix: 改为返回 `Result<Vec<u8>, String>`，把错误沿调用链向上传递；在保存路径上给出可读提示。
- Better long-term fix: 熵源失败时保留上一轮已生成的会话随机数，避免中途崩溃。
- Regression test suggestion: 抽象熵源依赖后，用失败的假实现断言返回错误而非 panic。
- Estimated effort: 2–4 小时

### Finding: `+page.svelte` 仍是 2620 行 / 68 个顶层函数的巨型组件

- Severity: Low
- Confidence: High
- Category: Maintainability
- Status: Confirmed
- Affected area: `src/routes/+page.svelte`
- Evidence:
  - File: `src/routes/+page.svelte`（2620 行；68 个顶层函数；混合壳布局、导入导出编排、面板尺寸状态机、TCATO 覆盖层、移动端分支）
  - Function / Module: 应用根组件
  - Relevant behavior: 上一轮审计记录为 3338 行 / 91 个函数，本次已下降，但仍远超 1000 行的可维护性阈值。
- Problem: 单一组件仍聚合四类以上互不相干的职责。
- Why it matters: 任何功能改动都在同一文件碰撞，回归只能靠整体验证；本次的移动端 SAF、长按菜单、安全区三处改动全部落在同一文件的不同区域。
- Realistic failure scenario: 新增一种导入格式时，修改导出编排附近的局部变量名，与面板宽度状态机的作用域冲突，回归只在特定布局下显现。
- Minimal fix: 把导入导出编排抽到 `lib/services/io.ts`，窗口与面板尺寸状态机抽为 composable，把该文件降到 1500 行以内。
- Better long-term fix: 按“shell / io / layout-state / session-view”四块拆分，并为每块补运行时测试。
- Regression test suggestion: 抽取后为 io 编排补 `node:test` 单测（复用现有 fake-invoke 模式）。
- Estimated effort: 2–3 天

## 5. Architecture Concerns / 架构关注点

- Coverage: High — 目录树、状态归属、插件注册、IPC 调用面。
- Inspected evidence: `src-tauri/src/**` 模块划分、`src-tauri/src/lib.rs`（命令与插件注册）、`src/lib/services/*`、`src/routes/*`。
- Exclusions / limits: 未做运行时调用图。

要点：分层方向正确（UI → services → IPC → Rust 领域层），密钥不出后端，视图纪元与 `SessionViewToken` 的一致性维护得比同类项目好。唯一结构性债务是前端根组件（见 F-20）与脚本层与产品层混杂（发布脚本承担了版本、变更日志、身份校验三类职责）。

## 6. Security Concerns / 安全关注点

- Coverage: High — 密钥生命周期、IPC 掩码、bridge/rpc、capabilities、SAF 授权、发布签名。
- Inspected evidence: `config/store.rs`、`vault/sessions.rs`、`platform/clipboard.rs`、`bridge/server.rs`、`plugins/saf/**`、两个工作流的权限与密钥注入路径。
- Exclusions / limits: 未做动态渗透。

要点：本次未发现认证绕过或注入类漏洞。安全面的主要敞口集中在**构建与分发**：签名私钥暴露给构建代码（F-2）、写权限令牌与构建代码同域（F-17）、产物未签名（F-4）、远端可控 key 参与本地路径构造（F-6）、明文附件落盘权限偏宽（F-7）。

## 7. Stability Concerns / 稳定性关注点

- Coverage: High — 原子写、SAF 写入、下载边界、panic 路径、发布脚本状态机。
- Inspected evidence: `util.rs`、`vault/file_io.rs`、`vault/persist.rs`、`remote/*`、`crypto/primitives.rs`、`scripts/release.mjs`。
- Exclusions / limits: 未做长时间压力运行。

要点：桌面保存路径的原子性与错误分类仍然扎实；风险集中在移动端 SAF 写入（F-5、F-19）与无上限读取（F-8）、以及发布脚本在异常状态下的恢复能力（F-9、F-10）。

## 8. Performance Concerns / 性能关注点

- Coverage: Medium — 依赖重量、虚拟列表、下载与解析路径。
- Inspected evidence: `package.json`、`src-tauri/Cargo.toml`、`EntryTable.svelte` 虚拟窗口、`remote/*` 超时设置。
- Exclusions / limits: 无 profiling 与基准数据。

要点：未发现热点问题。条目表虚拟化（可见行 + 每侧 6 行 overspac）、远程客户端单例、favicon 并发上限（1–16，默认 8）都是合理的。唯一的性能类风险是 F-8（内存无上限）。

## 9. Testing Gaps / 测试缺口

- Coverage: High — 26 个前端测试文件、Rust 测试分布、CI 门禁。
- Inspected evidence: `tests/*.mjs`、`src-tauri/src/**/tests.rs`、`.github/workflows/ci.yml`。
- Exclusions / limits: 未统计行覆盖率。

要点：Rust 侧 455 个测试覆盖了 KDF 向量、错误口令拒绝、并发保存、合并冲突、配置往返等关键不变量，质量高。缺口集中在三处：① 前端 IPC 胶水层（`services/vault.ts`）只有源码正则级契约测试，缺少运行时行为测试；② 主 CI 不编译 Android，Android 专属代码路径（`cfg(target_os = "android")`、Kotlin 插件）只有在打标签时才会被编译；③ 本次 20 条发现中，**没有任何一条能被现有测试捕获**——发布脚本状态机、Gradle 补丁脚本、SAF 写入失败路径都缺少对应的负向用例。

## 10. Maintainability Concerns / 可维护性关注点

- Coverage: High — 文件行数排行、函数计数、脚本与文档一致性。
- Inspected evidence: `src/**` 行数统计、`scripts/**`、`docs/**`。
- Exclusions / limits: 未逐文件比对命名规范。

要点：注释密度与“为何这样写”的说明在 Rust 侧明显高于同类项目（`docs/PITFALLS.md` 记录了大量带证据的坑）。主要问题是 F-20 的巨型组件、发布脚本的多职责、以及 `docs/android.md` 中若干“已完成”表述与实际能力不完全对应（SAF 写入的原子性、持久授权的可用性）。

## 11. Design / Principles Concerns / 设计原则关注点

- Coverage: High — 逐条对照 `rubrics/principles.md`。
- Inspected evidence: 全量代码 + 上一次审计的原则结论。
- Exclusions / limits: —

要点：fail-fast（4.4）、原子性、命令查询分离、视图纪元防陈旧回写做得很好；主要违反项是 SRP（1.1）与文件体量（1.2）在前端巨型组件上失守，以及脚本层的“隐式副作用”（rebase/删标签/推标签都发生在同一个 `release.mjs` 进程内）。

## 12. Release Concerns / 发布关注点

- Coverage: High — `release.yml` 逐行、全部发布脚本、一次真实 release run（run 39526337094，6 个 job 全绿）。
- Inspected evidence: `release.yml`、`ci.yml`、`scripts/release.mjs`、`scripts/changelog.mjs`、`scripts/version.mjs`、`scripts/validate-release-version.mjs`、`scripts/patch-android-gradle-repos.ps1`、`scripts/configure-android-signing.ps1`。
- Exclusions / limits: 未执行完整发布演练。

要点：这是本次最弱的一环（5.5/10）。正面：版本源三方一致校验、action SHA 固定、工具链锁定、校验和、per-ABI APK 校验与 fail-closed 暂存都做得很扎实。负面：F-1（发布身份未绑定提交）、F-2（签名私钥暴露）、F-18（验证不阻塞 Windows 产物）、F-9/F-10（脚本异常状态下的恢复）、F-12（Android 仓库补丁自证无效）、F-14（预发布标签不触发）。

## 13. Documentation Analysis / 文档分析

- Coverage: High — `docs/*`、README、AGENTS、skills references、TODO。
- Inspected evidence: `docs/android.md`、`docs/PITFALLS.md`、两个 skills reference 与本次代码行为逐条比对。
- Exclusions / limits: 未审查历史 CHANGELOG 全文。

要点：文档质量总体高于同类项目，`docs/PITFALLS.md` 的坑位记录带证据、可检索。偏差集中在三处：① `docs/android.md` 把 SAF 读写描述为“已落地”，但未说明“原地截断写入”带来的失败即毁库风险（F-5）与“持久授权可能被 provider 拒绝”的实际表现（F-19）；② `skills/version-release/SKILL.md` 声称预发布标签会触发 release（F-14）；③ `scripts/patch-android-gradle-repos.ps1` 的注释宣称“为每个 repositories 块补齐 google()/mavenCentral()”，与实际行为不符（F-12）。

## 14. Configuration Safety Analysis / 配置安全分析

- Coverage: High — Rust/TS 双端 defaults 与归一化、版本源、capabilities。
- Inspected evidence: `config/settings.rs`、`config/store.rs`、`lib/types/settings.ts`、`lib/services/settings.ts`、`tauri.conf.json`、`capabilities/*.json`。
- Exclusions / limits: Android 真机行为未验证。

要点：`recentFileNames` 的双端归一化（只保留仍在最近列表中的路径、名称截断 120 字符）完全对齐；Rust 侧 `#[serde(default)]` 容器级默认保证旧配置文件不崩。要点问题只有 F-13（lockfile 版本漂移）。

## 15. Observability / Operability Analysis / 可观测性分析

- Coverage: Medium — 日志语句、错误分类、CI 可诊断性。
- Inspected evidence: `eprintln!` 使用点、错误字符串风格、workflow job 划分与 artifact 命名。
- Exclusions / limits: 桌面应用无 metrics / tracing / alerting 体系。

要点：错误信息普遍带上下文（中文、指向具体动作），附件清扫会打印计数日志，CI 的 artifact 名与 job 名清晰可定位。缺口是：发布流程没有留下“本次发布对应哪次 commit / 哪些资产哈希”的机器可读清单（F-1 的直接对策），也没有产物的 provenance 记录。

## 16. Data Integrity Analysis / 数据完整性分析

- Coverage: High — 原子写、备份轮转、合并不变量、镜像落盘、SAF 写入。
- Inspected evidence: `util.rs`、`remote/backup.rs`、`vault/persist.rs`、`vault/file_io.rs`、合并与回收站相关测试。
- Exclusions / limits: 备份恢复未实测演练。

要点：桌面路径的数据完整性是本项目的强项（tmp + fsync + rename + 目录 fsync，镜像写前先复制旧字节再原子替换，备份轮转带去重后缀）。破坏点只有 F-5（移动端无原子替换）与 F-6（镜像落盘路径可被远端 key 影响）。

## 17. Privacy / Data Governance Analysis / 隐私与数据治理分析

- Coverage: High — 遥测、剪贴板、临时文件、配置、日志、IPC 掩码。
- Inspected evidence: 全仓 `console.` / 遥测 / 网络外发搜索，`platform/clipboard.rs`，`vault/temp_attachments.rs`，`config/store.rs`。
- Exclusions / limits: —

要点：零遥测、零遥测 SDK、剪贴板只擦除自己复制的内容、凭据不跨 IPC、错误串不含密文——隐私卫生优秀。本次的两个偏差是 F-7（明文附件默认权限）与 F-15（原子写放宽库文件权限）。

## 18. Accessibility / UX Correctness Analysis / 可访问性与 UX 正确性分析

- Coverage: Medium — 语义与 ARIA、焦点管理、窄屏与移动端布局、错误态。
- Inspected evidence: `ModalShell.svelte`、`EntryTable.svelte`、`SettingsDialog.svelte`、`routes/+page.svelte` 的窄屏分支、本次新增的安全区与长按菜单改动。
- Exclusions / limits: 未用屏幕阅读器与真机实测。

要点：`ModalShell` 的焦点陷阱、模板化的 `Toggle`/`TextField`/`Button`、行内操作的 aria 标注都到位。移动端新改动方面：长按菜单原先依赖 `MouseEvent` 构造器携带坐标，在 Android WebView 上不可靠，现改为把 `pointerdown` 坐标显式传给页面菜单状态并对 `(0,0)` 的原生事件做回退（本次会话内已修复）；设置页此前未预留系统栏，本次已补上 `.settings-shell` 的安全区内边距。仍有一处未验证点：真机长按与系统栏的实际观感。

## 19. Supply Chain / Reproducibility Analysis / 供应链与可重现性分析

- Coverage: High — action 固定、工具链、签名、密钥注入、依赖清单。
- Inspected evidence: 两个工作流的全部 `uses:`、两个 `rust-toolchain.toml` / `package-lock.json` / `Cargo.lock`、签名脚本。
- Exclusions / limits: 依赖 CVE 未与漏洞库全量比对。

要点：SHA 固定、工具链锁定、lockfile 提交、`cargo update -p` 的最小化更新策略（避免夹带全量依赖升级）都做对了。风险点是 F-2（签名私钥暴露给构建代码）与 F-4（产物无签名/无证明）。

## 20. Cost / Resource Economics Analysis / 成本与资源经济性分析

- Coverage: Medium — 外部请求、后台任务、存储增长点。
- Inspected evidence: HIBP / favicon / S3 / WebDAV 调用点与并发上限、`backup.rs` 的保留修剪。
- Exclusions / limits: 无生产遥测数据。

要点：单机桌面应用无显著成本驱动。HIBP 为 k-匿名分片、favicon 并发上限 8 且单次 20 秒超时、备份按 `backupCount` 修剪、远端下载有 120 秒上限。唯一无界项是响应体积（F-8）。

## 21. AI / LLM Safety Analysis / AI 安全分析

- Coverage: Not assessed — 产品代码不存在 LLM / prompt / RAG / tool 调用面（`.opencode/` 与 `skills/` 属工具链，不在产品边界内），该维度不适用。

## 22. Fallback / Defensive Code Analysis / 兜底与防御式代码分析

- Coverage: High — 静默兜底、空 catch、兼容性分支、防御式猜测扫描。
- Inspected evidence: `unwrap_or_default` / `_ => {}` / 空 `catch` / `let _ =` 全量扫描结果与逐点判定。
- Exclusions / limits: —

要点：绝大多数 `let _ =` 都是有意的 best-effort（fsync 目录、清理临时文件），注释也写明了原因。真正的静默吞错有两处：F-19（SAF 授权失败被吞）与 `SafPlugin.kt:99-101`（`displayName` 查询失败回退到 URI 末段，可接受）。桌面端未发现把真实错误降级为默认值的分支。

## 23. Testing Authenticity Analysis / 测试真实性分析

- Coverage: High — 逐文件评估断言对象。
- Inspected evidence: `tests/*.mjs` 26 个文件与 Rust 测试模块。
- Exclusions / limits: —

要点：Rust 测试多为真实行为断言（标准向量、错误口令、并发、合并语义），可信度高。前端测试里有一类需要警惕：`component-contracts.test.mjs` / `ui-primitives.test.mjs` 是对源码文本的正则断言（例如断言某组件未使用已退役的 class 名），它们能守住“样式契约”，但对运行时行为零覆盖——本次 F-5、F-19 这类运行时问题正是它们结构上无法发现的。此外 `tests/entry-table-contract.test.mjs` 这类“读源码字符串”的测试在重构时会给出假绿/假红，建议逐步替换为对导出纯函数的断言。

## 24. Type Safety Analysis / 类型安全分析

- Coverage: High — `as any` / `@ts-ignore` 扫描、tsconfig、Rust `Result`。
- Inspected evidence: `src/**`、`src-tauri/src/**`、`tsconfig.json`。
- Exclusions / limits: —

要点：`as any` / `@ts-ignore` 零命中；Rust 侧统一 `Result<T, String>`，公共 API 未使用裸 `unwrap()`（仅测试与“调用方保证”的内部断言）。本次新增的 `ContextMenuPoint` 显式类型与 `point?.x ?? event.clientX` 的可选参数模式，避免了 `undefined` 流入样式字符串。

## 25. Frontend State Analysis / 前端状态分析

- Coverage: Medium — 组件体量、状态重复、effect 链、菜单所有者模型。
- Inspected evidence: `routes/+page.svelte`、`components/EntryTable.svelte`、`services/vault.ts`、`services/settings.ts`、本次改动面（安全区、长按菜单）。
- Exclusions / limits: 一次全量前端 diff 审计因子代理端点故障未完成。

要点：状态归属清晰（后端持有库，前端只持有 UI 态与脱敏配置），`$effect` 订阅 store 的模式统一，`activeContextMenu` 所有者模型保证同一时刻只有一个菜单。风险点是根组件体量（F-20）与 `+page.svelte` 中“面板宽度写入顺序”这一类已被注释自证的隐式耦合。

## 26. Backend API Analysis / 后端 API 分析

- Coverage: Medium — IPC 命令契约、输入校验、错误结构。
- Inspected evidence: `commands/*`、`vault/sessions.rs`、`remote/*`。
- Exclusions / limits: IPC 非传统 REST API。

要点：命令入参校验充分（路径规范化、远程 key 校验、密钥文件 10 MiB 上限、附件 64 MiB 上限），错误一律为可读中文串且不含密文。`Result<T, String>` 的粗粒度错误类型是可维护性上的小债，但可接受。

## 27. Dependency Weight Analysis / 依赖重量分析

- Coverage: High — 前后端依赖清单与 feature 纪律。
- Inspected evidence: `package.json`、`src-tauri/Cargo.toml`、`plugins/saf/Cargo.toml`。
- Exclusions / limits: 未逐包统计传递体积。

要点：依赖极轻（前端运行时依赖个位数），Rust 侧刻意避免 `openssl`（全 rustls 栈），Android 侧新增的 `webpki-root-certs` 只在 Android 目标引入。新增的 `tauri-plugin-saf` 为本地插件（仅依赖 tauri + serde），符合“插件自持”的低重量原则。

## 28. Code Consistency Analysis / 代码一致性分析

- Coverage: Medium — 命名、导入组织、错误处理风格抽查。
- Inspected evidence: `src/**` 抽样、`scripts/**` 全文。
- Exclusions / limits: 未逐文件比对。

要点：命名与错误风格高度一致（中文错误串、动词开头的函数名、`snake_case` Rust / `camelCase` 线上字段）。不一致点：`scripts/` 的注释与实现有偏差（F-12、F-14），`docs/android.md` 的“已落地”表述比实现更乐观（F-5、F-19）。

## 29. Comment Coverage Analysis / 注释覆盖分析

- Coverage: Medium — 关键模块注释密度与过期注释抽查。
- Inspected evidence: `vault/*`、`remote/*`、`services/*`、两个 `scripts/*.ps1`。
- Exclusions / limits: —

要点：注释普遍解释“为什么”而非“做了什么”，包含大量已验证事故的记录，这是本项目最值得保留的工程习惯。过期注释有两处：`patch-android-gradle-repos.ps1` 宣称“为每个 repositories 块补齐”但实现有缺陷（F-12）；`configure-android-signing.ps1` 的注释描述的依赖顺序与实际“明文落盘进工作区”的实现未提示安全含义（F-2）。

---

## 30. Principles Compliance / 原则遵循

### Principles Violated

| Principle                        | Violations | Severity    | Affected Areas                                                                                                              |
| -------------------------------- | ---------- | ----------- | --------------------------------------------------------------------------------------------------------------------------- |
| Single Responsibility (SRP) 1.1  | 3          | Medium      | `+page.svelte`、`release.mjs`（版本+变更日志+身份校验+历史改写）、`patch-android-gradle-repos.ps1`（字符串补丁 + 仓库策略） |
| File Size Limit 1.2              | 1          | Low/Medium  | `+page.svelte` 2620 行、`services/vault.ts` 1700 行、`services/settings.ts` 1361 行                                         |
| Fail-Fast 4.4                    | 2          | Medium      | SAF 持久授权失败被吞（F-19）、`random_bytes` panic 而非返回错误（F-16）                                                     |
| No Hidden Side Effects 5.3       | 2          | High/Medium | `release.mjs` 在同一进程内 rebase / 删标签 / 推标签（F-9、F-10）；构建 job 静默持有写权限令牌（F-17）                       |
| Principle of Least Privilege 4.6 | 2          | Medium      | 构建 job 令牌（F-17）、编译期可见的签名私钥（F-2）                                                                          |
| Timeout Every External Call 10.4 | 1          | Medium      | 有超时但无体积上限的下载/读取（F-8）                                                                                        |
| Don't Swallow Errors 6.1         | 1          | Medium      | SAF 授权失败（F-19）                                                                                                        |

### Principles Respected

- **Fail-Fast 与原子性（4.4 / 1.2 的正面例）**：桌面保存路径坚持 tmp + fsync + rename + 目录 fsync，失败即返回且清理临时文件。
- **命令查询分离（3.2）**：`util.rs` 的 `url_host` 等纯函数与 I/O 函数分离清楚。
- **视图纪元（5.1 的正面例）**：`SessionViewToken` / `sessionView.capture()` 模式系统性阻断了陈旧异步回写，是本次审计中最亮眼的设计。
- **Immutable Preferences（5.1）**：前端 store 更新一律以新对象替换，禁绝原地修改。
- **稳定依赖（2.6）**：`services/vault.ts` 作为唯一 IPC 门面，其余组件不直接 `invoke`。
- **测试行为而非实现（8.1）**：Rust 测试使用标准向量与真实 round-trip，而非 mock 内部调用。

---

## 31. Recommended Fix Order / 建议修复顺序

### Fix Immediately

1. **F-2** 签名私钥与构建代码隔离（改为独立签名 job）。
2. **F-1** 发布身份绑定提交（validate 校验 HEAD == 标签提交；禁止非 tag dispatch 覆盖已发布 release）。
3. **F-5** 移动端保存的原子性（写前本地备份 / 两阶段写入）。
4. **F-6** 远程 key 的文件名净化 + 路径逃逸断言。

### Fix Before Stable Release

5. **F-18** `build` job 依赖 `verify`。
6. **F-17** 构建 job 令牌收敛（`persist-credentials: false` + 独立发布 job）。
7. **F-9** regenerate 流程的顺序与恢复（门禁前置 + 恢复文档）。
8. **F-12** Gradle 仓库补丁改为块内判定（并加回归测试）。
9. **F-3** 放弃 `x86-64-v3` 或补最低指令集冒烟测试。
10. **F-7 / F-15** 临时附件与原子写的文件权限。
11. **F-8** 下载与读取的体积上限。
12. **F-19** 持久授权状态回传与最近文件策略。
13. **F-4** 产物签名 / provenance（可与下个版本一起推进）。

### Schedule Later

14. **F-10 / F-11 / F-13 / F-14** 发布脚本与版本源的健壮性（标签一致性、变更日志基线、lockfile 版本、预发布过滤）。
15. **F-16** 熵源失败的错误传播。
16. **F-20** `+page.svelte` 拆分。
17. 主 CI 增加 Android 编译 job（建议但不阻塞发布：当前仅 tag 触发时编译 Android，PR 阶段无法拦截 Android 专属编译错误）。

### Ignore for Now

18. `remote/backup.rs` 的备份文件命名细节（不影响正确性）。
19. `docs/android.md` 中若干措辞润色（不影响操作）。

---

## 32. Quick Wins / 快速收益

| 项   | 内容                                            | 成本    | 收益                                    |
| ---- | ----------------------------------------------- | ------- | --------------------------------------- |
| F-14 | 标签过滤改为 `v*`（版本合法性已有 semver 校验） | 5 分钟  | 预发布标签不再“看起来已发布却无产物”    |
| F-13 | `version.mjs` 同步 `package-lock.json` 根版本   | 20 分钟 | 消除因 `npm install` 触发的发布前置失败 |
| F-18 | `build: needs: [validate, verify]`              | 5 分钟  | 红测试不再产出可发布安装包              |
| F-17 | checkout 增加 `persist-credentials: false`      | 5 分钟  | 阻断构建期令牌读取                      |
| F-10 | 标签已存在时校验其指向发布提交                  | 30 分钟 | 消除“标签停在旧提交”的静默分叉          |
| F-3  | `target-cpu` 降到 `x86-64-v2`                   | 10 分钟 | 覆盖 2013 年前的老机器                  |
| F-12 | 仓库补丁脚本改为括号配对 + 固化为测试           | 2 小时  | 恢复该脚本本来的保护作用                |

---

## 33. Long-term Refactor Plan / 长期重构计划

1. **发布管线**（动机：F-1/F-2/F-9/F-10/F-18 全部源于“发布身份、构建、签名、发布”四件事挤在一条工作流与一个脚本里）。
   - 方案：拆成“身份校验 → 只读构建 → 产物签名 → 发布”四段；引入 release manifest（版本 + 提交 + 资产哈希）；发布脚本只做编排，不做历史改写。
   - 风险：中等（涉及发布流程重排）；缓解：先用 dry-run 与 fixture 仓库覆盖每段。
   - 测试策略：为 validate / 标签一致性 / 幂等重跑补齐脚本级测试。
2. **移动端文档写入层**（动机：F-5/F-19 表明 SAF 与桌面路径的语义差异没有在类型层体现）。
   - 方案：引入 `DocumentTarget` 抽象（桌面原子替换 / 移动两阶段写入 + 授权状态），让 UI 拿到结构化结果而非裸 `String`。
   - 风险：中等；缓解：先落地本地备份这一最小修复，再逐步抽象。
   - 测试策略：注入式失败测试覆盖“写一半失败”“授权丢失”。
3. **前端壳层拆分**（动机：F-20）。
   - 方案：把 `+page.svelte` 拆为 shell / io / layout-state 三块，逐块补运行时测试。
   - 风险：中等（回归面大）；缓解：先抽 io 编排并补单测，再动状态机。
   - 测试策略：每抽一块补对应单测，`npm run verify` 全量把关。
