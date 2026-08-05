# SecPivot Desktop v1.1.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-05

---

## 新功能

- **历史版本查看器** — 详情面板以差异徽标直观对比版本，支持手动删除单个历史版本 | [`73faf5b1`](https://github.com/muutot/SecPivot/commit/73faf5b1)
- **受保护自定义字段按需解析** — 自定义受保护字段不再随快照下发，仅在需要时取回 | [`e492e2ee`](https://github.com/muutot/SecPivot/commit/e492e2ee)
- **条目存储占用统计** — 详情元信息展示条目存储使用量 | [`01bece84`](https://github.com/muutot/SecPivot/commit/01bece84)

## 安全加固

- **互斥锁中毒防护** — bridge/rpc 锁位置捕获处理器 panic，避免锁中毒导致服务不可用 | [`2be1377c`](https://github.com/muutot/SecPivot/commit/2be1377c)
- **主密钥零化与原子写** — 失败时清零新主密钥、原子写使用 fsync、退出时清剪贴板、TCATO 覆盖层事件补全 | [`a3ae08a1`](https://github.com/muutot/SecPivot/commit/a3ae08a1)
- **TCATO 焦点锁防护** — 覆盖层焦点锁守卫、空闲重装、CSPRNG 覆写 | [`63a539c8`](https://github.com/muutot/SecPivot/commit/63a539c8)

## 稳定性与修复

- **条目切换竞态修复** — 切换条目时丢弃陈旧的密码 / 自定义字段 / 存储获取结果 | [`943a5b8b`](https://github.com/muutot/SecPivot/commit/943a5b8b)
- **回收站分组永久删除** — 可永久删除已进回收站的分组 | [`1eda4148`](https://github.com/muutot/SecPivot/commit/1eda4148)
- **主界面审计修复** — 实时设置、选择、锁确认、favicon / 提交 / 快捷键守卫、TOTP 拆除 | [`5ab8f876`](https://github.com/muutot/SecPivot/commit/5ab8f876)

## 性能

- **批量导入** — CSV / XML 条目一次性通过单个 `import_entries` IPC 导入 | [`f0ea7fb4`](https://github.com/muutot/SecPivot/commit/f0ea7fb4)
- **分组子树计数预计算** — 单次 O(N) 遍历预计算子树条目数 | [`2b00c5d6`](https://github.com/muutot/SecPivot/commit/2b00c5d6)

## 界面

- **详情面板微调** — 细节排版优化 | [`6c82556f`](https://github.com/muutot/SecPivot/commit/6c82556f)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.1.0_x64-setup.exe`
- **便携版 (免安装)**: `SecPivot-1.1.0-portable.zip`(由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`)
