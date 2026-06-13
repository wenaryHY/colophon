# 快速开始指南

## Windows 用户

### 方法 1：使用 PowerShell（推荐）

```powershell
# 运行基准测试
.\benches\run_benchmarks.ps1
```

### 方法 2：使用 Git Bash

```bash
# 运行所有测试
bash benches/run_all.sh

# 或单独运行 Criterion 测试
cargo bench --bench api_benchmarks
```

### 内存监控（单独窗口）

**PowerShell:**
```powershell
.\benches\monitor_memory.ps1
```

**Git Bash:**
```bash
bash benches/monitor_memory.sh > memory.csv
```

---

## Linux/macOS 用户

### 一键运行

```bash
bash benches/run_all.sh
```

### 分步运行

```bash
# 1. Criterion 微基准测试
cargo bench --bench api_benchmarks

# 2. 启动服务器（新终端）
cargo run --release

# 3. 负载测试
bash benches/load_test.sh

# 4. 内存监控（新终端）
bash benches/monitor_memory.sh > memory.csv
```

---

## 安装负载测试工具

### wrk（推荐）

**Ubuntu/Debian:**
```bash
sudo apt install wrk
```

**macOS:**
```bash
brew install wrk
```

**Windows:**
- 使用 WSL：`wsl sudo apt install wrk`
- 或下载：https://github.com/wg/wrk/releases

### bombardier（跨平台替代）

下载地址：https://github.com/codesenberg/bombardier/releases

**Windows:**
1. 下载 `bombardier-windows-amd64.exe`
2. 重命名为 `bombardier.exe`
3. 放入 PATH 或项目根目录

**Linux:**
```bash
wget https://github.com/codesenberg/bombardier/releases/latest/download/bombardier-linux-amd64
chmod +x bombardier-linux-amd64
sudo mv bombardier-linux-amd64 /usr/local/bin/bombardier
```

**macOS:**
```bash
brew install bombardier
```

---

## 查看结果

### Criterion 报告

```bash
# 浏览器打开
open target/criterion/report/index.html        # macOS
xdg-open target/criterion/report/index.html    # Linux
start target\criterion\report\index.html       # Windows CMD
```

### 负载测试报告

```bash
# 查找最新报告
ls -lt benches/reports/

# 分析结果
bash benches/analyze_results.sh benches/reports/20260613_120000
```

### 对比基线

```bash
# 对比两次测试
bash benches/compare_results.sh benches/reports/20260613_140000 benches/reports/baseline
```

---

## 常见问题

### Q: 编译时间很长？

A: 首次编译 Criterion 需要 5-10 分钟，后续只需几秒。可以加速：

```bash
# 使用 sccache 缓存编译结果
cargo install sccache
export RUSTC_WRAPPER=sccache

# 或使用 mold 链接器（Linux）
cargo build --release
```

### Q: wrk 无法在 Windows 运行？

A: 使用以下任一方法：
1. 在 WSL 中运行：`wsl bash benches/load_test.sh`
2. 使用 bombardier：`bash benches/load_test_bombardier.sh`
3. 使用 PowerShell 脚本：`.\benches\run_benchmarks.ps1`

### Q: 服务器连接失败？

A: 确保：
1. 服务器正在运行：`cargo run --release`
2. 端口正确：默认 3000，可通过环境变量修改
   ```bash
   export COLOPHON_URL=http://localhost:YOUR_PORT
   ```
3. 防火墙未阻止：`netstat -an | grep 3000`

### Q: 如何只运行特定基准测试？

A: 使用 `--` 传递过滤器：

```bash
# 只运行 query 相关的测试
cargo bench --bench api_benchmarks -- query

# 只运行 JSON 序列化测试
cargo bench --bench api_benchmarks -- json
```

---

## 下一步

1. **记录基线指标**：编辑 `benches/README.md`，填入当前系统的基线数据
2. **设置 CI**：在 GitHub Actions 中自动运行基准测试
3. **性能优化**：根据报告识别瓶颈并优化

详细文档：`benches/README.md`
