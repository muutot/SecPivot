# SecPivot Desktop v1.6.3

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-10-20

---

## Android 支持

- **Release 默认构建 APK** — canonical 仓库的发布流程默认并行构建 `arm64-v8a` 与 `x86_64` 两个签名 APK 并上传到 draft release；fork 缺签名 secrets 时自动跳过，`ANDROID_RELEASE_ENABLED=false` 可显式关闭 | [`6544d6f`](https://github.com/muutot/SecPivot/commit/6544d6f4)
- **Android 文档 URI 读写** — 新增 `vault/file_io.rs`，通过 `tauri-plugin-fs` 解析文档选择器返回的 `content://`，本地库与密钥文件可在手机上打开、新建、另存；打开后直接保存受系统只读授权限制，需使用「另存为」 | [`ca8f2d9`](https://github.com/muutot/SecPivot/commit/ca8f2d9f)

## 凭据与密钥安全

- **远程凭据落盘加密** — S3/WebDAV 访问密钥改由操作系统钥匙串保存，配置文件中不再出现明文 | [`a87a458`](https://github.com/muutot/SecPivot/commit/a87a4582)
- **IPC 凭据脱敏** — 远程凭据经 IPC 输出时以占位符掩码，编辑时保留未改动的密钥 | [`630f774`](https://github.com/muutot/SecPivot/commit/630f774b)
- **认证比较加固** — 统一凭据生命周期并改为常量时间比较；RPC 的 SRP 证明校验同样使用常量时间比较 | [`813d227`](https://github.com/muutot/SecPivot/commit/813d2271) · [`457f732`](https://github.com/muutot/SecPivot/commit/457f7322)
- **主密码槽位清理** — 更换主密码/密钥文件时覆盖被替换的旧密码槽位 | [`89ad66a`](https://github.com/muutot/SecPivot/commit/89ad66ac)
- **配置与解锁边界** — Unix 下配置文件收紧为仅所有者可读写；解锁时清空欢迎页凭据，并在启用密钥文件时正确门控「记住密码」 | [`863c77f`](https://github.com/muutot/SecPivot/commit/863c77f6) · [`666ff95`](https://github.com/muutot/SecPivot/commit/666ff953)

## 数据完整性

- **CSV 公式注入防护** — 导出与导入双向中和 `=`/`+`/`-`/`@` 前缀，并收紧表头识别 | [`f1cc216`](https://github.com/muutot/SecPivot/commit/f1cc2168) · [`9b3fdaf`](https://github.com/muutot/SecPivot/commit/9b3fdfa0)
- **远程保存与合并** — 加固远程保存、合并并列项的取舍与临时文件生命周期；历史快照按修改时间去重 | [`182d922`](https://github.com/muutot/SecPivot/commit/182d9224) · [`c55eb31`](https://github.com/muutot/SecPivot/commit/c55eb314)
- **历史与回收站** — RPC/浏览器写入路径同样裁剪条目历史；回收站祖先判定改为递归 | [`956be1e`](https://github.com/muutot/SecPivot/commit/956be1e1) · [`bcc2317`](https://github.com/muutot/SecPivot/commit/bcc23179)
- **输入边界** — 拒绝导入中的负图标 ID，限制不可信输入长度，并修正只读计数器归属 | [`b8e16b8`](https://github.com/muutot/SecPivot/commit/b8e16b8d) · [`a5abeef`](https://github.com/muutot/SecPivot/commit/a5aebeef)

## 界面与交互

- **模态与快捷键** — Escape 只关闭最顶层模态；模态打开时暂停全局快捷键 | [`e1e7f79`](https://github.com/muutot/SecPivot/commit/e1e7f796) · [`15ad671`](https://github.com/muutot/SecPivot/commit/15ad6710)
- **竞态修复** — 关闭组件卸载竞态，条目创建改为原子操作；修复揭示密码、复制锁定与关闭标签页的陈旧视图缺口 | [`c58703b`](https://github.com/muutot/SecPivot/commit/c58703bb) · [`aff7893`](https://github.com/muutot/SecPivot/commit/aff7893e)
- **复制与验证码** — 剪贴板读取失败时不再清空内容；TOTP 刷新时清除「已复制」标记 | [`cba8412`](https://github.com/muutot/SecPivot/commit/cba84121) · [`6ea8398`](https://github.com/muutot/SecPivot/commit/6ea8398d)
- **外链与可达性** — 外部 URL 改为白名单跳转，加固表格拖放与导出路径；为行内操作和 TOTP 按钮补充无障碍标签 | [`46c9029`](https://github.com/muutot/SecPivot/commit/46c9029a) · [`e9a846a`](https://github.com/muutot/SecPivot/commit/e9a846a5)

## 稳定性

- **设置持久化** — 为持久化重试设置上限，并保护已保存搜索的规范化与重试 | [`a44488c`](https://github.com/muutot/SecPivot/commit/a44488ce) · [`a4a3afa`](https://github.com/muutot/SecPivot/commit/a4a3afa2)
- **配置锁** — 配置写入期间持有存储锁，避免并发覆盖 | [`935d459`](https://github.com/muutot/SecPivot/commit/935d4595)
- **取消归属** — 中止、探测与粘贴路径绑定到发起会话 | [`a25fcc9`](https://github.com/muutot/SecPivot/commit/a25fcc9a)
- **Auto-Type** — URL 主机匹配剥离 userinfo；窗口标题通配符改为带回溯的实现 | [`bb3deea`](https://github.com/muutot/SecPivot/commit/bb3deea6) · [`d4df2dd`](https://github.com/muutot/SecPivot/commit/d4df2ddf)

## 平台与 CI

- **Linux 目标** — 新增 Linux Rust CI job 并修复该目标下的 clippy；测试运行改为 `--no-fail-fast` | [`ee1d821`](https://github.com/muutot/SecPivot/commit/ee1d8213) · [`8326392`](https://github.com/muutot/SecPivot/commit/83263926) · [`c1c4dba`](https://github.com/muutot/SecPivot/commit/c1c4dbad)
- **测试门控** — 钥匙串相关测试在无 OS 钥匙串时跳过，DPAPI 双重包装测试限定 Windows，并在 `verify` 中加入 TS-Rust 设置契约检查 | [`46a2ece`](https://github.com/muutot/SecPivot/commit/46a2ece5) · [`f7f3a6a`](https://github.com/muutot/SecPivot/commit/f7f3a6a8) · [`fba79d7`](https://github.com/muutot/SecPivot/commit/fba79d7f) · [`689c6f8`](https://github.com/muutot/SecPivot/commit/689c6f85)
- **内部整理** — 移除被替代的瞬时谓词，集中保存错误分类；清理过时的提取来源文档 | [`f7ca600`](https://github.com/muutot/SecPivot/commit/f7ca6005) · [`ad4b83a`](https://github.com/muutot/SecPivot/commit/ad4b83a7) · [`2e7e748`](https://github.com/muutot/SecPivot/commit/2e7e748d)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.6.3_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.6.3-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 64 位签名 release APK（`arm64-v8a` / `x86_64`），由 CI 的 Android job 在 Linux 上构建并逐包校验
