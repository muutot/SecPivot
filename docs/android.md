# Android 安卓支持评估与落地清单

SecPivot（Svelte 5 + Tauri 2 + Rust）对安卓平台的可移植性评估、已改动仓库文件、以及剩余落地步骤。本文档同时作为"必须由具备工具链与网络的环境执行"的任务清单——**凡需 Android APK 或真机行为的部分，必须以真实工具链/设备证据为准；Windows 本机代理不可达 `dl.google.com`，只能由 GitHub-hosted Linux runner 等环境闭环**。

## 结论

- **核心可行**：Tauri 2 原生支持安卓；KeePass 打开/编辑/保存、TOTP、加密等纯逻辑（`keepass` crate、`crypto/`、`vault/`）跨平台直接复用。
- **代价**：需补工具链、一次真实 Android 编译（借此清瘦 desktop 专用依赖）、前端触控 / 文件选择 / 后台锁适配、APK 出包与 CI。
- **桌面零影响**：所有移动端改动已用 `#[cfg(desktop)]` / `#[cfg(mobile)]` / 平台限定 `capability` / `tauri.android.conf.json` 隔离；桌面分支行为不变，须在有网环境跑 `npm run verify` 回归确认。

## 已完成的仓库改动

| 改动              | 文件                                          | 说明                                                                                                                                                                                                                                 |
| ----------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 安卓最小 SDK 声明 | `src-tauri/tauri.conf.json`                   | `bundle.android.minSdkVersion: 24`；桌面 NSIS 分支不受影响                                                                                                                                                                           |
| 安卓独立包名      | `src-tauri/tauri.android.conf.json`（新增）   | `identifier: com.secpivot.mobile`；桌面仍为 `com.secpivot.desktop`                                                                                                                                                                   |
| 安卓能力          | `src-tauri/capabilities/android.json`（新增） | `platforms:["android"]` + `core/dialog/opener` 权限；Windows 下被过滤                                                                                                                                                                |
| 桌面功能门控      | `src-tauri/src/lib.rs`                        | 系统托盘 / 全局热键 / auto-type / TCATO 的 `register_global_hotkey`、`setup_tray`、`handle_global_hotkey`、`toggle_main_window`、`handle_close_requested` 及 import/常量加 `#[cfg(desktop)]`；builder 链重构为分步变量以支持条件编译 |

> 验证状态（2026-08-13）：
>
> - Windows host 的完整 `npm run verify` 已在本轮审查中通过；它只能证明桌面分支与跨平台纯逻辑，不能替代 Android APK/真机证据。
> - GitHub Release 运行 [31619359721](https://github.com/muutot/SecPivot/actions/runs/31619359721) 已真实完成 `tauri android init`、release 签名配置和 Android Rust 交叉编译的大部分流程；失败发生在 vendored OpenSSL 安装阶段：`openssl-src` 调用了不存在的 `aarch64-linux-android-ranlib`。
> - 当前工作流显式选择同一 NDK、导出其 `llvm-ranlib` 为 `TARGET_RANLIB`、安装四个 Android Rust targets，并只构建/验证签名后的 universal release APK。此修复仍需新的远端运行证明 APK 生成与上传成功。

## 后续必建（需工具链环境，按顺序执行）

### 1. 工具链与脚手架

- 安装 JDK（`tauri android init` 硬要求）、Android Studio / SDK（platform + build-tools + NDK）、Gradle
- `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`
- `npx tauri android init` 生成 `src-tauri/gen/android/`（该目录常被 `.gitignore` 排除）
- 签名 keystore 由 `scripts/configure-android-signing.ps1` 在 CI 读取 secrets 自动配置（见下）

### 2. 完成真实 Android 编译（继续暴露剩余裁剪点）

- `npm run build` 产出前端静态资源，再 `npx tauri android build --apk --ci`
- Tauri 2 universal APK 默认编译四个 Android targets；NDK 的 `toolchains/llvm/prebuilt/<host>/bin/llvm-ranlib` 必须通过 `TARGET_RANLIB` 提供给 `openssl-src`，否则会在 `make install_dev` 阶段退回不存在的 `<target>-ranlib`
- 排查后端依赖在安卓 toolchain 的可编译性：`enigo`、`keyring`、`tungstenite`、`rust-s3`、`windows-sys`
- 补齐 `#[cfg(desktop)]` 遗漏：`platform/autotype`、bridge/RPC loopback 服务、DPAPI、TCATO 命令、`localStorage` 演示回退、桌面剪贴板语义等
- 真机/模拟器跑通首包

### 3. 前端移动端适配

- ~~小屏响应式断点~~（已落地 `@media (max-width: 720px)`：主界面单列堆叠、分组抽屉、条目摘要列表、详情全屏覆盖；设置页一级分类改为抽屉并让正文占满宽度；移动平台默认将另存为、详情、安全报告、导出和设置收纳到“更多”菜单，且可在“通用 → 紧凑”关闭；新增分组位于分组标题栏的展开/折叠按钮左侧）
- 触控精度（拖动阈值 / 长按）
- 隐藏自绘标题栏 `WindowControls`、适配无窗口语义
- Android 返回键（返回上层 / 退出）
- 文件选择走 SAF / 文档选择器（打开、新建、另存 `.kdbx`；`tauri-plugin-dialog` 在安卓的等价实现）
- 状态栏 / 沉浸式 / 软键盘弹起 / 捏合缩放

### 4. 安全语义裁剪 + 权限

- 禁用 auto-type、TCATO、托盘、全局热键（命令层与入口一并裁掉）
- **后台 / 设备锁自动锁定**（`onPause` 清内存密钥）——移动端核心安全点
- 剪贴板定时清除在安卓的语义适配
- 敏感配置加密：桌面的 DPAPI → 安卓用 Android Keystore；已有明文降级路径
- `AndroidManifest` 权限声明（远端 S3/WebDAV 需 `INTERNET`；读写走 SAF 则无需存储权限）

### 5. 打包与 CI

- 跟随桌面发布：`release.yml` 的 `android` job 与桌面 `build` 并行（两者都只依赖 `verify`），在 **`ubuntu-latest`** 上构建四 ABI 的 **universal release APK**；工作流用 `apksigner verify` 校验签名，将精确命名的 APK 暂存为 Actions artifact，再由同时依赖 Windows/Android 构建成功的 `publish-android` job 上传 `SecPivot-<version>-android-universal.apk`。这样不会用固定超时轮询较慢的极限 LTO 桌面构建；APK 缺失、未签名、draft release 缺失或上传失败仍会使发布失败。
- Android 目标需编译 `openssl-sys`（rust-s3 的 native-tls 硬依赖，无特性开关可避）；NDK 不带 OpenSSL，故在 `Cargo.toml` 对 `cfg(target_os = "android")` 启用 `openssl = { features = ["vendored"] }`，由 openssl-src 交叉编译。该交叉编译只能发生在非 Windows host（OpenSSL 拒绝 Windows perl 路径格式），因此 `android` job 固定在 Linux。
- 签名配置：`scripts/configure-android-signing.ps1` 要求全部四个 secrets（`ANDROID_KEYSTORE_BASE64` / `ANDROID_KEYSTORE_PASSWORD` / `ANDROID_KEY_PASSWORD` / `ANDROID_KEY_ALIAS`），解码 keystore 并幂等 patch `build.gradle.kts`（Tauri 2 模板默认无 signingConfigs）；Gradle 在构建时从环境读取存储密码与独立 key password，缺任一项立即失败，不把密码写入 `keystore.properties`，并清理旧版脚本遗留的该文件。需先在任意有 JDK 的机器生成 keystore：
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
