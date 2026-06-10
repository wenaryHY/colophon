# Colophon Production Hardening

这份清单用于把 Colophon 从“能跑”收敛到更稳的生产部署。目标是：应用进程不以 root 运行，服务文件不可被普通用户改写，生产目录只包含运行所需文件，密钥不写进 systemd unit。

## 1. 创建运行用户和目录

```bash
adduser --system --group --home /var/lib/colophon colophon
install -d -o colophon -g colophon -m 0750 /var/lib/colophon
install -d -o colophon -g colophon -m 0750 /var/lib/colophon/uploads
install -d -o colophon -g colophon -m 0750 /var/backups/colophon
install -d -o root -g root -m 0755 /opt/colophon
install -d -o root -g root -m 0750 /etc/colophon
```

## 2. 发布最小运行包

生产目录建议只保留这些内容：

```text
/opt/colophon/
├── colophon
├── config/
├── migrations/
├── themes/
└── src/admin/dist/
```

不要把 `.git`、`.idea`、`node_modules`、`target`、测试报告或源码工作区整体放到生产目录。需要排障时可以保留构建产物版本号和提交 SHA，但不要依赖服务器上的源码树作为发布方式。

## 3. 配置密钥

```bash
openssl rand -hex 32 > /tmp/colophon_secret
install -o root -g root -m 0600 /dev/null /etc/colophon/colophon.env
printf 'COLOPHON__AUTH__SECRET=%s\n' "$(cat /tmp/colophon_secret)" > /etc/colophon/colophon.env
rm -f /tmp/colophon_secret
```

`COLOPHON__RUNTIME__MODE=production` 会阻止默认 JWT secret 启动。不要在 `/etc/systemd/system/colophon.service` 里直接写真实密钥。

## 4. 安装 systemd 服务

仓库提供了模板：`deploy/colophon.service`。

```bash
install -o root -g root -m 0644 deploy/colophon.service /etc/systemd/system/colophon.service
systemctl daemon-reload
systemctl enable --now colophon
systemctl status colophon --no-pager
```

关键检查：

```bash
stat -c '%a %U:%G %n' /etc/systemd/system/colophon.service
systemctl show colophon -p User -p Group -p EnvironmentFiles
ss -tulpn | grep ':2000'
```

期望结果：

- 服务文件权限是 `644 root:root`。
- 进程用户是 `colophon`。
- 应用只监听 `127.0.0.1:2000`，由 Nginx 暴露公网入口。

## 5. Nginx 反代要点

```nginx
server {
    listen 80;
    server_name example.com;

    location / {
        proxy_pass http://127.0.0.1:2000;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /ws/ {
        proxy_pass http://127.0.0.1:2000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

上线 HTTPS 后，确认 `X-Forwarded-Proto` 传递的是 `https`，并保留应用自身的安全 headers。

## 6. 快速巡检

```bash
curl -fsS http://127.0.0.1:2000/api/v1/health
curl -fsS -I http://127.0.0.1/api/v1/health
journalctl -u colophon -n 100 --no-pager
```

如果启用备份复制，优先使用 Docker 镜像里的 Litestream 链路，或者在裸机上单独安装 Litestream 并把 SQLite 数据库路径指向 `/var/lib/colophon/colophon.db`。
