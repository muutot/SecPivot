# SecPivot Desktop v1.4.1

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-31

---

## 设置与主题定制

- **自定义配色主题** — 支持保存多个具名 配色主题、按 当前配色/已保存主题 切换编辑、另存为/应用/删除，深浅色预设显示只读调色板 | [`2d64d19`](https://github.com/muutot/SecPivot/commit/2d64d19) · [`c05cf78`](https://github.com/muutot/SecPivot/commit/c05cf78)
- **完整工具栏自定义** — 18 个按钮的全局排序 与 左侧/右侧 归属、分隔线、逐项显隐，点击条目自动展开详情面板开关 | [`2d64d19`](https://github.com/muutot/SecPivot/commit/2d64d19) · [`411adbe`](https://github.com/muutot/SecPivot/commit/411adbe)
- **导入导出收纳到工具栏级联菜单** — 精简空白菜单 | [`e5b2184`](https://github.com/muutot/SecPivot/commit/e5b2184)
- **配置健壮性** — 损坏配置文件自动修复且对齐 TS/Rust 归一化，清除的自定义配色保持为空串穿越往返 | [`ec2b67d`](https://github.com/muutot/SecPivot/commit/ec2b67d) · [`817c27d`](https://github.com/muutot/SecPivot/commit/817c27d)

## 凭据表单与弹窗统一

- **欢迎页/锁屏共用凭据表单** — 抽取 `StandaloneVaultShell` 外壳与 `VaultCredentialFields` 凭据字段（主密码/确认/密钥文件/显示切换/错误）| [`ac56ffd`](https://github.com/muutot/SecPivot/commit/ac56ffd)
- **模板层收敛** — 引入 Toggle/TextField/Button/Feedback 模板，全部 modal-button 迁移到 Button、退役旧 modal/viewport 共享样式表 | [`c1ed0e2`](https://github.com/muutot/SecPivot/commit/c1ed0e2) · [`057340d`](https://github.com/muutot/SecPivot/commit/057340d) · [`7b29062`](https://github.com/muutot/SecPivot/commit/7b29062) · [`cceca37`](https://github.com/muutot/SecPivot/commit/cceca37)

## 菜单与交互细节

- **级联菜单溢出翻转** — 菜单靠近窗口边缘时自动反向展开 | [`424ca85`](https://github.com/muutot/SecPivot/commit/424ca85)
- **抽屉内固定定位修复** — 统一菜单拥有者对称性，打开条目菜单时关闭列菜单 | [`a9cd128`](https://github.com/muutot/SecPivot/commit/a9cd128) · [`9e034f4`](https://github.com/muutot/SecPivot/commit/9e034f4)
- **表格滚动条贴边** — 垂直滚动条固定到面板边缘共享滚动容器 | [`afd6083`](https://github.com/muutot/SecPivot/commit/afd6083)

## 移动端与响应式

- **长按上下文菜单** 与 窄屏选中式列网格 | [`d44c2b2`](https://github.com/muutot/SecPivot/commit/d44c2b2)
- **详情面板收起** — 面板内隐藏按钮替换移动端返回键 | [`0b3e8fa`](https://github.com/muutot/SecPivot/commit/0b3e8fa) · [`d489d22`](https://github.com/muutot/SecPivot/commit/d489d22)

## 远程同步

- **ETag 前置条件保存** — 远程保存前对比观察到的 ETag，拒绝丢失的竞态写覆盖 | [`674b7a0`](https://github.com/muutot/SecPivot/commit/674b7a0)

## Vault 后端修复

- **回收站语义** — 禁用回收时跳过建桶并修正尺寸裁剪索引、安全报告排除回收站、history 字节预算在裁剪快照时强制执行、还原版本时恢复整条记录、回收站禁用时永久删除 | [`b2579d9`](https://github.com/muutot/SecPivot/commit/b2579d9) · [`398dac4`](https://github.com/muutot/SecPivot/commit/398dac4) · [`8e3dc67`](https://github.com/muutot/SecPivot/commit/8e3dc67) · [`63d84f4`](https://github.com/muutot/SecPivot/commit/63d84f4) · [`b07a2c8`](https://github.com/muutot/SecPivot/commit/b07a2c8)
- **搜索语义** — 按 KeePass 语义继承父组 EnableSearching，浏览时显示不可搜索分组，切换库清空搜索 | [`05eaeb7`](https://github.com/muutot/SecPivot/commit/05eaeb7) · [`0baba44`](https://github.com/muutot/SecPivot/commit/0baba44)
- **隐私收敛** — 历史快照/浏览器持久化中清空受保护自定义字段值 | [`72a519d`](https://github.com/muutot/SecPivot/commit/72a519d) · [`35dbb99`](https://github.com/muutot/SecPivot/commit/35dbb99)

## 依赖与供应链

- **依赖对齐** — RustCrypto 与 quick-xml 与 keepass 对齐去重 lockfile；前端依赖更新到最新补丁版 | [`eef7a76`](https://github.com/muutot/SecPivot/commit/eef7a76) · [`065dc1f`](https://github.com/muutot/SecPivot/commit/065dc1f)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.4.1_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.4.1-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 按 64 位 ABI 拆分签名的 release APK（aarch64/x86_64，由 release 工作流 android job 在 Linux 并行构建）
