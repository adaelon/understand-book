# understandbook.top 域名接入 — 2026-10-09

正式阅读器：[https://understandbook.top/](https://understandbook.top/)。管理端：[https://understandbook.top/admin/](https://understandbook.top/admin/)。生产主机 `115.190.121.150`，release `/opt/understand-book/releases/adm10-20261008`，数据根 `/opt/understand-book/data/multi-reader`。

## 当前配置

用户配置 Cloudflare 根域名 A 记录到生产主机，橙色云朵代理开启，并确认加密模式 Full (strict)、Always Use HTTPS 开启。公网 HTTPS 响应实际经过 Cloudflare，浏览器侧证书由 Cloudflare 管理。

源站证书 `/etc/nginx/ssl/understandbook.top.pem`，私钥 `/etc/nginx/ssl/understandbook.top.key`；SAN 包含 `understandbook.top` 和 `*.understandbook.top`，有效期 2026-10-09 04:37 UTC 至 2041-10-05 04:37 UTC。证书 0644、私钥 0600、目录 0700。官方 RSA Origin CA 根证书保存在 `/etc/nginx/ssl/cloudflare-origin-ca-rsa.pem`，用于源站 TLS 验收；没有修改系统全局信任库。

生产 Nginx 配置 `/etc/nginx/conf.d/understand-book-multi.conf` 使用域名与新证书，`/api/` 转发 Host 为 `understandbook.top`，后端仍监听 `127.0.0.1:8788`。环境文件 `/opt/understand-book/multi-reader.env` 的 Origin 改为 `https://understandbook.top`；服务已重启加载配置，Nginx 已验证并 reload。切换前检查在途运行数为 0。

域名的 HTTP 入口跳转到 HTTPS；旧 IP 的 HTTPS 入口保留原 Let's Encrypt IP 证书并跳转到域名，保留路径和查询参数。IP 站点作为 443 的默认站点，保证不发送域名 SNI 的旧 IP 访问也能选中原 IP 证书。旧 IP 的续期定时器与既有 80 端口站点配置沿用。

## 验收

| 路径 | 结果 |
| --- | --- |
| 源站 TLS | 官方 Origin CA 验证证书链和域名通过；证书公钥与私钥公钥一致 |
| 源站页面与静态资源 | 阅读器、后台、后台深链、各自 JS/CSS 均 200；缺失资源 404 |
| Host / Origin | 新域名 Origin 进入认证；旧 IP Origin 写请求返回 ORIGIN_REJECTED |
| 源站身份 | 已有 admin 登录、身份读取、后台汇总和退出均 200；匿名私人 API 返回 401 |
| 公网经 Cloudflare | 根页面、后台、后台深链 200；TLS 校验通过；匿名身份 API 401 |
| 公网登录与后台 | 现有 admin 登录、身份读取、后台汇总、带 CSRF 的退出均 200 |
| 跳转 | 源站域名 HTTP 与旧 IP HTTPS 保留 `/admin/` 路径返回 308 到域名 |
| 常驻服务 | nginx 与 understand-book-multi 均 active |

源站回执 [server-configuration.json](server-configuration.json)，公网回执 [public-verification.json](public-verification.json)，生产配置的公开副本 [nginx.conf](nginx.conf)。远端回执 `/opt/understand-book/acceptance/domain-20261009/server-configuration.json`。凭据与 Cookie 不进入这些文件，测试会话已退出，模型请求 0。

## 配置回退

切换前备份 `/opt/understand-book/backups/domain-20261009T045337Z`，包含原 Nginx 配置与私有环境文件，目录 0700、环境备份 0600。需要恢复旧 IP 正式入口时执行：

```bash
cp -p /opt/understand-book/backups/domain-20261009T045337Z/understand-book-multi.conf /etc/nginx/conf.d/understand-book-multi.conf
cp -p /opt/understand-book/backups/domain-20261009T045337Z/multi-reader.env /opt/understand-book/multi-reader.env
nginx -t && systemctl restart understand-book-multi && systemctl reload nginx
```

该回退恢复旧 IP 入口和旧 Origin，域名入口需另行重新配置；生产程序版本与数据根继续使用 ADM10。

## 已知限制

- 此次未调用模型验证回答流，也未写入笔记；已验证域名下登录、身份、后台读取与退出。
- 当前内置浏览器导航超时，未完成页面渲染的可视化验收；页面、资源和认证通过 HTTPS 请求实际验证。
- 当前只配置根域名 DNS；`www.understandbook.top` 没有作为入口启用。
- 源站 Origin CA 证书依赖 Cloudflare 代理；浏览器直连源站需要公开可信的域名证书。
