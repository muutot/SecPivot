<h1 align="center">
  <img src="static/app-icon.png" width="128" alt="SecPivot">
</h1>

<p align="center">
  <strong>专业 · 紧凑 · 本地优先的 KeePass 客户端</strong><br>
  <sub>Built with Tauri 2 · Rust · Svelte 5 · KDBX 4.0</sub>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Android-blue" alt="platform">
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="license">
  <img src="https://img.shields.io/badge/Tauri-2.x-ffc131?logo=tauri" alt="tauri">
  <img src="https://img.shields.io/badge/Svelte-5.x-ff3e00?logo=svelte" alt="svelte">
  <img src="https://img.shields.io/badge/Rust-edition2021-dea584?logo=rust" alt="rust">
</p>

---

## 简介

SecPivot 是一款**专业、紧凑、信息密度高**的 KeePass 客户端。三栏布局（分组树 → 条目列表 → 详情面板）常驻一屏，支持标准 **KDBX 4.0** 读写、多数据库标签页、远程金库（S3 / WebDAV）与浏览器集成（KeePassHttp / KeePassRPC），中英双语界面。

> **隐私承诺**：本地优先，主密码与密钥从不离开本机会话，不包含任何遥测。仅在以下用户显式操作时访问网络：打开远程金库（你自己的 S3 / WebDAV）、下载网站图标、HIBP 泄露检查（仅发送 SHA-1 前 5 位前缀）；敏感配置在 Windows 上经 DPAPI 加密落盘。

### 快速预览

```
欢迎页打开 .kdbx → 主密码解锁 → 三栏浏览 / 搜索 → 条目详情复制 → 空闲自动锁定
```

---

## 核心特性

<table>
<tr>
<td width="50%">

### 🗄️ 金库与条目

- 标准 **KDBX 4.0** 读写（`keepass` crate），原子保存
- **多数据库标签页**：多库并存、独立会话、dirty 标记
- 条目 / 分组 CRUD，回收站（恢复 / 清空，跨重开持久化）
- 条目历史版本（自动快照，最多保留 10 版）
- 内置图标 0–68 + `#RRGGBB` 颜色标记
- 收藏条目、条目过期提醒、自定义字段与附件
- 变更时间线：全库字段变更事件流

### 🔍 搜索与列表

- 即时搜索（标题 / 用户名 / URL / 备注），分组子树筛选
- **高级搜索**：字段范围、正则、排除取反、过期 / 收藏 / 标签 / 质量条件
- 命名搜索配置持久化（保存搜索）
- 窗口化条目列表：排序 / 多选 / 键盘导航 / 拖拽移动分组
- 密码列遮蔽，就地显示 10 秒自动遮回

### ⌨️ 自动填充

- **全局 Auto-Type 热键**：按前台窗口标题匹配条目（域名 / 标题，`*` 通配）
- 窗口关联 + 默认序列，条目 / 分组两级编辑器
- 多命中候选选择器，`{REF:...}` 字段引用
- **TCATO** 双通道覆盖层注入，密码不离开后端
- TOTP 截图取码：区域选取 / 全屏识别二维码

</td>
<td width="50%">

### 🔒 密码与安全

- 密码生成器规则引擎：字符集 / 必含 / 排除 / pattern，命名配置档
- Rust 端镜像实现，桥接协议共用同一规则
- TOTP / HOTP / Steam Guard 显示与倒计时
- 复制后按秒数自动清空剪贴板，锁定即清可配
- 空闲自动锁定，锁定清内存会话
- 防截屏窗口守卫（`WDA_EXCLUDEFROMCAPTURE`，默认关闭）
- HIBP 泄露检查：k-anonymity，密码绝不出本机
- 相似密码聚类、只读降级（连续保存失败保护数据）

### ☁️ 远程金库

- **S3**（自研 SigV4，rustls，无 OpenSSL）/ **WebDAV**
- 多 profile 配置，欢迎页远程文件浏览
- 保存模式：仅回传 / 本地镜像（时间戳 `.bak` 轮转）
- 远程变更检测（SHA-256 base hash）与冲突解决：覆盖 / 下载 / 条目级合并

### 🎨 界面与设置

- 20 语义主题色 + 暗 / 亮预设 + 自定义映射
- 小屏响应式（720px 断点）：单列堆叠、分组抽屉、详情全屏
- 通用 / 安全 / 数据库 / 远程 / 集成 / 关于设置面板
- 数据库设置对话框：KDF / cipher / 压缩 / 历史上限 / 回收站
- 便携模式：exe 旁 `portable.flag` 即配置随行

