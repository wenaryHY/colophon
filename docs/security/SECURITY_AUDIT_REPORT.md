# Colophon 安全审计报告

**审计日期**: 2026-06-09 10:09  
**审计范围**: Colophon v1.0.0 (Rust CMS)  
**审计方法**: 静态代码分析 + 架构审查  
**严重等级**: P0（严重）> P1（高危）> P2（中危）> P3（低危）

---

## 执行摘要

Colophon 采用 Rust + Axum 框架，整体安全架构**良好**，多数 OWASP Top 10 风险已有防御措施。主要优势在于使用类型安全的 sqlx、argon2 密码哈希、ammonia XSS 防护。

**关键发现**：
- ✅ 0 个 P0 漏洞
- ⚠️ 2 个 P1 高危漏洞（Webhook SSRF）
- ⚠️ 3 个 P2 中危风险
- ℹ️ 2 个 P3 低危问题

**总体评分**: 7.5/10（良好，需修复高危项）

---

## 🔴 P1 高危漏洞

### 1. Webhook SSRF（Server-Side Request Forgery）

**位置**: src/modules/webhook/service.rs:329-343

**问题描述**:
Webhook 功能允许管理员配置任意 URL，系统会向该 URL 发送 HTTP POST 请求。当前实现**没有过滤私有 IP 段**，攻击者可以：
- 探测内网服务（127.0.0.1, 10.0.0.0/8, 192.168.0.0/16, 169.254.0.0/16）
- 攻击内网未授权的 API
- 绕过防火墙扫描内部端口

**受影响代码**:
`ust
// src/modules/webhook/service.rs:384
if url::Url::parse(body.url.trim()).is_err() {
    return Err(AppError::BadRequest("invalid webhook URL".into()));
}
// ❌ 仅校验 URL 格式，未检查目标 IP
`

**攻击场景**:
`ash
# 管理员创建 webhook 指向内网
POST /api/v1/admin/webhooks
{
  "name": "exploit",
  "url": "http://127.0.0.1:6379/",  # 攻击 Redis
  "events": "post.after_publish"
}
# 发布文章触发 webhook → Colophon 请求 Redis → 信息泄露
`

**修复建议**:
`ust
// 在 create_webhook / update_webhook 中添加
fn is_safe_webhook_url(url: &str) -> Result<(), AppError> {
    let parsed = url::Url::parse(url)?;
    
    // 仅允许 http/https
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::BadRequest("仅支持 http/https".into()));
    }
    
    // 解析主机名为 IP
    if let Some(host) = parsed.host_str() {
        // 拒绝 localhost
        if host == "localhost" || host == "127.0.0.1" || host == "::1" {
            return Err(AppError::BadRequest("禁止回环地址".into()));
        }
        
        // 如果是 IP 地址，检查私有段
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(v4) => {
                    if v4.is_private() || v4.is_loopback() || v4.is_link_local() {
                        return Err(AppError::BadRequest("禁止私有 IP".into()));
                    }
                }
                std::net::IpAddr::V6(v6) => {
                    if v6.is_loopback() {
                        return Err(AppError::BadRequest("禁止回环地址".into()));
                    }
                }
            }
        }
    }
    Ok(())
}
`

**优先级**: **P1 - 立即修复**

---

### 2. Webhook URL 重定向跟随风险

**位置**: src/modules/webhook/service.rs:37-40

**问题描述**:
reqwest Client 默认**跟随重定向**，攻击者可以：
1. 创建 webhook 指向公网合法域名 https://evil.com/redirect
2. evil.com 返回 302 → http://192.168.1.100:8080/admin
3. Colophon 跟随重定向攻击内网

**修复建议**:
`ust
reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .redirect(reqwest::redirect::Policy::none())  // ✅ 禁止重定向
    .build()
`

**优先级**: **P1 - 立即修复**

---

## ⚠️ P2 中危风险

### 3. CSP 允许 'unsafe-inline' 脚本

**位置**: src/shared/security.rs:20

