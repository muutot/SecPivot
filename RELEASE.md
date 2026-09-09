# SecPivot Desktop v1.5.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-09-09

---

## 条目列表：分组分隔线与行高

- **KeePass 式分组分隔线** — 子分组标题行、组内排序、分隔线字体颜色（空=默认弱化文本色），PageUp/PageDown 跨分隔行导航钳制，分隔行对辅助技术隐藏、无障碍序号只计条目 | [`83cd2cf`](https://github.com/muutot/SecPivot/commit/83cd2cf) · [`3d1cc43`](https://github.com/muutot/SecPivot/commit/3d1cc43) · [`ba6b3d9`](https://github.com/muutot/SecPivot/commit/ba6b3d9) · [`45d875d`](https://github.com/muutot/SecPivot/commit/45d875d)
- **可配置条目行高** — 紧凑密度默认常开，行高设置驱动虚拟化数学与 CSS（桌面下限 24px、窄屏下限 36px）| [`bbdc454`](https://github.com/muutot/SecPivot/commit/bbdc454)
- **分组计数修正** — 全部条目 = 根计数减回收站 | [`8433714`](https://github.com/muutot/SecPivot/commit/8433714)
- **虚拟化与面板修复** — 从表格容器测量滚动恢复虚拟化，拖拽前快照面板宽度防止回弹 | [`01a28e6`](https://github.com/muutot/SecPivot/commit/01a28e6) · [`7143c52`](https://github.com/muutot/SecPivot/commit/7143c52)

## HIBP 泄露检查

- **取消与进度** — 泄露检查对标 favicon 流程：结束等待取消、按前缀进度 | [`6d0c59b`](https://github.com/muutot/SecPivot/commit/6d0c59b)
- **取消不再误报干净** — 取消的检查显示“结果不完整”，收集到的为部分结果 | [`70a1b9b`](https://github.com/muutot/SecPivot/commit/70a1b9b)

## Favicon

- **可取消下载** — 结束等待按钮中止 favicon 抓取 | [`ae5a23f`](https://github.com/muutot/SecPivot/commit/ae5a23f)
- **站点 link 解析** — 解析页面 link 标签并 http 回退 | [`b74117f`](https://github.com/muutot/SecPivot/commit/b74117f)

## TCATO 两通道填充

- **焦点与目标守卫** — 覆盖层不抢焦点（重开只显示不激活）、不向自身注入，关闭/锁定/切换标签清理旧目标，空通道拒绝注入且禁用按钮，回收站条目拒绝填充 | [`a7be76d`](https://github.com/muutot/SecPivot/commit/a7be76d) · [`a319075`](https://github.com/muutot/SecPivot/commit/a319075) · [`7b38185`](https://github.com/muutot/SecPivot/commit/7b38185) · [`4f4515e`](https://github.com/muutot/SecPivot/commit/4f4515e) · [`73d1be2`](https://github.com/muutot/SecPivot/commit/73d1be2)
- **错误可视** — 视图缺席时打开错误浮出、标签切换自动关闭并上报过期打开错误 | [`c87518b`](https://github.com/muutot/SecPivot/commit/c87518b) · [`a1b395a`](https://github.com/muutot/SecPivot/commit/a1b395a) · [`686904e`](https://github.com/muutot/SecPivot/commit/686904e)

## 笔记与详情

- **仅链接化 URL** — 笔记阅读视图只把 URL 变成可点击链接（邮件/电话不再复制按钮）| [`da259b9`](https://github.com/muutot/SecPivot/commit/da259b9)
- **笔记区排版** — 恢复分隔线、统一读写显示、收紧间距，去侧边间隙并隐藏滚动条（保持可滚），笔记填满整区 | [`0175b45`](https://github.com/muutot/SecPivot/commit/0175b45) · [`d635704`](https://github.com/muutot/SecPivot/commit/d635704) · [`fa81d17`](https://github.com/muutot/SecPivot/commit/fa81d17)

## 设置与主题

- **自定义配色打磨** — 主题切换保留自定义调色板，操作换行时标题不挤压、动作右对齐、副标题隐藏，主题操作图标化且选择器置末，多主题动作并入配色卡片、存档/改名走 ModalShell 对话框 | [`34b2312`](https://github.com/muutot/SecPivot/commit/34b2312) · [`b199c1b`](https://github.com/muutot/SecPivot/commit/b199c1b) · [`ad61864`](https://github.com/muutot/SecPivot/commit/ad61864) · [`5e1d36f`](https://github.com/muutot/SecPivot/commit/5e1d36f) · [`dd3e70a`](https://github.com/muutot/SecPivot/commit/dd3e70a) · [`37f90c8`](https://github.com/muutot/SecPivot/commit/37f90c8) · [`37f6c5f`](https://github.com/muutot/SecPivot/commit/37f6c5f) · [`57beeaf`](https://github.com/muutot/SecPivot/commit/57beeaf) · [`1600515`](https://github.com/muutot/SecPivot/commit/1600515) · [`ecf36fc`](https://github.com/muutot/SecPivot/commit/ecf36fc)
- **版本号来自 Tauri** — 设置关于页不再硬编码 | [`3de15ec`](https://github.com/muutot/SecPivot/commit/3de15ec)

## 锁屏、工具栏与搜索

- **独立窗口居中** — 锁定/欢迎壳居中、锁屏路径卡片化 | [`dc93229`](https://github.com/muutot/SecPivot/commit/dc93229)
- **移除未保存徽标** — 工具栏不再显示 dirty badge | [`3a3b50c`](https://github.com/muutot/SecPivot/commit/3a3b50c)
- **高级搜索对话框排版** — 修复布局溢出与块间距 | [`4d45d0a`](https://github.com/muutot/SecPivot/commit/4d45d0a)

## 杂项

- **Clippy 与依赖** — 修复 HIBP/favicon 循环 clippy 告警，keepass 钉版避开 aes 冲突 | [`2c8777c`](https://github.com/muutot/SecPivot/commit/2c8777c) · [`1e4f77f`](https://github.com/muutot/SecPivot/commit/1e4f77f)
- **格式** — 全仓 prettier 对齐 | [`3d687fe`](https://github.com/muutot/SecPivot/commit/3d687fe) · [`2b848ad`](https://github.com/muutot/SecPivot/commit/2b848ad) · [`693bd07`](https://github.com/muutot/SecPivot/commit/693bd07)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.5.0_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.5.0-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 按 64 位 ABI 拆分签名的 release APK（aarch64/x86_64，由 release 工作流 android job 在 Linux 并行构建）