</td>
</tr>
</table>

### 🔌 浏览器集成（桌面）

- **KeePassHttp 兼容**：loopback `127.0.0.1:19455`，AES-256-CBC 逐字段加密 + HMAC-SHA256，关联审批板
- **KeePassRPC 兼容**：SRP-6a + WebSocket `:12546`，旁路密码对话框，AddLogin / UpdateLogin 写路径
- 服务仅绑定 active 会话，锁定即销毁会话密钥
- BROWSER_SETTINGS_SYNC 仅客户端特性，不实现不宣告（见 `docs/browser-integration.md`）

### 🔄 导入导出

- 导入：CSV（含 LastPass 表头别名）、KeePass XML、Bitwarden JSON、1PIF
- 导出：CSV（明文确认）、KeePass XML（`Protected` + Base64）、HTML 应急表
- 附件：内存预览（2 MiB 截断）、受控临时打开、导入修改写回（64 MiB 上限）

---

## 技术栈

<p align="left">
  <img src="https://skillicons.dev/icons?i=tauri,rust,svelte,typescript,vite" alt="tech stack" />
</p>

| 层级     | 技术                       | 说明                                              |
| :------- | :------------------------- | :------------------------------------------------ |
| 桌面框架 | **Tauri 2**                | 轻量跨平台桌面壳，Rust 后端 + Web 前端            |
| 后端     | **Rust** (edition 2021)    | KDBX 会话、加密原语、同步、桥接协议               |
| 金库格式 | **KDBX 4.0** (`keepass`)   | 打开 / 新建 / 原子保存，Argon2id / AES / ChaCha20 |
| 远程传输 | **自研 S3 SigV4 + WebDAV** | rustls 全链路，无 OpenSSL / native-tls 依赖       |
| 前端     | **Svelte 5 + SvelteKit**   | Runes 响应式语法，adapter-static SPA 模式         |
| 构建     | **Vite 8 + Cargo**         | 前端构建 + Rust 增量编译                          |
| 打包     | **Tauri Bundler**          | NSIS 安装包 + 便携 ZIP（Windows），APK（Android） |

---

## 快速开始

### 环境要求

- **Node.js 24**（`.nvmrc` 已固定；类型检查与构建依赖其原生 TS 剥离）
- **Rust** 稳定版工具链（版本由根目录 `rust-toolchain.toml` 固定，rustup 自动安装）
- **Windows**：Visual Studio Build Tools（MSVC）+ WebView2 运行时；首次 `cargo build` 需要联网拉取 crates
- Android 构建需另备 JDK 17 + Android SDK/NDK（见 `docs/android.md`）

### 开发运行

```sh
npm install          # 安装前端依赖
npm run tauri dev    # 启动桌面应用（热重载）
npm run dev          # 或仅运行前端（浏览器预览 + 演示数据）
```

> 浏览器模式仅用于 UI 开发（`localStorage` 演示金库）；真实 KDBX 读写行为以 Rust 后端为准。

### 生产构建

```sh
npm run tauri build  # 在 src-tauri/target/release/bundle/ 生成安装包
```

---

## 平台支持状态

| 能力                     | Windows 桌面         | Android（进行中）               |
| :----------------------- | :------------------- | :------------------------------ |
| KDBX 打开 / 保存 / 锁定  | ✅ NSIS + 便携版     | 🚧 与桌面同源 Rust 会话逻辑     |
| Auto-Type / TCATO / 托盘 | ✅ 桌面专属          | ❌ 移动端裁剪（后台锁代替）     |
| 浏览器桥接（HTTP/RPC）   | ✅ loopback 本地服务 | ❌ 桌面专属                     |
| S3 / WebDAV 远程金库     | ✅                   | 🚧 复用同一 rustls 传输层       |
| 文件选择                 | ✅ 系统对话框        | 🚧 SAF / 文档选择器适配         |
| APK 分发                 | —                    | 🚧 按 ABI 拆分签名包，CI 构建中 |

- ✅ 已交付 — 🚧 进行中 — ❌ 明确不做（见 `docs/android.md`）

---

## 开发命令