**问题描述**:
主题 HTML 页面的 CSP 策略包含 script-src 'self' 'unsafe-inline'，这**削弱了 XSS 防护**：
- 虽然 ammonia 清理 HTML，但如果清理器被绕过，攻击者可以注入内联脚本
- 违反 CSP 最佳实践

**优先级**: **P2 - 建议修复**

---

### 4. 默认 JWT Secret 在生产环境可被绕过

**位置**: src/bootstrap/config.rs:119-136

**问题描述**:
代码检测不安全的默认 JWT secret，但允许通过 COLOPHON__AUTH__ALLOW_INSECURE_DEFAULT_SECRET=true 绕过。

**修复建议**: 完全移除 allow_insecure_default_secret 选项，生产环境必须设置 COLOPHON__AUTH__SECRET。

**优先级**: **P2 - 建议修复**

---

### 5. 主题上传 Zip Bomb 风险

**位置**: src/modules/theme/handler.rs:116-167

**问题描述**:
主题上传功能解压 ZIP 文件时**没有大小限制**，可能导致磁盘空间耗尽（DoS）。

**修复建议**: 限制解压后总大小不超过 100MB。

**优先级**: **P2 - 建议修复**

---

## ✅ 安全优势（已实施）

| 安全措施 | 实施位置 | OWASP 分类 |
|---------|---------|-----------|
| **Argon2 密码哈希** | src/infra/hash.rs | A02:认证失效 |
| **sqlx 参数化查询** | 全项目 | A03:注入攻击 |
| **ammonia XSS 防护** | src/shared/content.rs | A03:XSS |
| **JWT + AdminUser 提取器** | src/shared/auth.rs | A01:访问控制 |
| **CORS 白名单** | src/bootstrap/router.rs | A05:安全配置错误 |
| **HttpOnly + SameSite Cookie** | src/modules/auth/handler.rs | A02:认证失效 |
| **登录速率限制** | src/shared/security.rs | A07:认证攻击 |
| **路径遍历检查** | src/modules/backup/handler.rs | A01:路径遍历 |
| **Zip Slip 防护** | src/modules/theme/handler.rs | A08:不安全反序列化 |
| **文件上传 MIME 白名单** | src/modules/media/service.rs | A04:不安全设计 |

---

## 📊 OWASP Top 10 对照

| OWASP 分类 | 风险等级 | 发现数 | 备注 |
|-----------|---------|-------|------|
| A01:访问控制失效 | ✅ 低 | 0 | AdminUser 提取器覆盖良好 |
| A02:加密失效 | ✅ 低 | 0 | Argon2 + JWT |
| A03:注入 | ✅ 低 | 0 | sqlx 类型安全查询 |
| A04:不安全设计 | ⚠️ 高 | 2 | Webhook SSRF（P1） |
| A05:安全配置错误 | ⚠️ 中 | 2 | CSP unsafe-inline（P2） |
| A07:认证失效 | ⚠️ 中 | 1 | 默认 secret 可绕过（P2） |
| A10:SSRF | ⚠️ 高 | 2 | Webhook 未过滤私有 IP（P1） |

---

## 🔧 修复优先级路线图

### 立即修复（本周）
1. **Webhook SSRF** - 添加私有 IP 黑名单
2. **禁用重定向跟随** - reqwest 配置

### 下周修复
3. **移除 CSP unsafe-inline** - 使用 nonce
4. **强制 JWT secret** - 移除 allow_insecure_default_secret
5. **Zip Bomb 防护** - 限制解压大小

---

## 🎯 总结

Colophon 的安全架构**基础扎实**，Rust 的类型安全特性避免了大部分内存安全和注入漏洞。主要问题集中在应用层逻辑漏洞（Webhook SSRF）和配置松散（CSP、默认 secret）。

修复 P1 高危项后，安全评分可提升至 **8.5/10**。

---

**审计人员**: Kiro AI Security Audit  
**下次审计**: 建议每季度审计一次