# 域名根地址首页修复

2026-10-11。修复直接访问 `https://understandbook.top/` 仍显示登录页的问题。

## 路由

**决策**：根地址显示展示首页，阅读器使用 `/index.html`。

**否决**：只发布 `/home.html`：用户直接打开域名仍无法看到首页。

**命门**：已有带参数的根地址链接转到 `/index.html`，保留账号、密码找回和独立讲解窗口参数；登录后刷新继续进入阅读器。

`scripts/linux/nginx-multi-reader.conf.example` 与线上 Nginx 同步增加精确根路径规则：无查询参数返回 `home.html`，带参数请求以 302 转到阅读器入口，响应保持 `no-store`。原 `/home.html` 仍可访问，首页已有登录和注册链接沿用。

## 发布与验证

08:22（香港时间）配置检查通过后平滑重载 Nginx，后端 PID 456214 保持不变。配置备份在 `/opt/understand-book/backups/home-root-20261011T002230Z`；回退恢复其中 `nginx.conf` 并检查配置、重载 Nginx。

浏览器验收覆盖手机尺寸根地址首页、注册入口、既有账号与独立窗口链接的参数保留、真实账号登录及刷新后的书架。结果见 [live-verification.json](live-verification.json)，发布信息见 [deployment.json](deployment.json)，画面见 [手机首页](root-mobile.png)。此次仅调整入口路由，没有重新构建应用或发起模型请求。

公网 HTTP 验证根地址返回 200、`no-store` 且正文与 `/home.html` 相同。浏览器最初受本机代理连接中断影响；关闭代理并将同一 HTTPS 域名解析到源站后，完整交互验证通过，保留证书校验，无页面脚本异常。
