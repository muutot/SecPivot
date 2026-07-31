# KeyVault Desktop v0.1.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-07-31

---

## 核心功能

- **标准 KDBX 4.0 读写** — 基于 `keepass` crate，打开 / 新建 / 原子保存 | [`2002b79d`](https://github.com/keyvault/KeyVault/commit/2002b79d)
- **三栏紧凑布局** — 分组树 / 条目列表 / 详情面板，全屏无状态栏的信息密度设计
- **条目与分组 CRUD** — 新建 / 编辑 / 删除，支持标题、用户名、密码（掩码 + 显示）、URL、备注、TOTP 种子 | [`13453428`](https://github.com/keyvault/KeyVault/commit/13453428)
- **密码生成器** — 字符集开关、排除相似 / 易混字符、实时熵值读数
- **搜索过滤** — 跨标题 / 用户名 / URL / 备注即时过滤，按分组筛选

## 安全

- **自动锁定** — 空闲超时自动锁定（`autoLockMinutes`）| [`13453428`](https://github.com/keyvault/KeyVault/commit/13453428)
- **剪贴板安全** — 复制密码后按设定秒数自动清空；锁定时清空剪贴板
- **锁定屏** — 记住路径快速重开，或切换到其他数据库 | [`2db43566`](https://github.com/keyvault/KeyVault/commit/2db43566)

## 搜索与生产力

- **TOTP 显示** — `totp_code` 命令 + 倒计时组件，支持 `otpauth://` 与裸 Base32 种子 | [`419611ea`](https://github.com/keyvault/KeyVault/commit/419611ea)
- **URL 快速打开** — 通过 `@tauri-apps/plugin-opener` 在系统浏览器打开条目 URL | [`7e7f7289`](https://github.com/keyvault/KeyVault/commit/7e7f7289)
- **收藏 / 置顶** — 星标收藏，收藏条目优先排序 | [`a7ebb016`](https://github.com/keyvault/KeyVault/commit/a7ebb016)

---

## 构建产物

- **NSIS 安装包**: `KeyVault_0.1.0_x64-setup.exe`
