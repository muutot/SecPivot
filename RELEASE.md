# SecPivot Desktop v1.6.5

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-10-21

---

## 发布链路加固

- **发布身份绑定标签提交** — `validate` job 除比对三处版本外，还要求当前 checkout 的提交就是该标签指向的提交；手动 dispatch 到已发布版本的分支会被直接拒绝，不再能用未发布提交覆盖已发布产物 | [`16251c9`](https://github.com/muutot/SecPivot/commit/16251c9f)
- **构建 job 不再持有写权限令牌** — Windows 构建改为 `contents: read` + `persist-credentials: false`，移除同时构建与发布的 `tauri-action`，新增只下载产物、执行 `gh release create --draft` 的 `publish` job；任何 npm 依赖或 crate 的 `build.rs` 都拿不到可写令牌 | [`ff84112`](https://github.com/muutot/SecPivot/commit/ff841126)
- **Android 签名与构建分离** — 构建 job 环境中不再出现任何签名 secret，独立的 `sign-android` job 把 keystore 解码到 runner 临时目录后用 `apksigner` 重签并逐包校验，发布签名身份不再与第三方构建代码同域 | [`175d72d`](https://github.com/muutot/SecPivot/commit/175d72d7)
- **验证门禁覆盖 Windows 产物** — `build` 改为依赖 `verify`，测试全红时不再产出可发布的草稿 | [`ff84112`](https://github.com/muutot/SecPivot/commit/ff841126)
- **发布脚本健壮性** — regenerate 流程先校验发布说明再改写历史（并暂存脏文件保护 rebase）；已有标签必须指向发布提交；changelog 基线改用「已合并到 HEAD 的最新标签」；npm lockfile 版本随版本提升同步（并纳入发布文件白名单）；预发布标签也能触发 release 工作流 | [`75d4aaf`](https://github.com/muutot/SecPivot/commit/75d4aaf6)
- **Gradle 仓库补丁改为按括号配对定位块体** — 旧实现用「向后 1200 字符窗口」判断块内是否已有 `google()` / `mavenCentral()`，会读到下一个块的内容而误判「无需修改」，正是它本要防的 Kotlin 插件解析失败；现已按大括号配对定位并支持单行块，附 fixture 测试 | [`4f2fa12`](https://github.com/muutot/SecPivot/commit/4f2fa124)

## Android 与移动端

- **SAF 写入失败自动回滚** — SAF 无 rename，文档只能原地截断覆写，provider 中途报错会留下空/半截库文件（移动端常常是唯一副本）。现在写入前先读出旧内容，写入后读回比对，失败即回滚旧内容并给出明确提示 | [`e05aea9`](https://github.com/muutot/SecPivot/commit/e05aea94)
- **非持久授权文档不再进入最近文件** — provider 只给会话级 URI 时，插件回报 `persisted=false`，该路径不写入最近文件，避免重启后点开必然失败的条目 | [`a8d3c5f`](https://github.com/muutot/SecPivot/commit/a8d3c5f2)
- **长按菜单跟随条目位置** — Android WebView 可能发出坐标为 `(0, 0)` 的原生 `contextmenu`，且合成的 `MouseEvent` 可能丢失坐标；现在记录 `pointerdown` 坐标并单独传给页面菜单状态，必要时回退到行/表头位置 | [`e9dc689`](https://github.com/muutot/SecPivot/commit/e9dc6899)
- **设置页预留系统栏** — `/settings` 路由包装层统一承担 `100dvh` 与 `env(safe-area-inset-*)`，设置对话框填充安全区内容盒，不再被状态栏遮挡 | [`1a23955`](https://github.com/muutot/SecPivot/commit/1a239556)

## 安全与稳定性

- **库文件与附件临时文件权限收紧** — 原子写用 `create_new` + 0600 创建临时文件（同时拒绝被预置的符号链接），不再把用户收紧过的库文件权限放大回 umask 默认值；解密后的附件临时文件在 Unix 上改为 0700 目录 + 0600 文件 | [`30a3958`](https://github.com/muutot/SecPivot/commit/30a39583)
- **远程 key 不再逃逸本地镜像目录** — 远端列表返回的 key 现在只保留一个安全文件名段，并断言目标仍在镜像目录内（Windows 下反斜杠与盘符前缀不再能穿越目录） | [`469e3bc`](https://github.com/muutot/SecPivot/commit/469e3bc9) · [`b491c54`](https://github.com/muutot/SecPivot/commit/b491c544)
- **下载与本地读取有体积上限** — 超时只限制时间、不限制内存；S3/WebDAV 下载改为先校验 `Content-Length` 再限长拷贝，本地库文件读取前置体积校验 | [`b83780d`](https://github.com/muutot/SecPivot/commit/b83780d8)

## 文档

- **2026-10-21 全量审计与修复状态** — 20 条发现逐条复核（其中 6 条候选被证伪排除、1 条更正触发场景），17 条已修复、1 条部分处理、2 条列为计划；报告与修复状态表见仓库 `audit-report-SecPivot-2026-10-21.md` | [`589e968`](https://github.com/muutot/SecPivot/commit/589e968d)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.6.5_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.6.5-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 64 位签名 release APK（`arm64-v8a` / `x86_64`），由 CI 的 `android` + `sign-android` job 在 Linux 上构建、签名并逐包校验
