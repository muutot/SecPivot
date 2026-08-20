# SecPivot Desktop v1.3.1

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-20

---

## 详情面板编辑体验

- **笔记编辑器置底固定** — 笔记字段固定在底部、其余字段滚动，并带防抖自动保存 | [`07e04a9`](https://github.com/muutot/SecPivot/commit/07e04a9) · [`286919e`](https://github.com/muutot/SecPivot/commit/286919e)
- **右键内联编辑字段** — 受保护字段先揭示后再允许编辑，未揭示时拒绝内联修改 | [`75eee88`](https://github.com/muutot/SecPivot/commit/75eee88) · [`10872c1`](https://github.com/muutot/SecPivot/commit/10872c1)
- **网址描述仅在有值时显示** — 去除协议前缀的 URL 描述只在存在时展示 | [`3fa33e2`](https://github.com/muutot/SecPivot/commit/3fa33e2) · [`313f55e`](https://github.com/muutot/SecPivot/commit/313f55e)

## 界面交互

- **全局提示 toast** — 新增全局瞬时提示宿主 | [`8e1ef73`](https://github.com/muutot/SecPivot/commit/8e1ef73)
- **单一可见上下文菜单** — 应用级只保留一个可见右键菜单，主界面抑制原生菜单 | [`162b497`](https://github.com/muutot/SecPivot/commit/162b497)
- **展开/折叠合并为单按钮** — 全部展开/全部折叠收敛为一个切换按钮，且不再标记库为未保存 | [`4f52d53`](https://github.com/muutot/SecPivot/commit/4f52d53) · [`726289f`](https://github.com/muutot/SecPivot/commit/726289f)
- **拖拽区域扩展** — 设置对话框与主工具栏拖拽区加大 | [`0ec0b23`](https://github.com/muutot/SecPivot/commit/0ec0b23)
- **外观开关迁入显示页** — 分组图标/箭头开关与紧凑模式解耦 | [`6c5cf17`](https://github.com/muutot/SecPivot/commit/6c5cf17)
- **中键自动滚动前置取消** — 中键自动滚动全局在触发前取消 | [`c6647ea`](https://github.com/muutot/SecPivot/commit/c6647ea)

## 远程传输重构

- **自研 SigV4 替代 rust-s3** — S3 远程传输改用基于 rustls reqwest 的手写 SigV4，移除 rust-s3 依赖 | [`cb10467`](https://github.com/muutot/SecPivot/commit/cb10467)

## Android 发布

- **仅 64 位 ABI** — APK 按 64 位 ABI 拆分并移除 32 位目标 | [`700f483`](https://github.com/muutot/SecPivot/commit/700f483)
- **桌面模块按平台门控** — 仅桌面依赖从移动端构建中排除 | [`b9807af`](https://github.com/muutot/SecPivot/commit/b9807af)
- **APK 极端优化** — Android 构建套用发布级优化配置 | [`14ca451`](https://github.com/muutot/SecPivot/commit/14ca451)

## 工程规范

- **提交前强制 format:check** — 提交前执行格式检查，并同步刷新 issue 跟踪 | [`674d8e9`](https://github.com/muutot/SecPivot/commit/674d8e9) · [`a195171`](https://github.com/muutot/SecPivot/commit/a195171) · [`d9f9969`](https://github.com/muutot/SecPivot/commit/d9f9969) · [`d02f013`](https://github.com/muutot/SecPivot/commit/d02f013)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.3.1_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.3.1-portable.zip`(由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`)
- **Android APK**: `app-release.apk`(release 签名，由 release 工作流的 `android` job 在 Linux 上并行构建)
