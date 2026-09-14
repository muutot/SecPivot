# SecPivot Desktop v1.6.1

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-09-14

---

## Android 发布修复

- **按 ABI 拆分的 APK 正确命名** — release 工作流此前从 APK 的直接父目录取 ABI，得到的是 `release` 而不是 `arm64`/`x86_64`，两个 ABI 于是撞名成同一个 `SecPivot-<version>-android-release.apk`，只有循环里最后一个（x86_64）存活。结果 v1.6.0 发布的安装包在 arm64 手机上无法安装，系统提示“未适配最新的64位系统”。现在改为从 cargo-mobile2 的 `apk/<arch>/release/` 取上两级 arch flavor 并映射为 `arm64-v8a`/`x86_64`，同时断言包内确实含对应的 `lib/<abi>/`，缺少任一 ABI 资产即让发布失败 | [`1376170`](https://github.com/muutot/SecPivot/commit/1376170)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.6.1_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.6.1-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 按 64 位 ABI 拆分签名的 release APK（`arm64-v8a`/`x86_64`，由 release 工作流 android job 在 Linux 并行构建）