| 命令                    | 说明                                                             |
| :---------------------- | :--------------------------------------------------------------- |
| `npm run dev`           | 启动 Vite 开发服务器（仅前端，演示数据）                         |
| `npm run build`         | 前端生产构建                                                     |
| `npm run tauri dev`     | Tauri 桌面应用开发模式                                           |
| `npm run tauri build`   | Tauri 桌面应用生产构建                                           |
| `npm run check`         | TypeScript / Svelte 类型检查                                     |
| `npm run format`        | 代码格式化（Prettier + `cargo fmt`）                             |
| `npm run format:check`  | 格式检查（不修改文件）                                           |
| `npm run test:frontend` | 前端行为测试（Node 内置 runner）                                 |
| `npm run test:rust`     | Rust 单元测试                                                    |
| `npm run lint:rust`     | Rust Clippy 检查                                                 |
| `npm run verify`        | **全量检查**：格式 + 类型 + 构建 + 前端测试 + Rust 测试 + Clippy |
| `npm run regression`    | 回归门：`verify` + 手动 PITFALLS 检查清单                        |

### 推荐 IDE 插件

- [Svelte for VS Code](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode)
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

---

## 项目结构

```
SecPivot/
├── src/                           # Svelte 5 前端 (SPA)
│   ├── routes/
│   │   ├── +page.svelte           # 主界面（欢迎/解锁、三栏、搜索、编辑器）
│   │   └── settings/+page.svelte  # 设置界面
│   └── lib/
│       ├── components/            # 复用组件（含 settings/* 各设置面板）
│       ├── composables/           # 面板布局、选择、过滤、编辑器状态机
│       ├── services/              # 设置/金库 IPC 封装、键盘、导入导出编排
│       ├── types/                 # 主题、设置、金库类型定义
│       ├── i18n/                  # 国际化（zh-CN 源、en 类型级对等）
│       ├── utils/                 # 密码、搜索、树、虚拟列表、TOTP 等纯函数
│       └── data/                  # 浏览器预览演示数据
├── src-tauri/                     # Rust 后端
│   ├── tauri.conf.json            # Tauri 2 配置（桌面包名与窗口）
│   ├── tauri.android.conf.json    # Android 独立包名
│   ├── Cargo.toml                 # Rust 依赖（桌面专属依赖已隔离）
│   └── src/
│       ├── lib.rs                 # 命令注册、托盘、热键、窗口生命周期
│       ├── commands/              # Tauri IPC 命令（金库/分组/条目/桥接/剪贴板…）
│       ├── vault/                 # 会话注册表、KDBX 仓储、匹配与历史
│       ├── crypto/                # AES/HMAC/OTP 纯原语
│       ├── config/                # 配置读写、DPAPI、便携模式
│       ├── remote/                # S3 / WebDAV 传输与同步
│       ├── bridge/                # KeePassHttp 协议服务
│       ├── rpc/                   # KeePassRPC 协议服务
│       └── platform/              # 自动填充、焦点、守卫、凭据（桌面）
├── docs/                          # 设计文档
│   ├── android.md                 # 安卓支持评估与落地清单
│   ├── browser-integration.md     # 浏览器集成协议提案与规格
│   ├── i18n.md                    # 国际化约定
│   └── PITFALLS.md                # 开发陷阱与约定
├── skills/secpivot-dev/           # 开发规约与子系统参考
├── scripts/                       # 版本、发布、打包、回归脚本
├── tests/                         # 前端行为测试
└── TODO.md                        # 路线图与验收（证据口径）
```

---

## 文档

| 文档                                                         | 内容                             |
| :----------------------------------------------------------- | :------------------------------- |
| [skills/secpivot-dev/SKILL.md](skills/secpivot-dev/SKILL.md) | 开发规约、任务流程、提交格式     |
| [TODO.md](TODO.md)                                           | 路线图与验收（直接证据口径）     |
| [docs/browser-integration.md](docs/browser-integration.md)   | 浏览器集成协议提案与偏差记录     |
| [docs/android.md](docs/android.md)                           | 安卓支持评估与落地清单           |
| [docs/i18n.md](docs/i18n.md)                                 | 国际化约定与字典对等要求         |
| [docs/PITFALLS.md](docs/PITFALLS.md)                         | Svelte 5 / Tauri / Rust 开发陷阱 |

---

## 安全报告

发现安全漏洞请**不要**提交公开 issue：通过 GitHub Security Advisories（仓库 Security 标签页 → Report a vulnerability）私密报告。密钥与主密码从不离开本机会话；报告请附复现步骤与影响范围，我们会在修复发布前保持保密。

---

## 贡献

欢迎提交 Issue 和 Pull Request。提交消息格式遵循 **gitmoji 约定**：

```
<gitmoji> <type>[<scope>]: <message>
```

示例：`✨ feat[vault]: add open_vault/create_vault backend session` | `🐛 fix[settings]: handle window close state`

---

## 许可证

MIT © SecPivot Contributors
