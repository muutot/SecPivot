# SecPivot Desktop v1.4.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-25

---

## 安全与隐私

- **剪贴板定时清除兜底** — 后端强制执行计划性剪贴板擦除安全网 | [`3b43a87`](https://github.com/muutot/SecPivot/commit/3b43a87)
- **HIBP 副本清零** — 泄露库检查完成后立即清零密码副本 | [`6789592`](https://github.com/muutot/SecPivot/commit/6789592)
- **桥接连率限制** — 对未认证的 associate 请求实施速率限制 | [`e4cd572`](https://github.com/muutot/SecPivot/commit/e4cd572)

## RPC 会话管理

- **SRP 会话密钥超时与活动连接管理** — 新增会话密钥超时机制和实时连接管理 | [`78268da`](https://github.com/muutot/SecPivot/commit/78268da) · [`30b9629`](https://github.com/muutot/SecPivot/commit/30b9629)
- **会话卡片统一风格** — RPC 会话卡片与设置原语对齐，采用共享设计令牌与等宽字体变量 | [`3867044`](https://github.com/muutot/SecPivot/commit/3867044) · [`fd6c01f`](https://github.com/muutot/SecPivot/commit/fd6c01f)

## 条目与历史

- **全库变更时间线** — 新增覆盖整个库的变更历史时间线对话框 | [`2bd41f9`](https://github.com/muutot/SecPivot/commit/2bd41f9)
- **后端版本差异计算** — 历史版本 diff 移入后端计算并提供分组标签页视图 | [`4128ec3`](https://github.com/muutot/SecPivot/commit/4128ec3)
- **列表密码列遮罩** — 密码列默认遮罩显示，点击揭示 | [`6eaf43e`](https://github.com/muutot/SecPivot/commit/6eaf43e)
- **条目大小列** — 新增与 KeePass 兼容的条目大小列，并纳入默认列配置 | [`391db1b`](https://github.com/muutot/SecPivot/commit/391db1b) · [`4bb0264`](https://github.com/muutot/SecPivot/commit/4bb0264)
- **TOTP 截图扫码** — 支持从屏幕截图扫描二维码/条码读取 TOTP 种子 | [`f7aed40`](https://github.com/muutot/SecPivot/commit/f7aed40)

## 导入导出与自动类型

- **KeePass XML 导出** — 新增遵循保护值约定的 XML 导出 | [`96045c9`](https://github.com/muutot/SecPivot/commit/96045c9)
- **自定义字符串字段作 {REF} 目标** — 自动类型 {REF:...} 现支持引用自定义字符串字段 | [`52620a7`](https://github.com/muutot/SecPivot/commit/52620a7)

## 便携化

- **便携数据根目录** — 数据根目录解析到可执行文件旁 | [`68af85a`](https://github.com/muutot/SecPivot/commit/68af85a)

## 稳定性与体验修复

- **模态焦点圈定** — Tab 焦点锁定在对话框内并还原到触发元素 | [`fa06986`](https://github.com/muutot/SecPivot/commit/fa06986)
- **附件 HTML5 拖放** — 关闭原生拖放以启用 HTML5 文件拖放 | [`9c09e1b`](https://github.com/muutot/SecPivot/commit/9c09e1b)
- **编辑后刷新缓存** — 条目编辑后重新加载缓存的历史与存储 | [`df1fb5c`](https://github.com/muutot/SecPivot/commit/df1fb5c)
- **内联密码强度提示** — 强度随标签行内展示，移除重复强度计 | [`0820f76`](https://github.com/muutot/SecPivot/commit/0820f76)
- **调用失败可见化** — fire-and-forget invoke 失败不再被静默吞掉 | [`7365c19`](https://github.com/muutot/SecPivot/commit/7365c19)
- **降级代替中止** — 远程客户端初始化与 rpc socket 克隆失败时降级处理 | [`41e0374`](https://github.com/muutot/SecPivot/commit/41e0374)
- **拒绝未知存储类型** — 不再静默按 S3 构建未知存储配置 | [`ab46078`](https://github.com/muutot/SecPivot/commit/ab46078)
- **移动端修复** — 桌面专用 QR 命令门控、窄布局滚轮滚动高度链恢复 | [`89d5cca`](https://github.com/muutot/SecPivot/commit/89d5cca) · [`eb78c4b`](https://github.com/muutot/SecPivot/commit/eb78c4b)

## 前端架构重构

- **组件与服务抽取** — 工具栏、条目编辑器流程、分组流程、搜索过滤管线、选择模型、收藏图标下载、列配置、面板布局状态机、导入导出编排等逐一抽离为组合式函数与服务层 | [`1095127`](https://github.com/muutot/SecPivot/commit/1095127) · [`101e393`](https://github.com/muutot/SecPivot/commit/101e393) · [`10ac1fb`](https://github.com/muutot/SecPivot/commit/10ac1fb) · [`6c09752`](https://github.com/muutot/SecPivot/commit/6c09752) · [`04ae103`](https://github.com/muutot/SecPivot/commit/04ae103) · [`a6cda02`](https://github.com/muutot/SecPivot/commit/a6cda02) · [`7703dc5`](https://github.com/muutot/SecPivot/commit/7703dc5) · [`343148c`](https://github.com/muutot/SecPivot/commit/343148c) · [`c4a114e`](https://github.com/muutot/SecPivot/commit/c4a114e) · [`b87e7ef`](https://github.com/muutot/SecPivot/commit/b87e7ef)
- **键盘调度通用化** — 快捷键匹配与分发迁移到带单元测试的服务层 | [`6904737`](https://github.com/muutot/SecPivot/commit/6904737) · [`0e5c3ab`](https://github.com/muutot/SecPivot/commit/0e5c3ab)
- **上下文菜单纯函数化** — 菜单项构建器改为纯函数并配套单元测试 | [`60979d7`](https://github.com/muutot/SecPivot/commit/60979d7)

## 构建与供应链

- **SHA256 校验发布** — Windows 与 Android 构件附带校验和文件 | [`842c687`](https://github.com/muutot/SecPivot/commit/842c687)
- **工具链固定** — 锁定 Rust 1.96.0 与 Node 24 保证构建可复现 | [`92e9183`](https://github.com/muutot/SecPivot/commit/92e9183)
- **Actions 按 SHA 固定** — 全部 GitHub Actions 固定到提交哈希并声明最小 contents:read 权限 | [`440dd42`](https://github.com/muutot/SecPivot/commit/440dd42) · [`8e2d8aa`](https://github.com/muutot/SecPivot/commit/8e2d8aa)
- **测试加固** — Tauri 内部 seam 的运行时 IPC 粘合测试、跨层冲突哨兵契约测试 | [`6ca1bf4`](https://github.com/muutot/SecPivot/commit/6ca1bf4) · [`fdeb1de`](https://github.com/muutot/SecPivot/commit/fdeb1de)
- **文档更新** — 库存储契约文档、README 前置条件与安全报告渠道、完整审计报告 | [`6fd5b2d`](https://github.com/muutot/SecPivot/commit/6fd5b2d) · [`933e5b2`](https://github.com/muutot/SecPivot/commit/933e5b2) · [`adcd1f3`](https://github.com/muutot/SecPivot/commit/adcd1f3)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.4.0_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.4.0-portable.zip`(由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`)
- **Android APK**: `app-release.apk`(release 签名，四 ABI 通用包，由 release 工作流的 `android` job 在 Linux 上并行构建)
