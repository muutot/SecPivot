# KeyVault

专业、紧凑、信息密度高、简约现代的 KeePass 客户端，基于 Svelte 5 + Tauri 2 + Rust 构建。界面语言与视觉体系取自 Clipboard Desktop（同级项目的设置样式规约），本地优先、无同步上传。

## 特性

- **标准 KDBX 4.0**：通过 [`keepass`](https://crates.io/crates/keepass) crate 读写（打开 / 新建 / 原子保存）。
- **三栏紧凑布局**：分组树 → 条目列表 → 详情面板；全屏无状态栏的信息密度。
- **条目与分组 CRUD**：新建 / 编辑 / 删除，条目标题、用户名、密码（掩码 + 显示切换）、URL、备注、TOTP 种子。
- **密码生成器**：字符集开关、排除相似 / 易混字符、实时熵值读数。
- **搜索过滤**：跨标题 / 用户名 / URL / 备注即时过滤，按分组筛选。
- **剪贴板安全**：复制密码后按设定秒数自动清空；锁定策略（自动锁定、锁定即清剪贴板）。
- **设置页**：外观 / 显示 / 紧凑 / 安全 / 数据库默认值 / 关于，修改即时生效，视觉与 Clipboard 设置页一致。

## 技术栈

| 层     | 选型                                                |
| ------ | --------------------------------------------------- |
| 前端   | Svelte 5（runes）+ SvelteKit（adapter-static SPA）  |
| 桌面壳 | Tauri 2（Rust）                                     |
| 后端   | Rust：`keepass`（KDBX）、serde、tauri 命令          |
| 样式   | 20 语义主题色 + 共享设置原语（源自 Clipboard 规约） |

## 开发

```powershell
npm install
npm run dev          # 浏览器预览（demo 金库，localStorage 演示）
npm run tauri dev    # Tauri 桌面运行
npm run verify       # 全量校验：格式化 + svelte-check + build + rust test + clippy
```

浏览器模式仅用于 UI 开发；真实 KDBX 读写行为以 Rust 后端为准。

## 文档

- 开发规约与任务流程：`.opencode/skills/keyvault-dev/SKILL.md`
- 样式与主题规约：`.opencode/skills/keyvault-dev/references/css-theming.md`
- 前后端契约：`.opencode/skills/keyvault-dev/references/data-contracts.md`
- 安全模型：`.opencode/skills/keyvault-dev/references/security-model.md`
- 路线图与验收：`TODO.md` · 坑位清单：`docs/PITFALLS.md`

## 许可证

MIT
