# Android 安卓支持评估与落地清单

SecPivot（Svelte 5 + Tauri 2 + Rust）对安卓平台的可移植性评估、已改动仓库文件、以及剩余落地步骤。本文档同时作为"必须由具备工具链与网络的环境执行"的任务清单——**凡需 Android 构建/真机的部分，在后端 cargo 已验证、但缺 Android SDK（本机代理不可达 `dl.google.com`）的环境下不会标记为已完成**。

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

> 验证状态（更新于当前会话）：
>
> - 前端 `npm run check` 0 错误、`npm run build` 成功、新增 JSON 已按 prettier 格式化。
> - 后端经本机系统代理（`127.0.0.1:51400`，放行 `github`/`crates.io`）补齐缺失依赖 `keepass 0.13.20` 后，`cargo test` **271 passed / 0 failed**、clippy `-D warnings` **0 警告**、`cargo fmt --check` 通过——已证明上表的 `lib.rs` 门控与 capability/config 可编译（Windows host）。
> - `rustup target add aarch64-linux-android` 已完成；但 `cargo check --target aarch64-linux-android` 被 `aws-lc-rs`（`rust-s3`/`reqwest` 的 rustls 依赖）的 C 编译阻断，需 Android NDK 的 `aarch64-linux-android-clang` 才能继续。

## 后续必建（需工具链环境，按顺序执行）

### 1. 工具链与脚手架

- 安装 JDK（`tauri android init` 硬要求）、Android Studio / SDK（platform + build-tools + NDK）、Gradle
- `rustup target add aarch64-linux-android`
- `npx tauri android init` 生成 `src-tauri/gen/android/`（该目录常被 `.gitignore` 排除）
- 配置签名 keystore，准备 release 出包

### 2. 一次真实 Android 编译（会暴露全部剩余裁剪点）

- `npm run build` 产出前端静态资源，再 `npx tauri android build`
- 排查后端依赖在安卓 toolchain 的可编译性：`enigo`、`keyring`、`tungstenite`、`rust-s3`、`windows-sys`
- 补齐 `#[cfg(desktop)]` 遗漏：`platform/autotype`、bridge/RPC loopback 服务、DPAPI、TCATO 命令、`localStorage` 演示回退、桌面剪贴板语义等
- 真机/模拟器跑通首包

### 3. 前端移动端适配

- ~~小屏响应式断点~~（已落地 `@media (max-width: 720px)`：主界面单列堆叠、分组抽屉、详情全屏覆盖；设置页一级分类改为抽屉并让正文占满宽度）
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

- 复用 `.github/workflows/android.yml`（本仓库已备，见 CI 改动）；需自有 SDK 的 runner 与签名配置

### 6. 文档与版本

- 更新 `skills/secpivot-dev/SKILL.md` 路由表与 `references/project-structure.md` 增加 android/移动面
- 移动端首发版本走 `version-release` 技能

## 环境阻塞记录

- **cargo 依赖线性已解**：本机系统代理 `127.0.0.1:51400` 放行 `github` 与 `crates.io`，缺失的 `keepass 0.13.20` 已补齐，后端全绿（见上）。
- **Android SDK/NDK 无法在本机取得**：该代理为白名单制，**不含 `dl.google.com`**（`google.com`/`dl.google.com` 均不可达），而 Android 官方 SDK/NDK 只发布在该域；国产镜像（腾讯/阿里/清华）亦不可用。故 JDK/SDK/NDK 无法安装，`tauri android init` 与 `tauri android build` 以及安卓 target 的真实编译（cargocling）在本机无法执行，APK 与真机行为需在**可达 `dl.google.com` 且有 Android 工具链的机器/CI**（如 GitHub-hosted runner，见 `.github/workflows/android.yml`）上闭环。

阻塞项需在具备工具链与网络的机器上按 1–6 执行即可闭环。
