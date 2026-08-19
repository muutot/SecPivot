# SecPivot Desktop v1.3.0

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-19

---

## 会话锁并发加固

- **长操作移出会话锁** — 写持久化、设置重加密、远程刷新/合并、附件临时导入读取等移出 session 锁，锁内仅轻量 clone+提交，缩短持锁时间 | [`fb7ad9b`](https://github.com/muutot/SecPivot/commit/fb7ad9b) · [`e8ca2c7`](https://github.com/muutot/SecPivot/commit/e8ca2c7) · [`a7a364c`](https://github.com/muutot/SecPivot/commit/a7a364c) · [`e336550`](https://github.com/muutot/SecPivot/commit/e336550)
- **按会话隔离渲染器操作** — 长操作绑定会话 id、页面变更按会话门控、提交/拓扑/快照按会话稳定化，切页与并发不再串扰 | [`ddbba9a`](https://github.com/muutot/SecPivot/commit/ddbba9a) · [`9cb0498`](https://github.com/muutot/SecPivot/commit/9cb0498) · [`aa9a458`](https://github.com/muutot/SecPivot/commit/aa9a458) · [`944eebb`](https://github.com/muutot/SecPivot/commit/944eebb) · [`51a4ee2`](https://github.com/muutot/SecPivot/commit/51a4ee2) · [`ebe763a`](https://github.com/muutot/SecPivot/commit/ebe763a) · [`607f5a4`](https://github.com/muutot/SecPivot/commit/607f5a4)
- **过期审批令牌清理** — 移除已过期关联令牌 | [`28575d5`](https://github.com/muutot/SecPivot/commit/28575d5)
- **set-login 语义对齐** — 关联密钥会话更新语义收敛 | [`0151177`](https://github.com/muutot/SecPivot/commit/0151177)

## 关联审批与秘密读取

- **两阶段关联审批** — 审批期间不再持有 vault 锁，并对会话切换做稳定性防护 | [`9893f4a`](https://github.com/muutot/SecPivot/commit/9893f4a) · [`ee99585`](https://github.com/muutot/SecPivot/commit/ee99585)
- **秘密读取绑定视图** — 详情复制、明文导出、附件预览/保存、保存为选择器均绑定视图生命周期，拒绝跨条目/会话的过期响应 | [`b0694c9`](https://github.com/muutot/SecPivot/commit/b0694c9) · [`740751d`](https://github.com/muutot/SecPivot/commit/740751d) · [`e324ea1`](https://github.com/muutot/SecPivot/commit/e324ea1) · [`823f05d`](https://github.com/muutot/SecPivot/commit/823f05d) · [`a2b867a`](https://github.com/muutot/SecPivot/commit/a2b867a) · [`aa34933`](https://github.com/muutot/SecPivot/commit/aa34933)
- **过期密码复制防护** — 拒绝已失效的密码副本 | [`4e59208`](https://github.com/muutot/SecPivot/commit/4e59208)
- **TCATO 焦点锁归属保持** — 焦点锁所有权随会话稳定 | [`9d79ed8`](https://github.com/muutot/SecPivot/commit/9d79ed8)
- **加载后揭示秘密** — 仅加载完成后揭示，拒绝提前响应 | [`1936f51`](https://github.com/muutot/SecPivot/commit/1936f51) · [`728c34f`](https://github.com/muutot/SecPivot/commit/728c34f)
- **附加修复** — 对话框临时文件清理、保存生命周期 await、过期保存目标拒绝、嵌套对话框复位 | [`ae70862`](https://github.com/muutot/SecPivot/commit/ae70862) · [`bd8dadc`](https://github.com/muutot/SecPivot/commit/bd8dadc) · [`9c29bd3`](https://github.com/muutot/SecPivot/commit/9c29bd3) · [`b432949`](https://github.com/muutot/SecPivot/commit/b432949) · [`bd9de36`](https://github.com/muutot/SecPivot/commit/bd9de36) · [`1102817`](https://github.com/muutot/SecPivot/commit/1102817) · [`a538be0`](https://github.com/muutot/SecPivot/commit/a538be0) · [`6a450cd`](https://github.com/muutot/SecPivot/commit/6a450cd) · [`4db2371`](https://github.com/muutot/SecPivot/commit/4db2371)

## 安全与密码生成器

- **受保护/可写字段契约** — 限制可写 cipher 契约与受保护自定义字段按需解析 | [`7ff1fd7`](https://github.com/muutot/SecPivot/commit/7ff1fd7) · [`e492e2e`](https://github.com/muutot/SecPivot/commit/e492e2e)
- **生成器约束强制** — 服务端强制配置的字符集/必含约束，编辑器状态最终选中 | [`a19f833`](https://github.com/muutot/SecPivot/commit/a19f833) · [`5d7c3bc`](https://github.com/muutot/SecPivot/commit/5d7c3bc)
- **会话锁密钥保持** — RPC 会话密钥仅跨锁定保留，编辑器标志持久化 | [`ad065bd`](https://github.com/muutot/SecPivot/commit/ad065bd) · [`f877aad`](https://github.com/muutot/SecPivot/commit/f877aad)

## 发布工具链与 CI

- **发布版本一致性校验** — 提交/标签前强制 package.json / tauri.conf.json / Cargo.toml 三者版本一致 | [`f725d57`](https://github.com/muutot/SecPivot/commit/f725d57) · [`c928842`](https://github.com/muutot/SecPivot/commit/c928842)
- **发布脚本健壮化** — 隔离发布提交文件、dry-run 只读、语义 bump 目标复用、仅再生才 force-push、子进程无 shell 传参、再生祖先校验 | [`6cc7d98`](https://github.com/muutot/SecPivot/commit/6cc7d98) · [`faf3dd2`](https://github.com/muutot/SecPivot/commit/faf3dd2) · [`2a89362`](https://github.com/muutot/SecPivot/commit/2a89362) · [`9e33321`](https://github.com/muutot/SecPivot/commit/9e33321) · [`e6f5eaa`](https://github.com/muutot/SecPivot/commit/e6f5eaa) · [`063357a`](https://github.com/muutot/SecPivot/commit/063357a) · [`bba1201`](https://github.com/muutot/SecPivot/commit/bba1201) · [`86a6828`](https://github.com/muutot/SecPivot/commit/86a6828)
- **Android 发布流水线修复** — APK 资产经 Actions artifact 跨 job、发布等待两构建器、upload 显式指定 repo | [`bd2a03e`](https://github.com/muutot/SecPivot/commit/bd2a03e) · [`055d8bd`](https://github.com/muutot/SecPivot/commit/055d8bd) · [`a945917`](https://github.com/muutot/SecPivot/commit/a945917)
- **Android 签名配置验证** + 仅安卓的死代码/未用变量告警消除 | [`42eb7c7`](https://github.com/muutot/SecPivot/commit/42eb7c7) · [`ea81e94`](https://github.com/muutot/SecPivot/commit/ea81e94)
- **CI 升级** — actions 升级 Node 24 运行时、前端行为测试纳入 verify、Rust cache 输入修正 | [`9530e41`](https://github.com/muutot/SecPivot/commit/9530e41) · [`c225b6c`](https://github.com/muutot/SecPivot/commit/c225b6c) · [`820c74a`](https://github.com/muutot/SecPivot/commit/820c74a)

## 界面与工程规范

- **应用图标更新 v22** — 头部横杆延伸至外环 | [`f2f68a3`](https://github.com/muutot/SecPivot/commit/f2f68a3)
- **prettier 以 jsonc 处理 JSON** — tauri.conf.json 豁免尾逗号重排 | [`3a8bf58`](https://github.com/muutot/SecPivot/commit/3a8bf58) · [`046ba9b`](https://github.com/muutot/SecPivot/commit/046ba9b)
- **LF 行尾强制** — .gitattributes 与 editorconfig/测试正则对齐 | [`95c9a0a`](https://github.com/muutot/SecPivot/commit/95c9a0a) · [`5ad9490`](https://github.com/muutot/SecPivot/commit/5ad9490) · [`0a22728`](https://github.com/muutot/SecPivot/commit/0a22728)
- **发布名精简** — release 名称仅版本号 | [`0d9ec15`](https://github.com/muutot/SecPivot/commit/0d9ec15)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.3.0_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.3.0-portable.zip`(由 `scripts/package-portable.ps1` 生成，解压即用，配置存于 exe 旁 `conf/`)
- **Android APK**: `app-release.apk`(release 签名，由 release 工作流的 `android` job 在 Linux 上并行构建)
