# SecPivot Desktop v1.0.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-05

---

## 核心功能

- **标准 KDBX 4.0 读写** — 基于 `keepass` crate，打开 / 新建 / 原子保存，`save_as` 另存为并切换会话目标 | [`c898ba80`](https://github.com/muutot/SecPivot/commit/c898ba80)
- **三栏紧凑布局** — 分组树 / 条目列表 / 详情面板，无边框窗口，全屏无状态栏的信息密度设计 | [`37cc5db1`](https://github.com/muutot/SecPivot/commit/37cc5db1)
- **条目与分组 CRUD** — 新建 / 编辑 / 删除（回收站 + 恢复 + 清空），自定义字段 / 附件 / 历史版本（保留 10 版）/ 过期提示 | [`f6b6f909`](https://github.com/muutot/SecPivot/commit/f6b6f909)
- **内置图标与颜色标记** — 条目 / 分组 / 详情统一图标与颜色，支持自定义配色 | [`519f887c`](https://github.com/muutot/SecPivot/commit/519f887c)
- **密码生成器** — 字符集开关、排除相似 / 易混字符、保证每类至少一个字符
- **搜索过滤** — 跨标题 / 用户名 / URL / 备注即时过滤，按分组筛选，过期条目提示
- **导入导出** — CSV / XML 导入（自动建组），CSV 导出 | [`c0048a61`](https://github.com/muutot/SecPivot/commit/c0048a61)

## 安全

- **自动锁定** — 空闲超时 / 失焦 / 操作后自动锁定；锁定时清空剪贴板 | [`13453428`](https://github.com/muutot/SecPivot/commit/13453428)
- **剪贴板安全** — 复制密码后按设定秒数自动清空，仅清本应用复制的内容
- **主密钥管理** — 更改主密钥（密码 / 密钥文件）并重新加密；密码与密钥文件在 lock/close 时堆内存清零（zeroize） | [`e2ff467e`](https://github.com/muutot/SecPivot/commit/e2ff467e)
- **Windows Hello 快速解锁** — 主密码存于 OS 凭据库 | [`62ca7a54`](https://github.com/muutot/SecPivot/commit/62ca7a54)
- **防截屏守卫** — 可选 WDA_EXCLUDEFROMCAPTURE，库打开期间生效 | [`8b149dc6`](https://github.com/muutot/SecPivot/commit/8b149dc6)
- **安全报告** — 应用内弱密码 / 重复密码 / 空密码检测 | [`df6b95b1`](https://github.com/muutot/SecPivot/commit/df6b95b1)

## 远程存储

- **S3 远程库** — 多 profile 配置（DPAPI 加密落盘）、远端列表、本地镜像与备份轮转 | [`3b623b39`](https://github.com/muutot/SecPivot/commit/3b623b39)
- **WebDAV 存储** — PROPFIND 列表，兼容常见云盘网关 | [`c9233d57`](https://github.com/muutot/SecPivot/commit/c9233d57)
- **本地优先** — 数据不上传，远程仅作为同步镜像

## 浏览器集成

- **KeePassRPC** — WS 127.0.0.1 端口 + SRP-6a 握手 + AES JSON-RPC，AddLogin / UpdateLogin 写路径 | [`d48a6cf7`](https://github.com/muutot/SecPivot/commit/d48a6cf7)
- **KeePassHttp** — 经典协议核心，关联审批、URL 匹配、库哈希 | [`b6028bef`](https://github.com/muutot/SecPivot/commit/b6028bef)
- **一次性 SRP 密码** — 旁路密码弹窗，挂载于主布局 | [`683a0507`](https://github.com/muutot/SecPivot/commit/683a0507)

## 自动填充

- **全局热键填充** — 按前台窗口标题匹配条目 | [`49ddc924`](https://github.com/muutot/SecPivot/commit/49ddc924)
- **字段引用** — `{REF:字段@搜索字段:文本}` 跨条目引用 | [`6878cd15`](https://github.com/muutot/SecPivot/commit/6878cd15)
- **TCATO 覆盖层** — 置顶小窗 + 按键注入通道，密码不离开后端 | [`d8881543`](https://github.com/muutot/SecPivot/commit/d8881543)

## TOTP 与 OTP

- **TOTP 显示** — 列表实时验证码 + 倒计时 + 复制，兼容 `otpauth://` 与裸 Base32 | [`419611ea`](https://github.com/muutot/SecPivot/commit/419611ea)
- **KeeOtp 兼容** — TOTP / HOTP / Steam Guard 一次性密码 | [`cda586a0`](https://github.com/muutot/SecPivot/commit/cda586a0)

## 界面与体验

- **列配置** — 可排序可调宽的 KeePass 风格条目表，右键列头自定义列 | [`ff41df04`](https://github.com/muutot/SecPivot/commit/ff41df04)
- **快捷键** — 录制式自定义应用内快捷键 | [`b5ef373c`](https://github.com/muutot/SecPivot/commit/b5ef373c)
- **系统托盘** — 显示 / 锁定 / 退出，最小化到托盘 | [`85632b9f`](https://github.com/muutot/SecPivot/commit/85632b9f)
- **网址图标** — 自动下载站点 favicon 存为自定义图标 | [`d650b807`](https://github.com/muutot/SecPivot/commit/d650b807)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.0.0_x64-setup.exe`
- **便携版 (免安装)**: `SecPivot-1.0.0-portable.zip`(由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`)
