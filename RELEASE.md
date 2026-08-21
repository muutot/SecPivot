# SecPivot Desktop v1.3.2

> 专业、紧凑、信息密度高的 KeePass 桌面客户端，本地优先、无同步上传
>
> Released: 2026-08-21

---

## 详情编辑与链接识别

- **双击内联编辑** — 双击详情字段即可进入内联编辑 | [`53e36893`](https://github.com/muutot/SecPivot/commit/53e36893)
- **笔记联系人识别** — 笔记中的联系人信息渲染为内联可点击项，并提供双模式编辑器 | [`9ea4c80a`](https://github.com/muutot/SecPivot/commit/9ea4c80a)
- **链接检测** — 新增可配置链接颜色，自动识别 URL / 邮箱 / 电话并转为可点击链接 | [`42bb961d`](https://github.com/muutot/SecPivot/commit/42bb961d)
- **笔记可靠性** — 快照替换时保留笔记草稿；自动保存进行期间的输入不再丢失 | [`2cd78b5f`](https://github.com/muutot/SecPivot/commit/2cd78b5f)、[`b18daf5b`](https://github.com/muutot/SecPivot/commit/b18daf5b)

---

## 附件

- **拖放添加** — 附件区域支持直接拖放文件添加附件 | [`b8e562dd`](https://github.com/muutot/SecPivot/commit/b8e562dd)

---

## 界面与主题

- **配色重制** — 重制强调色 / 选区色 / 链接色的默认调色板 | [`87be965a`](https://github.com/muutot/SecPivot/commit/87be965a)
- **列表前景色** — 编辑器条目提示优化，列表中渲染自定义前景色 | [`16d0098b`](https://github.com/muutot/SecPivot/commit/16d0098b)
- **更紧凑的行高** — 桌面端条目行高降至 30px | [`38213430`](https://github.com/muutot/SecPivot/commit/38213430)
- **欢迎屏焕新** — 应用 Logo 取代钥匙图标并置于标题旁，内容整体下移窗口高度 10% | [`fe3ed396`](https://github.com/muutot/SecPivot/commit/fe3ed396)、[`ae14ddfb`](https://github.com/muutot/SecPivot/commit/ae14ddfb)、[`e3671899`](https://github.com/muutot/SecPivot/commit/e3671899)

---

## 性能与安全

- **后端增量共享** — vault 增量共享未触碰的子树；打开 / 创建 / 保存 / 改密移至阻塞池执行 | [`283c6ebd`](https://github.com/muutot/SecPivot/commit/283c6ebd)、[`33ffb3c3`](https://github.com/muutot/SecPivot/commit/33ffb3c3)
- **树视图** — 每快照一次遍历 memoize 组 diff 与 reveal | [`a98f012f`](https://github.com/muutot/SecPivot/commit/a98f012f)
- **搜索与表格** — 搜索键入间 memoize 排序键；列宽拖动经 CSS 变量实时调整、释放时提交 | [`30fe085c`](https://github.com/muutot/SecPivot/commit/30fe085c)、[`668c274b`](https://github.com/muutot/SecPivot/commit/668c274b)
- **资源防护** — favicon 流式传输时强制大小上限；过期条目通知去重集设上限 | [`765a4021`](https://github.com/muutot/SecPivot/commit/765a4021)、[`184a2f53`](https://github.com/muutot/SecPivot/commit/184a2f53)
- **空闲锁** — 仅在 `autoLockMinutes` 变化时重新布防空闲锁 | [`f6fb1cd8`](https://github.com/muutot/SecPivot/commit/f6fb1cd8)

---

## 稳定性修复

- **S3 远程** — 无效端点返回错误而非在 `host_header` 中 panic | [`2b33d0ca`](https://github.com/muutot/SecPivot/commit/2b33d0ca)
- **OTP** — 超出范围的位数拒绝生成而非输出错误验证码 | [`8280868d`](https://github.com/muutot/SecPivot/commit/8280868d)
- **树视图 reveal** — 行尚未挂载时重试定位，清理时取消 rAF | [`02c1bd5a`](https://github.com/muutot/SecPivot/commit/02c1bd5a)
- **浏览器桥接** — 清除残留的审批提示关闭定时器 | [`c7ec37c0`](https://github.com/muutot/SecPivot/commit/c7ec37c0)

---

## 构建产物

- **NSIS 安装包**: `SecPivot_1.3.2_x64-setup.exe`
- **便携版 ZIP**: `SecPivot-1.3.2-portable.zip`(解压即用，配置存于 exe 旁 `conf/`)
- **Android APK**: 四 ABI universal release APK(`apksigner verify` 校验后发布)
