# SecPivot Desktop v1.6.4

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-10-21

---

## Android 支持

- **SAF 文件选择器** — 新增本地 `tauri-plugin-saf`，打开、新建、另存为改用 `ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT`，并调用 `takePersistableUriPermission` 持久化授权 | [`320cc32`](https://github.com/muutot/SecPivot/commit/320cc328)
- **最近文件跨重启** — Android 上保留 `content://` 路径，并把 provider 的 `DISPLAY_NAME` 写入 `general.recentFileNames`；欢迎页与锁屏显示可读文件名，授权有效时可直接重开 | [`320cc32`](https://github.com/muutot/SecPivot/commit/320cc328)
- **远程列表 TLS 崩溃修复** — Android 显式使用 `webpki-root-certs` + `tls_certs_only`，不再触发未初始化的 `rustls-platform-verifier` panic（此前表现为“远程列表任务异常”）；S3/WebDAV/favicon/HIBP 统一走共享 builder | [`320cc32`](https://github.com/muutot/SecPivot/commit/320cc328)
- **状态栏与安全区** — 启用 `viewport-fit=cover`，用 `env(safe-area-inset-*)` 预留系统栏，并隐藏移动端桌面拖拽条 | [`320cc32`](https://github.com/muutot/SecPivot/commit/320cc328)
- **移动端编译修复** — 修复 Android 目标下 auto-type 取消调用的门控，并按值传递 `tauri-plugin-fs` 的 `OpenOptions` | [`7074113`](https://github.com/muutot/SecPivot/commit/70741133)

## 打包与 CI

- **Windows 发布者** — NSIS 安装包的发布者与公司信息统一为 Muu | [`78660ce`](https://github.com/muutot/SecPivot/commit/78660ce7)
- **发布流程加速** — 并行运行桌面与 Android 构建，移除重复的极限优化编译，并让 Android 发布等待两个构建产物后再上传 | [`70c9b60`](https://github.com/muutot/SecPivot/commit/70c9b603)
- **Android 工具链** — 移除已从托管 runner SDK 下线的旧 `tools` 包，只安装 `platform-tools` | [`04b5b6d`](https://github.com/muutot/SecPivot/commit/04b5b6d5)
- **文档** — 记录 APK 构建证据、TLS 信任取舍、SAF 持久授权与移动端编译陷阱 | [`6b3fddc`](https://github.com/muutot/SecPivot/commit/6b3fddca) · [`320cc32`](https://github.com/muutot/SecPivot/commit/320cc328)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.6.4_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.6.4-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 64 位签名 release APK（`arm64-v8a` / `x86_64`），由 CI 的 Android job 在 Linux 上构建并逐包校验
