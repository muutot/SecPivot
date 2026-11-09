# Android 安卓支持评估与落地清单

SecPivot（Svelte 5 + Tauri 2 + Rust）对安卓平台的可移植性评估、已改动仓库文件、以及剩余落地步骤。本文档同时作为"必须由具备工具链与网络的环境执行"的任务清单——**凡需 Android APK 或真机行为的部分，必须以真实工具链/设备证据为准；Windows 本机代理不可达 `dl.google.com`，只能由 GitHub-hosted Linux runner 等环境闭环**。

## 结论

- **核心可行**：Tauri 2 原生支持安卓；KeePass 打开/编辑/保存、TOTP、加密等纯逻辑（`keepass` crate、`crypto/`、`vault/`）跨平台直接复用。
- **代价**：需补工具链、一次真实 Android 编译（借此清瘦 desktop 专用依赖）、前端触控 / 文件选择 / 后台锁适配、APK 出包与 CI。
- **桌面零影响**：所有移动端改动已用 `#[cfg(desktop)]` / `#[cfg(mobile)]` / 平台限定 `capability` / `tauri.android.conf.json` 隔离；桌面分支行为不变，须在有网环境跑 `npm run verify` 回归确认。

## 已完成的仓库改动

| 改动              | 文件                                                                                               | 说明                                                                                                                                                                                                                                 |
| ----------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 安卓最小 SDK 声明 | `src-tauri/tauri.conf.json`                                                                        | `bundle.android.minSdkVersion: 24`；桌面 NSIS 分支不受影响                                                                                                                                                                           |
| 安卓独立包名      | `src-tauri/tauri.android.conf.json`（新增）                                                        | `identifier: com.secpivot.mobile`；桌面仍为 `com.secpivot.desktop`                                                                                                                                                                   |
| 安卓能力          | `src-tauri/capabilities/android.json`（新增）                                                      | `platforms:["android"]` + `core/dialog/opener/saf` 权限；Windows 下被过滤                                                                                                                                                            |
| 桌面功能门控      | `src-tauri/src/lib.rs`                                                                             | 系统托盘 / 全局热键 / auto-type / TCATO 的 `register_global_hotkey`、`setup_tray`、`handle_global_hotkey`、`toggle_main_window`、`handle_close_requested` 及 import/常量加 `#[cfg(desktop)]`；builder 链重构为分步变量以支持条件编译 |
| 桌面专属依赖隔离  | `src-tauri/Cargo.toml`                                                                             | `enigo` / `keyring` / `tauri-plugin-global-shortcut` / `tungstenite` / `num-bigint` / `aes` / `cbc` / `cipher` / `block-padding` 移入 `[target.'cfg(any(windows,macos,linux))'.dependencies]`，Android 不编译                        |
| 桌面专属模块门控  | `src-tauri/src/lib.rs`、`src-tauri/src/{commands,platform,vault,crypto}`                           | `bridge`/`rpc` 模块、`commands::bridge/credential/tcato`、`platform::credential/focus`、`vault::hosts`、AES 加解密、auto-type 执行（`enigo`）整体 `#[cfg(desktop)]`；桌面 IPC 命令与 manage 状态同步门控                             |
| 移动端文件 I/O    | `src-tauri/src/vault/file_io.rs`（新增）、`Cargo.toml`、`lib.rs`                                   | `tauri-plugin-fs` 解析 `content://`；`setup` 中 `capture_app` 注入 AppHandle（`#[cfg(mobile)]`）；`read_keyfile` / `prepare_local_open` / `write_database_bytes` 统一走该模块，桌面分支保持原子写                                    |
| 安卓 TLS 信任     | `src-tauri/src/net.rs`（新增）、`Cargo.toml`、`remote/mod.rs`、`commands/{favicon,entries}.rs`     | reqwest 0.13 在 Android 默认使用未初始化的 `rustls-platform-verifier`，首次 TLS 即 panic；Android 改用 `webpki-root-certs` + `tls_certs_only`，S3/WebDAV/favicon/HIBP 统一走 `net::*_builder`，桌面/iOS 仍用平台验证器               |
| 安卓 SAF 选择器   | `src-tauri/plugins/saf/`（新增）、`lib.rs`、`capabilities/android.json`、`src/lib/services/saf.ts` | 本地 Tauri 插件：`ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT` + `takePersistableUriPermission`，返回 provider 的 `DISPLAY_NAME`；打开/新建/另存为改用 SAF，`content://` 可跨重启重开；桌面仍走 dialog 插件                      |

