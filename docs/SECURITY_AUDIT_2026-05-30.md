# InkForge v1.0.0 安全审计报告

**日期:** 2026-05-30
**审计范围:** https://wenary.me + GitHub 仓库

---

## 自动化扫描

| 工具 | 版本 | 模板/探测数 | 结果 |
|------|------|-------------|------|
| nuclei | v3.8.0 | 6220 | 0 真实漏洞 (1 误报: CVE-2026-33017 Langflow) |
| XSStrike | v3.1.5 | 全参数 | 0 |
| DalFox (Go) | v2.13.0 | 全站 | 0 |
| sqlmap | v1.9.6 | 全部测试 | 0 注入点 + Cloudflare WAF 拦截 89 次 |
| wafw00f | v2.4.2 | — | Cloudflare |
| katana | v1.6.1 | 全站爬虫 | 0 敏感端点泄露 |
| httpx | latest | 3 端点 | 公开 API 200, admin API 405 |
| testssl.sh | latest | TLS 深度 | TLS 1.3 + Forward Secrecy, 无弱密码 |

## 仓库扫描

| 工具 | 方法 | 结果 |
|------|------|------|
| Gitleaks | 文件系统扫描 2.3MB | 0 泄露 |
| TruffleHog v3.88.28 | 7248 文件块深度扫描 | 0 verified, 0 unverified |

## 手工渗透

| 端点 | 结果 |
|------|------|
| /.env, /.git/config, /wp-admin | 全部 401 |
| /api/v1/admin/plugins/* (未认证) | 全部 401 (AdminUser 中间件生效) |
| /api/v1/admin/backup (未认证) | 401 |
| 暴力登录 (admin:wrong) | 401 (限流器生效) |

## 代码层面修复记录

| 轮次 | 修复数 | 内容 |
|------|--------|------|
| 第一轮 | 5 | 插件 API 认证 + 评论净化 + CSP + Secure Cookie |
| 第二轮 | 5 | 插件路由 AdminUser + Cookie 清除 Secure + HTML sanitize + 登录 Cookie 恢复 |
| 架构加固 | 3 | OAuth2 refresh token + Cookie 分离 + refresh rotation |

## 安全 Headers (Cloudflare 回源)

```
Content-Security-Policy: ✅ 已设置
X-Content-Type-Options: nosniff ✅
X-Frame-Options: SAMEORIGIN ✅
Permissions-Policy: camera/microphone/geolocation=() ✅
Cross-Origin-Resource-Policy: same-origin ✅
Referrer-Policy: strict-origin-when-cross-origin ✅
```

## 结论

InkForge v1.0.0 安全基线通过自动化 + 手工双重验证。
