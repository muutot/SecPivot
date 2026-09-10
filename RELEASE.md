# SecPivot Desktop v1.6.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-09-10

---

## 中英文双语界面

- **字典基建与语言选择** — `zh-CN` 为源、`en` 编译期对等校验，通用语只读 `general.language`，中文硬编码废止 | [`7695604`](https://github.com/muutot/SecPivot/commit/7695604) · [`58e21c4`](https://github.com/muutot/SecPivot/commit/58e21c4) · [`f879728`](https://github.com/muutot/SecPivot/commit/f879728)
- **工具栏/菜单/表格/树** — 工具栏、右键菜单、条目表头、分组树（含右键菜单与重命名）| [`9a6546f`](https://github.com/muutot/SecPivot/commit/9a6546f) · [`b93d0df`](https://github.com/muutot/SecPivot/commit/b93d0df) · [`2a36814`](https://github.com/muutot/SecPivot/commit/2a36814)
- **详情/编辑器/设置面板** — 条目详情、条目编辑器、通用/键盘/安全/数据库设置 | [`7a87f6c`](https://github.com/muutot/SecPivot/commit/7a87f6c) · [`004d675`](https://github.com/muutot/SecPivot/commit/004d675) · [`826b9d2`](https://github.com/muutot/SecPivot/commit/826b9d2) · [`8ac9a04`](https://github.com/muutot/SecPivot/commit/8ac9a04)
- **远程/桥接/关于/维护对话框** — 远程库、桥接审批、RPC 通道、关于、HIBP/favicon/高级搜索、历史/过期/相似/分组/附件/列配置/安全报告 | [`e57ab23`](https://github.com/muutot/SecPivot/commit/e57ab23) · [`5b0595c`](https://github.com/muutot/SecPivot/commit/5b0595c) · [`4c7d24b`](https://github.com/muutot/SecPivot/commit/4c7d24b)
- **欢迎页/树/标签页/TOTP/壳** — 数据库属性、分组选择、标签页、验证码组件、弹窗壳与右键菜单壳 | [`bd68235`](https://github.com/muutot/SecPivot/commit/bd68235) · [`85a12aa`](https://github.com/muutot/SecPivot/commit/85a12aa) · [`8ed1a43`](https://github.com/muutot/SecPivot/commit/8ed1a43)
- **服务错误参数化** — vault 浏览器守卫收敛为 `browser.unsupported + {feature}` 单模板，回收站等持久化默认名统一英文 | [`34ff335`](https://github.com/muutot/SecPivot/commit/34ff335) · [`7729971`](https://github.com/muutot/SecPivot/commit/7729971) · [`c36913c`](https://github.com/muutot/SecPivot/commit/c36913c)
- **主页面** — 全部 toast/确认框/导入导出/删除/复制/自动填充/对话框共 109 处 | [`4611b18`](https://github.com/muutot/SecPivot/commit/4611b18)
- **格式** — 迁移行 prettier 对齐 | [`9f8a8fc`](https://github.com/muutot/SecPivot/commit/9f8a8fc) · [`e87ff77`](https://github.com/muutot/SecPivot/commit/e87ff77)

## TCATO 两通道填充

- **TOTP 注入通道** — 新增 totp 注入 | [`21ada52`](https://github.com/muutot/SecPivot/commit/21ada52)
- **全局热键召唤与目标记忆** — 快捷键召唤覆盖层、按条目记住上次目标窗口 | [`59bb960`](https://github.com/muutot/SecPivot/commit/59bb960) · [`81bf1c3`](https://github.com/muutot/SecPivot/commit/81bf1c3)
- **多匹配选择模式** — 当前窗口多条目命中时选择注入 | [`66725a0`](https://github.com/muutot/SecPivot/commit/66725a0) · [`c97f655`](https://github.com/muutot/SecPivot/commit/c97f655)

## 性能

- **favicon** — 属性正则按名缓存、取消时释放连接池 | [`5e3ca63`](https://github.com/muutot/SecPivot/commit/5e3ca63) · [`9cd0896`](https://github.com/muutot/SecPivot/commit/9cd0896)
- **条目表** — 分隔行按稳定组 id、计数并入 display-rows 构建、display-rows 抽取 | [`bac187e`](https://github.com/muutot/SecPivot/commit/bac187e) · [`a9286de`](https://github.com/muutot/SecPivot/commit/a9286de) · [`1272e5e`](https://github.com/muutot/SecPivot/commit/1272e5e)
- **搜索** — 高级查询每派生编译一次 | [`6c2ee94`](https://github.com/muutot/SecPivot/commit/6c2ee94)

## 安全与取消语义

- **HIBP 摘要擦除** — 匹配后擦除派生摘要 | [`fb699bb`](https://github.com/muutot/SecPivot/commit/fb699bb)
- **凭据加固** — 保存密码守卫收紧 | [`069e666`](https://github.com/muutot/SecPivot/commit/069e666)
- **取消信号** — 被取代的任务按 epoch 停止，命令/HIBP/favicon 取消测试覆盖 | [`9e7bb41`](https://github.com/muutot/SecPivot/commit/9e7bb41) · [`808def1`](https://github.com/muutot/SecPivot/commit/808def1) · [`b41a533`](https://github.com/muutot/SecPivot/commit/b41a533) · [`afb5989`](https://github.com/muutot/SecPivot/commit/afb5989)

## 测试与回归

- **条目表/契约测试** — 虚拟范围与属性契约、对话框重置断言按内容匹配 | [`0ba3bcc`](https://github.com/muutot/SecPivot/commit/0ba3bcc) · [`efc4752`](https://github.com/muutot/SecPivot/commit/efc4752)
- **回归清单** — 陷阱清单与一键回归脚本 | [`ac6be75`](https://github.com/muutot/SecPivot/commit/ac6be75)

## 杂项

- **重构/文档** — 笔记复制分支删除、分组计数抽取、分隔线文档、HIBP 死代码删除 | [`70f3fa9`](https://github.com/muutot/SecPivot/commit/70f3fa9) · [`1e24df3`](https://github.com/muutot/SecPivot/commit/1e24df3) · [`2ed23c1`](https://github.com/muutot/SecPivot/commit/2ed23c1) · [`7c95d35`](https://github.com/muutot/SecPivot/commit/7c95d35) · [`dc8646a`](https://github.com/muutot/SecPivot/commit/dc8646a)
- **杂务** — clippy 与断言格式 | [`13f58ee`](https://github.com/muutot/SecPivot/commit/13f58ee) · [`1cd34a2`](https://github.com/muutot/SecPivot/commit/1cd34a2) · [`58e330f`](https://github.com/muutot/SecPivot/commit/58e330f)
- **Android 构建** — Gradle 仓库回退，保证 Kotlin 插件解析 | [`217ed9f`](https://github.com/muutot/SecPivot/commit/217ed9f)
- **README** — 按 Clipboard 展示格式刷新 | [`4d0e33a`](https://github.com/muutot/SecPivot/commit/4d0e33a)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.6.0_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.6.0-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 按 64 位 ABI 拆分签名的 release APK（aarch64/x86_64，由 release 工作流 android job 在 Linux 并行构建）
