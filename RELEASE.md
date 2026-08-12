# SecPivot Desktop v1.2.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-12

---

## 官方同步

- **远程变更检测** — 打开/创建远程库记录内容 SHA-256，改密/存储重加密前比对，远程已变更返回冲突错误 | [`dd79b18`](https://github.com/muutot/SecPivot/commit/dd79b18)
- **冲突解析 UI** — 保存冲突弹窗「合并 / 覆盖远程 / 下载远程 / 保留本地」| [`496512b`](https://github.com/muutot/SecPivot/commit/496512b)
- **条目级合并** — 按条目/分组 UUID + 字段 last-modified 合并本地与远程，历史双方保留、回收站排除，冲突测试覆盖同时单改/删除 | [`f68095b`](https://github.com/muutot/SecPivot/commit/f68095b)
- **备份去重修复** — 同毫秒生成备份时间戳碰撞时追加 `_1/_2/…` 后缀，重命名不再静默覆盖旧备份；后缀排序不破坏按时间裁剪 | [`9f3e7ae`](https://github.com/muutot/SecPivot/commit/9f3e7ae)

## 数据安全

- **HIBP 泄露检测** — 严格 opt-in，仅发送 SHA-1 前 5 位十六进制前缀（k-anonymity），密码/完整散列绝不出本机 | [`c458213`](https://github.com/muutot/SecPivot/commit/c458213)
- **相似密码报告** — 服务端编辑距离聚类（< 4），回收站排除，密码不外出 | [`910a8ae`](https://github.com/muutot/SecPivot/commit/910a8ae)
- **只读降级** — 连续 3 次保存失败后拒绝写路径并提示「另存为」，另存为成功后复位 | [`eefc1cb`](https://github.com/muutot/SecPivot/commit/eefc1cb)

## 数据库设置与维护

- **数据库设置对话框** — KDF/cipher/compression/history 上限/回收站开关/模板组，变更自动重加密并 round-trip 保真 | [`071da49`](https://github.com/muutot/SecPivot/commit/071da49) · [`34f1f48`](https://github.com/muutot/SecPivot/commit/34f1f48)
- **过期条目维护** — 集中清单，单选全部延期 30 天或删除 | [`f709836`](https://github.com/muutot/SecPivot/commit/f709836)
- **全库历史清理** — 一键清空全部条目历史版本 | [`f12685a`](https://github.com/muutot/SecPivot/commit/f12685a)
- **库元数据编辑** — 数据库名称/描述可编辑并持久化 | [`51a2459`](https://github.com/muutot/SecPivot/commit/51a2459)
- **损坏库诊断** — 头部识别 KDBX/KDB/未知，非库文件快速失败并给出 XML 恢复兜底提示 | [`00aa2f3`](https://github.com/muutot/SecPivot/commit/00aa2f3) · [`f8fe0e0`](https://github.com/muutot/SecPivot/commit/f8fe0e0)

## 导入导出

- **HTML 应急表** — 离线可打印，含密码需显式勾选并带警告横幅 | [`64834df`](https://github.com/muutot/SecPivot/commit/64834df)
- **Bitwarden JSON / 1Password 1PIF / LastPass CSV** — 服务器端严格解析，文件夹→分组映射 | [`0e42b3f`](https://github.com/muutot/SecPivot/commit/0e42b3f) · [`e370f3b`](https://github.com/muutot/SecPivot/commit/e370f3b)

## 附件

- **内存预览** — 文本/图片 data URL（2 MiB 截断），不落盘 | [`794833c`](https://github.com/muutot/SecPivot/commit/794833c)
- **受控临时打开** — token 注册目录 + 外部查看 + 导入修改写回，锁定自动清除 | [`94597b1`](https://github.com/muutot/SecPivot/commit/94597b1) · [`d3f940d`](https://github.com/muutot/SecPivot/commit/d3f940d)

## 搜索与密码生成器

- **高级搜索** — 字段范围/正则/排除/过期/收藏/标签/质量条件 + 保存的搜索 | [`67a889f`](https://github.com/muutot/SecPivot/commit/67a889f) · [`d29780f`](https://github.com/muutot/SecPivot/commit/d29780f) · [`fb76bd8`](https://github.com/muutot/SecPivot/commit/fb76bd8)
- **密码生成器规则镜像** — 自定义字符集/排除/必含/pattern，Rust 镜像 + 配置档管理，bridge/RPC 同规则 | [`ca98f28`](https://github.com/muutot/SecPivot/commit/ca98f28) · [`51ca824`](https://github.com/muutot/SecPivot/commit/51ca824) · [`a62e2df`](https://github.com/muutot/SecPivot/commit/a62e2df)

## Auto-Type 与浏览器集成

- **Auto-Type 全量配置** — 条目/分组编辑页 + 窗口关联、多命中选择器 | [`9bda5e5`](https://github.com/muutot/SecPivot/commit/9bda5e5) · [`2ddb682`](https://github.com/muutot/SecPivot/commit/2ddb682) · [`aaac92e`](https://github.com/muutot/SecPivot/commit/aaac92e) · [`cc0cc73`](https://github.com/muutot/SecPivot/commit/cc0cc73)
- **KeePassRPC 增强** — 注册域匹配模式、锁后会话保持、KeyVault 配置页与 KPRPC JSON 匹配 | [`38d4783`](https://github.com/muutot/SecPivot/commit/38d4783) · [`31f8d1f`](https://github.com/muutot/SecPivot/commit/31f8d1f) · [`e5a3916`](https://github.com/muutot/SecPivot/commit/e5a3916) · [`cfa5fdf`](https://github.com/muutot/SecPivot/commit/cfa5fdf)

## 多数据库标签页

- **会话注册表与标签页** — 多库并存、切换、锁定全部标签，记住最后路径 | [`90da622`](https://github.com/muutot/SecPivot/commit/90da622) · [`94a7e8c`](https://github.com/muutot/SecPivot/commit/94a7e8c) · [`003c7d9`](https://github.com/muutot/SecPivot/commit/003c7d9) · [`05d23df`](https://github.com/muutot/SecPivot/commit/05d23df)

## 移动端 / Android

- **响应式界面** — 窄屏分组抽屉 + 详情浮层、设置分类抽屉、条目摘要行、工具栏收拢 | [`7a59adf`](https://github.com/muutot/SecPivot/commit/7a59adf) · [`41ff18f`](https://github.com/muutot/SecPivot/commit/41ff18f) · [`bed754f`](https://github.com/muutot/SecPivot/commit/bed754f)
- **Android 构建并入 release 工作流** — 由 secrets 配置签名，桌面/Windows 专属后端 `cfg` 门控以支持跨平台编译 | [`1772b3f`](https://github.com/muutot/SecPivot/commit/1772b3f) · [`3e4cc2d`](https://github.com/muutot/SecPivot/commit/3e4cc2d)
- **Android CI 构建修复** — 安卓 job 移至 `ubuntu-latest` 并与桌面构建并行，`openssl` vendored（openssl-src）解决 `openssl-sys` 交叉编译失败 | [`10b3a93`](https://github.com/muutot/SecPivot/commit/10b3a93)

## 性能

- **MutationDelta** — 收藏/展开不再经 IPC 重传完整树（省略自定义图标负载 + revision 排序）| [`e222620`](https://github.com/muutot/SecPivot/commit/e222620) · [`629f77e`](https://github.com/muutot/SecPivot/commit/629f77e) · [`9921aef`](https://github.com/muutot/SecPivot/commit/9921aef)
- **条目列表虚拟化** — 仅挂载可视区 + 缓冲区 | [`eedc771`](https://github.com/muutot/SecPivot/commit/eedc771)
- **批量分组展开** — 单次 IPC 一次事务写入 | [`0d4e6c7`](https://github.com/muutot/SecPivot/commit/0d4e6c7)
- **树索引与搜索缓存**、排序键预计算、favicon 连接池复用 | [`21d8fe9`](https://github.com/muutot/SecPivot/commit/21d8fe9) · [`0054124`](https://github.com/muutot/SecPivot/commit/0054124) · [`4d0ba69`](https://github.com/muutot/SecPivot/commit/4d0ba69)

## 界面与可用性

- **统一弹窗基础设施** — ModalShell 为唯一弹窗表面，全部对话框迁移复用 | [`6560706`](https://github.com/muutot/SecPivot/commit/6560706) · [`f17ba08`](https://github.com/muutot/SecPivot/commit/f17ba08)
- **列头拖拽排序**（持久化）与分组内置图标选择器 | [`342f321`](https://github.com/muutot/SecPivot/commit/342f321) · [`cb38cf2`](https://github.com/muutot/SecPivot/commit/cb38cf2)
- **Ctrl+G 定位分组** — 选中条目在树中定位（含折叠场景可靠展开）| [`de59048`](https://github.com/muutot/SecPivot/commit/de59048) · [`68d75cc`](https://github.com/muutot/SecPivot/commit/68d75cc)
- **条目可编辑标签** 与按标签搜索，分组注释/标签/搜索开关编辑 | [`499c1ba`](https://github.com/muutot/SecPivot/commit/499c1ba) · [`54e4498`](https://github.com/muutot/SecPivot/commit/54e4498)
- **修复** — 右键条目不再强制打开详情面板、favicon 保存移出异步线程避免毒化数据库锁、编辑保留前景色、布局持久化顺序、识别建议去重等 | [`fd34436`](https://github.com/muutot/SecPivot/commit/fd34436) · [`6ada39f`](https://github.com/muutot/SecPivot/commit/6ada39f)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.2.0_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.2.0-portable.zip`(由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`)
- **Android APK**: `app-release.apk`(release 签名，由 release 工作流的 `android` job 在 Linux 上并行构建)
