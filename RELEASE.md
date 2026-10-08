# SecPivot Desktop v1.6.2

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-10-08

---

## 条目数据交换（KeePass Data Exchange）

- **载荷核心与端到端接线** — 新增 `build/parse_entries_document`、KeePass 帧编码与可选 DPAPI 包装，粘贴在全新 UUID 下还原字段/附件/标签/自定义数据/自动填充等，前后端打通 | [`c90e387`](https://github.com/muutot/SecPivot/commit/c90e387) · [`d203817`](https://github.com/muutot/SecPivot/commit/d203817)

## 搜索与快捷键

- **输入即聚焦搜索** — 主窗口输入非快捷键字符时自动聚焦搜索框并填入首字符，新增「外观 → 显示」开关 `focusSearchOnType`（默认开启）| [`b3d8209`](https://github.com/muutot/SecPivot/commit/b3d8209)
- **清空搜索快捷键** — 新增 `clear-search` 动作（默认 `Ctrl+Shift+K`）| [`6d38114`](https://github.com/muutot/SecPivot/commit/6d38114)
- **过滤后聚焦行收敛** — 过滤使列表变短时按新边界钳制聚焦行 | [`0fb6f4c`](https://github.com/muutot/SecPivot/commit/0fb6f4c)

## 分组树

- **定位到所在分组按钮** — 「新建分组」左侧新增定位按钮，与 `Ctrl+G` 共用同一实现 | [`5ccb1f4`](https://github.com/muutot/SecPivot/commit/5ccb1f4)
- **重命名取消修复** — 分组重命名弹窗的取消按钮现在真正取消 | [`65a4c17`](https://github.com/muutot/SecPivot/commit/65a4c17)

## 条目详情与编辑

- **右键复制整条条目** — 条目右键菜单新增「复制条目」（KeePass 字段顺序，空字段省略，受保护自定义字段不输出明文）| [`c5ebfc6`](https://github.com/muutot/SecPivot/commit/c5ebfc6)
- **详情面板重开时机** — 仅在选中变化时重开详情面板，而非任意库更新 | [`72a5c2c`](https://github.com/muutot/SecPivot/commit/72a5c2c)
- **备注草稿不再丢失** — 保留最后一次备注草稿 | [`d89696a`](https://github.com/muutot/SecPivot/commit/d89696a)
- **过期秒级保留** — 未改动过期字段时保留原有秒数 | [`5fde8cd`](https://github.com/muutot/SecPivot/commit/5fde8cd)
- **过期响应与滚动遮罩** — 丢弃过期的密钥揭示响应，并重新遮罩已滚动行 | [`5f46adb`](https://github.com/muutot/SecPivot/commit/5f46adb)

## 设置外观

- **外观提升为一级分类** — 原「通用 → 外观」拆分为独立一级分类（主题/显示/窗口/布局/工具栏），界面语言与网络留在「通用」| [`1d1af61`](https://github.com/muutot/SecPivot/commit/1d1af61)

## 安全

- **剪贴板明文清除** — 计划清理生效后立即丢弃内存中的明文副本 | [`020b0e1`](https://github.com/muutot/SecPivot/commit/020b0e1)
- **注册域匹配收敛** — 阻止无关主机共享同一注册域 | [`8ee5e26`](https://github.com/muutot/SecPivot/commit/8ee5e26)
- **锁定清理凭据** — 关闭「记住密码」时锁定即清除已保存凭据 | [`70b07c1`](https://github.com/muutot/SecPivot/commit/70b07c1)
- **批量原子性与加固** — 批量操作失败原子回滚，会话/OTP/HIBP 处理加固 | [`691a9f4`](https://github.com/muutot/SecPivot/commit/691a9f4)

## 导入 / 导出

- **CSV 无标题行保留** — 保留有内容但缺标题或密码的 CSV 行 | [`4300cfb`](https://github.com/muutot/SecPivot/commit/4300cfb)
- **KeePass XML 过期保留** — 导入时保留条目过期时间 | [`6fee555`](https://github.com/muutot/SecPivot/commit/6fee555)
- **CSV 别名与竞态** — 修正 LastPass 别名方向、剪贴板归属与关闭竞态 | [`d5aad80`](https://github.com/muutot/SecPivot/commit/d5aad80)

## 稳定性与后端

- **命令 panic 隔离** — 命令 panic 不再毒化会话注册表 | [`22de067`](https://github.com/muutot/SecPivot/commit/22de067)
- **持久化门限等待** — 为持久化 gate 等待设置上界 | [`0316a8b`](https://github.com/muutot/SecPivot/commit/0316a8b)
- **RPC 帧上限** — websocket 重组阶段强制帧大小上限 | [`f6b03b3`](https://github.com/muutot/SecPivot/commit/f6b03b3)
- **远程镜像原子写** — 远程库本地镜像改为原子写 | [`fabba84`](https://github.com/muutot/SecPivot/commit/fabba84)
- **取消按标签页隔离** — HIBP 与 favicon 取消绑定到所属标签页 | [`8d7a59b`](https://github.com/muutot/SecPivot/commit/8d7a59b)
- **IPv6 主机匹配** — 双侧解析带方括号的 IPv6 主机 | [`c46d6b7`](https://github.com/muutot/SecPivot/commit/c46d6b7)
- **HOTP 修改时间** — HOTP 计数器推进时更新最后修改时间 | [`b170b9f`](https://github.com/muutot/SecPivot/commit/b170b9f)
- **配置默认值与运行时应用** — 加载时保留 true 默认值，透明度与截屏防护运行时即时生效 | [`7d349dd`](https://github.com/muutot/SecPivot/commit/7d349dd)
- **工具栏空白拖窗** — 工具栏空白区域可拖动窗口 | [`f62a9b1`](https://github.com/muutot/SecPivot/commit/f62a9b1)
- **列拖拽监听释放** — 表格卸载时释放列拖拽监听 | [`e95764f`](https://github.com/muutot/SecPivot/commit/e95764f)

## 构建与 CI

- **发布工作流校验修复** — `release.yml` 的 `concurrency.group` 引用了 `concurrency` 中不可用的 `env` 上下文，导致整个 workflow 校验失败、run 在 0 个 job 时即失败（无日志、无构建）；改为内联表达式后恢复发布 | [`75c0df0`](https://github.com/muutot/SecPivot/commit/75c0df0)
- **发布写权限收敛** — 发布作业 write 权限仅限发布步骤 | [`cdd779a`](https://github.com/muutot/SecPivot/commit/cdd779a)
- **CI 运行治理** — 取消被取代的运行并限制作业时长 | [`5326c60`](https://github.com/muutot/SecPivot/commit/5326c60) · [`2039a71`](https://github.com/muutot/SecPivot/commit/2039a71)
- **发布脚本** — 发布脚本不再升级全部依赖 | [`71e8e95`](https://github.com/muutot/SecPivot/commit/71e8e95)

## 测试与文档

- **前端门禁 fail-closed** — 前端测试门禁失败即阻断 | [`320e957`](https://github.com/muutot/SecPivot/commit/320e957)
- **剪贴板测试确定性** — 清理测试确定且无副作用 | [`a41895b`](https://github.com/muutot/SecPivot/commit/a41895b)
- **Argon2id 默认 KDF** — 端到端覆盖默认 KDF | [`0fc8696`](https://github.com/muutot/SecPivot/commit/0fc8696)
- **格式与文档** — TCATO 重复 title 移除、clippy 修复、取消契约行 prettier 重排 | [`3356c20`](https://github.com/muutot/SecPivot/commit/3356c20) · [`28169de`](https://github.com/muutot/SecPivot/commit/28169de) · [`e17ae30`](https://github.com/muutot/SecPivot/commit/e17ae30)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.6.2_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.6.2-portable.zip`（由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`）
- **Android APK**: 按 64 位 ABI 拆分签名的 release APK（`arm64-v8a`/`x86_64`，由 release 工作流 android job 在 Linux 并行构建）
