# 注册页面与密码规则 · 2026-10-27

新密码统一为 **8–64 个字符**，不强制大小写、数字或特殊符号组合。前端使用 Unicode 码点计数，服务端使用 Rust `chars()`，中文按字符计算。既有密码的登录与当前密码验证沿用原限制。

注册页采用暖白纸张背景、书页插画和独立表单卡片；桌面双栏、手机单栏。增加填写信息／验证邮箱两步进度、密码显示切换、输入项旁的错误提示与错误输入定位。保留一次性内测码、邮箱验证和注册赠送 10 元／30 天额度。找回、重置页面共用该布局，个人设置修改密码共用新规则。

## 实现与验证

- `packages/web/src/components/AccountAccess.vue`：布局、输入反馈、密码显示、验证码自动聚焦及准确的重发倒计时。
- `packages/web/src/account-forms.ts`、`crates/server/src/auth.rs:encode_password`：创建与修改密码规则同步；`PasswordChange.vue` 自动复用前端规则。
- `packages/i18n/src/en.json`：新增中英文界面文案。
- 19 项账号组件测试、25 项服务端认证／注册／改密测试通过。注册与重置现有测试使用 8 字符密码；覆盖字符上下界、中文、原有长密码登录和当前密码验证。
- 最终发布前端通过类型检查和多人生产构建；9 项浏览器检查覆盖桌面、手机、中英文、验证输入、邮箱验证、返回登录及短窗口找回／重置。
- 直接查看桌面、手机、窄屏底部、错误提示与邮箱验证截图，确认排版与交互；证据见 [页面验证](visual-qa.json)、[构建日志](web-build.log)。

## 发布状态

正式发布包基于线上 `registration-gift-20261027` 源码加入本次修改，排除了本机另外 9 处未发布差异；见 [源码范围](source-comparison.json)。Linux `server` 与 `manage_reader` 正在编译，候选目录为 `/opt/understand-book/releases/account-access-20261027`。

![桌面注册页](registration-desktop.png)

![手机注册页](registration-mobile.png)