> 验证状态（2026-08-19）：
>
> - Windows host 的完整 `npm run verify` 已通过（格式、Svelte/Vite 前端、Rust 测试 373 个、clippy -D warnings）；它只能证明桌面分支与跨平台纯逻辑，不能替代 Android APK/真机证据。
> - GitHub Release 运行 [31619359721](https://github.com/muutot/SecPivot/actions/runs/31619359721) 已真实完成 `tauri android init`、release 签名配置和 Android Rust 交叉编译的大部分流程；失败发生在 vendored OpenSSL 安装阶段：`openssl-src` 调用了不存在的 `aarch64-linux-android-ranlib`。
> - 当前工作流显式选择同一 NDK、导出其 `llvm-ranlib` 为 `TARGET_RANLIB`、只安装 64 位 Android Rust targets（`aarch64-linux-android`、`x86_64-linux-android`），用 `--split-per-abi` 产出按 ABI 拆分的签名 release APK，再逐包 `apksigner verify` 并上传。此项修复仍需新的远端运行证明 APK 生成、签名与上传成功。（注：`TARGET_RANLIB` 导出现在已属冗余——S3/WebDAV 网络栈全部 rustls 化后不再有 openssl-src 交叉编译需求，见下文「打包与 CI」；工作流中保留该导出仅为历史兼容。）

> 验证状态（2026-10-20，移动端文件 I/O）：
>
> - Windows host 已通过 `cargo check`、`clippy -D warnings`、`cargo fmt --check`、`npm run check`、`npm run build`、`npm run test:rust`（454 通过）。
> - 本机未安装 NDK/clang，`cargo check --target aarch64-linux-android` 在 `aws-lc-sys` 的 cc 阶段失败（找不到 `aarch64-linux-android-clang`），本地无法编译移动端分支。
> - CI 已闭环编译与打包：[run 39409873118](https://github.com/muutot/SecPivot/actions/runs/39409873118) 的 `Build (Android APK)` 全绿，`cfg(mobile)` 分支在 `aarch64-linux-android` / `x86_64-linux-android` 上真实编译，v1.6.3 Release 已发布 arm64-v8a 与 x86_64 两个签名 APK。
> - 仍未闭环：`content://` 读写的**真机运行验证**（打开、另存为、挂载 SMB/WebDAV 后的保存行为），需在设备上按本文步骤实测。

> 验证状态（2026-10-20，Android TLS）：
>
> - 根因：reqwest 0.13.5 在未显式提供 roots 时启用 `rustls-platform-verifier`；Android 缺少 JNI 初始化与 Kotlin 组件，首次校验即 panic（`Expect rustls-platform-verifier to be initialized`），最终表现为 `远程列表任务异常: event loop thread panicked`。
> - 修复：`src-tauri/src/net.rs` 在 Android 上用 `webpki-root-certs` + `tls_certs_only` 固定 Mozilla roots，S3/WebDAV/favicon/HIBP 共用；桌面/iOS 仍走平台验证器。**代价**：Android 不信任用户安装的企业 CA。
> - Windows host 已通过 `cargo check`、`clippy -D warnings`、`cargo fmt --check`、`npm run check`、`npm run build`、`npm run test:rust`（454 通过）与前端测试；Android 分支仍需下一次 CI Android job 真实编译，真机需复测 S3/WebDAV 列表、favicon 与 HIBP 不再触发该 panic。

> 验证状态（2026-10-20，Android SAF 与最近文件）：
>
> - 打开/新建/另存为在 Android 上改为 `tauri-plugin-saf`：`ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT` 申请读写并调用 `takePersistableUriPermission`，provider 支持时授权跨重启；取消选择返回空对象，由前端按“未选择”处理。
> - 最近文件在 Android 上保留 `content://` 路径，并把 `DISPLAY_NAME` 存入 `general.recentFileNames`（键为路径，仅保留仍在 `recentFiles` 中的项，trim + 120 字符上限）；欢迎页与锁屏优先显示该名称，旧配置无映射时回退到 URI 末段。
> - 新建/另存为的 SAF intent 使用 `type="*/*"` + `EXTRA_MIME_TYPES`（`application/octet-stream`、`application/x-keepass2`）并只设置 `EXTRA_TITLE`；能否离开 Downloads 取决于 OEM DocumentsUI，仍可能被 provider 限制。
> - 本地无 NDK，插件 Kotlin/`cfg(target_os="android")` 分支只能由 CI Android job 编译；真机需验证打开、新建、另存为、重启后最近文件重开，以及 Google Drive 等 provider 的 display name。

## 后续必建（需工具链环境，按顺序执行）

> 验证状态（2026-11-09，SAF ACL）：
>
> - 用户确认安装版本为 v1.6.5，原始反馈“另存为失败: Command plugin:safe|create_document not allowed by ACL”，选择器未启动。当前源码注册/调用的是 `saf`，反馈中 `safe` 的拼写差异仍需安装包或原始设备日志核实。源码另有已确认权限问题：Android capability 误用 `screens`，Tauri 会忽略该字段，导致 `saf:default` 没有匹配主窗口；已改为 `windows: ["main"]`。
> - `src-tauri/tests/android_acl.rs` 使用 Tauri 实际 ACL 解析器和构建生成的插件权限清单，在 Windows host 上按 Android target 复现修复前拒绝、验证修复后打开/新建命令可用于本地主窗口，并验证不授权其他窗口、远程网页或桌面/iOS。
> - ACL 修复需重新构建安装 APK 后生效；系统选择器及提供商写入仍需真机复测。

### 1. 工具链与脚手架

- 安装 JDK（`tauri android init` 硬要求）、Android Studio / SDK（platform + build-tools + NDK）、Gradle
- `rustup target add aarch64-linux-android x86_64-linux-android`（仅 64 位 ABI，见「打包与 CI」）
- `npx tauri android init` 生成 `src-tauri/gen/android/`（该目录常被 `.gitignore` 排除）
- 签名：CI 采用「构建 / 签名」两段式（见「打包与 CI」）。本地手动出签名包时用 `scripts/configure-android-signing.ps1`（见下）

### 2. 完成真实 Android 编译（继续暴露剩余裁剪点）

- `npm run build` 产出前端静态资源，再 `npx tauri android build --apk --ci --split-per-abi --target aarch64 --target x86_64`
- Tauri 2 Android 构建默认会编译四个 Rust targets；当前工作流用 `--split-per-abi --target aarch64 --target x86_64` 限定 64 位 ABI 并产出按 ABI 拆分的 APK
- 已完成的后端依赖裁剪（不进 Android 编译）：`enigo`、`keyring`、`tauri-plugin-global-shortcut`、`tungstenite`、`num-bigint`、`aes/cbc/cipher/block-padding` 移入 `Cargo.toml` 的桌面专属 target 段；bridge/RPC loopback 服务、TCATO、凭据存储、全局热键、auto-type 执行、DPAPI 相关命令已用 `#[cfg(desktop)]` 隔离。S3 传输已自研 SigV4（`remote/s3.rs`，rustls 无原生依赖），`rust-s3` 与 `openssl` vendored 段已彻底移除；`tokio` 与 `reqwest`（rustls）保持跨平台共享，Android 编译不再链接任何 OpenSSL
- 真机/模拟器跑通首包

### 3. 前端移动端适配

- ~~小屏响应式断点~~（已落地 `@media (max-width: 720px)`：主界面单列堆叠、分组抽屉、条目摘要列表、详情全屏覆盖；设置页一级分类改为抽屉并让正文占满宽度；移动平台默认将另存为、详情、安全报告、导出和设置收纳到“更多”菜单，且可在“通用 → 布局”调整密度；新增分组位于分组标题栏的展开/折叠按钮左侧）
- 触控精度（拖动阈值 / 长按）：长按菜单已按 `pointerdown` 坐标传给页面菜单状态，并对 Android WebView 的 `(0, 0)` 原生 `contextmenu` 回退到触发元素位置；长按手势由 `src/lib/utils/long-press.ts` 统一管理——一次手势只武装一个长按，事件冒泡到空白区域处理器时不会重复武装，因此单击只选中条目并打开详情面板，只有长按才弹出菜单，且长按条目行弹出的是条目菜单而非空白区菜单；拖动阈值仍待真机调优
- 隐藏自绘标题栏 `WindowControls`、适配无窗口语义
- Android 返回键（返回上层 / 退出）
- ~~文件选择走 SAF / 文档选择器（打开、新建、另存 `.kdbx`；`tauri-plugin-dialog` 在安卓的等价实现）~~（已落地：`vault/file_io.rs` 用 `tauri-plugin-fs` 经 ContentResolver 读写 `content://`；打开/新建/另存为改用 `tauri-plugin-saf` 的 `ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT` 并 `takePersistableUriPermission`，`content://` 可写入最近文件并跨重启重开；桌面仍走 `std::fs` / 原子写与 dialog 插件。**限制**：provider 可能拒绝持久授权或撤销旧授权，失败时需重新选择；`ACTION_CREATE_DOCUMENT` 的初始位置仍由系统 DocumentsUI 决定。真机读写仍待验证。）
- 附件保存 / CSV 导出 / 文本导入（`write_attachment_file`、`write_csv_file`、`read_text_file`）仍直接使用 `std::fs`，移动端尚不可用，后续接入 `vault/file_io.rs`
- 状态栏 / 沉浸式：主界面 `.app-shell.mobile` 与 `/settings` 的 `.settings-shell` 均用 `env(safe-area-inset-*)` 预留系统栏；主界面隐藏 30px 桌面拖拽条，设置对话框填充安全区内容盒。软键盘弹起 / 捏合缩放仍待适配，真机视觉验证待补
- SAF 写入的事务性：SAF 无 rename，文档只能原地覆写（先截断），provider 中途报错就会留下空/半截库文件，而移动端该文档常常是用户唯一副本。`vault/file_io.rs` 因此先读出旧内容 → 写入 → 读回比对 → 失败即回滚旧内容（桌面走 `util::atomic_write`，天然原子）。provider 返回陈旧副本会让校验失败，此时保留旧库并把保存报为失败，而不是静默丢数据；真机回滚路径仍待验证

### 4. 安全语义裁剪 + 权限

- 2026-11-09：SAF 保存读取旧内容失败时直接取消，禁止把读取错误当成空文件；回滚也读回验证。URI 流在分配过程中限制读取量（库 512 MiB、密钥文件 10 MiB）。Windows 故障注入测试覆盖共享事务助手，不能代替真机提供商验证。

- 禁用 auto-type、TCATO、托盘、全局热键（命令层与入口一并裁掉）
- **后台 / 设备锁自动锁定**（`onPause` 清内存密钥）——移动端核心安全点
- 剪贴板定时清除在安卓的语义适配
- 敏感配置加密：桌面的 DPAPI → 安卓用 Android Keystore；已有明文降级路径
- `AndroidManifest` 权限声明（远端 S3/WebDAV 需 `INTERNET`；读写走 SAF 则无需存储权限）

### 5. 打包与 CI

- 跟随桌面发布：`android` 与 Windows `build` 和 `verify` 并行（均依赖廉价 `validate` 门禁），Linux 构建 `aarch64` / `x86_64` 未签名 split APK，映射 `arm64`→`arm64-v8a` 后先 stage 为 `SecPivot-<version>-android-<abi>.apk`，以 `android-unsigned-apk` 工件传递。canonical 仓库默认构建，`ANDROID_RELEASE_ENABLED=false` 可跳过，fork 自动跳过；缺签名 secrets 时失败。
- **独立签名任务**：`sign-android` 依赖 `validate` / `android`，无 checkout、构建源码或依赖脚本，仅用固定 SHA 的工具动作、build-tools 35.0.0 和内联命令处理 APK。四项签名 secrets 仅注入签名 step；keystore 在私有 runner temp 目录，EXIT/INT/TERM 均清理；密码参数使用 `env:` 引用。必须恰有两个命名正确的 APK，各自包含 `lib/<abi>/libsecpivot_desktop_lib.so` 且无其他 native ABI，临时签名结果与最终 staged APK 均须通过 `apksigner verify`。最后对最终文件生成 SHA256SUMS，再以 `android-release-apk` 工件交给 `publish-android`；上传依赖验证、Windows draft 和签名成功。合成 shell 回归覆盖秘密缺失、验签失败、ABI 缺失/混入和 keystore 清理，不能代替真实 CI/APK 验证。
- **stage 必须先于签名，之后不得再从 Gradle 输出目录复制**，否则未签名原件覆盖同名签名产物（v1.6.5 曾因此安装失败）。Windows 发布说明随工件传递，发布任务也无 checkout。Android 仍使用 `CARGO_PROFILE_RELEASE_*` 的发布优化覆盖，x86-64 专用的 `target-cpu=x86-64-v3` 不施加于 Android。
- Gradle 仓库回退：`tauri android init` 生成的 `gen/android`（gitignored）在 `:buildSrc` 配置阶段曾只查 `plugins.gradle.org/m2`，其 Kotlin 插件 POM 与父 BOM 不同步时（`kotlin-gradle-plugin:2.0.21` 可解析而 `kotlin-gradle-plugins-bom:2.0.21` 缺失）整个构建失败。`release.yml` 在 init 之后、任何 gradlew 调用之前运行 `scripts/patch-android-gradle-repos.ps1`，幂等地给 buildSrc / settings（pluginManagement + dependencyResolutionManagement）/ 根构建文件的每个 `repositories` 块补齐 `google()` + `mavenCentral()` + `gradlePluginPortal()`。
- 所有网络栈均为 rustls（reqwest blocking + rustls），Android 不再编译任何 OpenSSL/native-tls/openssl-sys 依赖，也不再有 openssl-src 交叉编译与 `TARGET_RANLIB` 需求。
- 本地手动签名配置：`scripts/configure-android-signing.ps1` 要求全部四个 secrets（`ANDROID_KEYSTORE_BASE64` / `ANDROID_KEYSTORE_PASSWORD` / `ANDROID_KEY_PASSWORD` / `ANDROID_KEY_ALIAS`），解码 keystore 并幂等 patch `build.gradle.kts`（Tauri 2 模板默认无 signingConfigs）；Gradle 在构建时从环境读取存储密码与独立 key password，缺任一项立即失败，不把密码写入 `keystore.properties`，并清理旧版脚本遗留的该文件。**该脚本只用于本地手动出包，CI 不再调用它**——CI 的签名在独立 `sign-android` job 中完成（keystore 解码到 runner 临时目录，就地签名 staged APK）。需先在任意有 JDK 的机器生成 keystore：
  ```
  keytool -genkey -v -keystore upload-keystore.jks -storetype JKS -keyalg RSA -keysize 2048 -validity 10000 -alias upload
  [Convert]::ToBase64String([IO.File]::ReadAllBytes("upload-keystore.jks"))   # 存入 ANDROID_KEYSTORE_BASE64
  ```

### 6. 文档与版本

- 更新 `skills/secpivot-dev/SKILL.md` 路由表与 `references/project-structure.md` 增加 android/移动面
- 移动端首发版本走 `version-release` 技能

## 环境阻塞记录

- **cargo 依赖线性已解**：本机系统代理 `127.0.0.1:51400` 放行 `github` 与 `crates.io`，缺失的 `keepass 0.13.20` 已补齐，后端全绿（见上）。
- **Android SDK/NDK 无法在本机取得**：该代理为白名单制，**不含 `dl.google.com`**（`google.com`/`dl.google.com` 均不可达），而 Android 官方 SDK/NDK 只发布在该域；国产镜像（腾讯/阿里/清华）亦不可用。故 JDK/SDK/NDK 无法安装，`tauri android init` 与 `tauri android build` 不能在本机执行。GitHub-hosted runner 已提供真实交叉编译证据，但 APK 与真机行为仍需新的成功 CI 运行和设备验证闭环。

阻塞项需在具备工具链与网络的机器上按 1–6 执行即可闭环。
