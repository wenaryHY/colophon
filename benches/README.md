# Colophon Performance Benchmarks

性能基准测试框架，用于测量和跟踪 Colophon 的性能指标，为重构提供回归检测基准。

## 目录

- [快速开始](#快速开始)
- [基准测试类型](#基准测试类型)
- [工具要求](#工具要求)
- [使用指南](#使用指南)
- [基线指标](#基线指标)
- [如何解读结果](#如何解读结果)

---

## 快速开始

### 一键运行所有基准测试

```bash
bash benches/run_all.sh
```

这会：
1. 运行 Criterion 微基准测试
2. 询问是否运行负载测试（需要启动服务器）
3. 生成汇总报告

---

## 基准测试类型

### 1. 微基准测试（Criterion）

测量底层组件性能：

- **数据库查询**：列表查询、单条查询、通过 slug 查询
- **JSON 序列化/反序列化**：不同数据量级（1, 10, 50, 100 条）
- **插入操作**：单条 INSERT 性能

**运行方式：**

```bash
cargo bench --bench api_benchmarks
```

**结果位置：** `target/criterion/report/index.html`

### 2. 负载测试（wrk / bombardier）

测试端到端 API 性能：

- **并发请求处理能力**
- **响应时间分布**（p50, p75, p90, p95, p99）
- **吞吐量**（req/s）

**运行方式：**

```bash
# 使用 wrk（推荐，Linux/macOS）
bash benches/load_test.sh

# 使用 bombardier（跨平台）
bash benches/load_test_bombardier.sh
```

### 3. 内存监控

实时监控服务器内存占用：

**Linux/macOS/Git Bash：**
```bash
# 终端 1：启动服务器
cargo run --release

# 终端 2：监控内存
bash benches/monitor_memory.sh > memory.csv
```

**Windows PowerShell：**
```powershell
# 终端 1：启动服务器
cargo run --release

# 终端 2：监控内存
.\benches\monitor_memory.ps1 memory.csv
```

---

## 工具要求

### 必需

- **Rust 工具链**：cargo, rustc
- **Criterion**（自动安装）：通过 `Cargo.toml` 的 `[dev-dependencies]`

### 可选（用于负载测试）

#### wrk（推荐，性能最优）

- **Linux**: `apt install wrk` / `yum install wrk`
- **macOS**: `brew install wrk`
- **Windows**: 使用 WSL 或下载预编译版本

下载：https://github.com/wg/wrk

#### bombardier（跨平台替代）

- 下载预编译二进制：https://github.com/codesenberg/bombardier/releases
- Windows: 下载 `bombardier-windows-amd64.exe`，重命名为 `bombardier.exe`
- 添加到 PATH 或放在项目根目录

---

## 使用指南

### 场景 1：首次建立基线

```bash
# 1. 运行 Criterion 基准测试
cargo bench --bench api_benchmarks

# 2. 启动服务器（新终端）
cargo run --release

# 3. 运行负载测试
bash benches/load_test.sh

# 4. 保存基线报告
LATEST=$(ls -td benches/reports/20* | head -1)
cp -r "$LATEST" benches/reports/baseline

# 5. 更新 README 中的基线指标（见下方）
```

### 场景 2：重构后性能验证

```bash
# 1. 运行基准测试
bash benches/run_all.sh

# 2. 对比新结果与基线
LATEST=$(ls -td benches/reports/20* | head -1)
bash benches/compare_results.sh "$LATEST" benches/reports/baseline

# 3. 检查是否有回归（p95 增加 > 10%）
```

### 场景 3：性能调优迭代

```bash
# 1. 修改代码
# 2. 快速验证关键指标
cargo bench --bench api_benchmarks -- query_posts_list

# 3. 全面测试
bash benches/run_all.sh
```

---

## 基线指标

> **记录日期：** YYYY-MM-DD  
> **环境：** 描述硬件和软件环境（CPU、内存、OS、Rust 版本）  
> **数据量：** 测试数据库中的记录数

### Criterion 微基准测试

| 基准测试 | 平均时间 | 标准差 |
|---------|---------|--------|
| query_posts_list_20 | XX.XX ms | ±X.XX ms |
| query_post_by_id | XX.XX µs | ±X.XX µs |
| query_post_by_slug | XX.XX µs | ±X.XX µs |
| serialize_posts_json/100 | XX.XX µs | ±X.XX µs |
| insert_post | XX.XX ms | ±X.XX ms |

### 负载测试（wrk/bombardier）

| 测试 | 吞吐量 (req/s) | p50 | p95 | p99 |
|------|---------------|-----|-----|-----|
| GET /api/health | XXXX | X ms | X ms | X ms |
| GET /api/posts | XXXX | X ms | XX ms | XX ms |
| GET /api/posts/:slug | XXXX | X ms | XX ms | XX ms |

### 内存占用

| 状态 | RSS | 备注 |
|------|-----|------|
| 启动后（idle） | ~XX MB | 无负载 |
| 负载测试中 | ~XX MB | 100 并发连接 |
| 峰值 | ~XX MB | 持续 30 秒负载 |

---

## 如何解读结果

### Criterion 报告

**HTML 报告位置：** `target/criterion/report/index.html`

- **绿色**：性能改进（比基线快）
- **红色**：性能回退（比基线慢）
- **Slope（斜率）**：时间复杂度的实际表现

**关注点：**
- 标准差是否稳定（抖动小）
- 是否有明显的性能悬崖（outliers）

### 负载测试指标

#### 吞吐量（Throughput）

- **目标：** > 1000 req/s（单实例）
- **评估：**
  - \> 2000 req/s：优秀
  - 1000-2000 req/s：良好
  - < 1000 req/s：需要优化

#### 响应时间（Latency）

| 百分位 | 目标 | 评估 |
|--------|------|------|
| p50 | < 50ms | 大部分请求的体验 |
| p95 | < 100ms | 可接受的尾延迟 |
| p99 | < 200ms | 极端情况，需监控 |

**颜色编码：**
- 🟢 绿色：满足目标
- 🟡 黄色：接近阈值
- 🔴 红色：需要优化

#### 内存占用

- **目标：** < 50 MB（对标 Strapi/Directus 的 200-500 MB）
- **评估：**
  - < 30 MB：优秀
  - 30-50 MB：良好
  - 50-100 MB：可接受
  - \> 100 MB：需要调查

### 性能回归判定

**回归阈值：**
- p50 增加 > 20%：严重回归
- p95 增加 > 10%：需要关注
- 吞吐量下降 > 15%：严重回归
- 内存增加 > 30%：需要调查

---

## 脚本说明

| 脚本 | 用途 | 平台 |
|------|------|------|
| `run_all.sh` | 一键运行所有基准测试 | Linux/macOS/Git Bash |
| `load_test.sh` | wrk 负载测试 | Linux/macOS/Git Bash |
| `load_test_bombardier.sh` | bombardier 负载测试 | 跨平台 |
| `monitor_memory.sh` | 内存监控（bash） | Linux/macOS/Git Bash |
| `monitor_memory.ps1` | 内存监控（PowerShell） | Windows |
| `analyze_results.sh` | 分析测试结果 | Linux/macOS/Git Bash |
| `compare_results.sh` | 对比两次测试结果 | Linux/macOS/Git Bash |

---

## 配置选项

### 环境变量

```bash
# 服务器地址（默认：http://localhost:3000）
export COLOPHON_URL=http://your-server:port

# 负载测试持续时间（默认：30s）
export DURATION=60s

# 并发连接数（默认：100）
export CONNECTIONS=200

# wrk 线程数（默认：4）
export THREADS=8

# JWT Token（用于 POST 请求测试）
export COLOPHON_TOKEN=your_jwt_token
```

### 自定义测试

**编辑 `benches/load_test.sh` 添加新测试：**

```bash
# 测试自定义端点
echo -e "\n${YELLOW}Test X: GET /api/custom${NC}"
wrk -t4 -c100 -d30s --latency "${BASE_URL}/api/custom" | tee "${REPORT_DIR}/0X_custom.txt"
```

---

## 故障排查

### 问题 1：Criterion 测试失败

**错误：** `could not compile criterion`

**解决：**
```bash
cargo clean
cargo bench --bench api_benchmarks
```

### 问题 2：wrk 不可用（Windows）

**解决方案 A：** 使用 WSL
```bash
# 在 WSL 中安装 wrk
sudo apt install wrk
wsl bash benches/load_test.sh
```

**解决方案 B：** 使用 bombardier
```bash
# 下载 bombardier-windows-amd64.exe
# 重命名为 bombardier.exe，放入 PATH
bash benches/load_test_bombardier.sh
```

### 问题 3：服务器连接失败

**错误：** `Error: Server not responding`

**检查：**
```bash
# 1. 服务器是否运行
curl http://localhost:3000/api/health

# 2. 端口是否正确
export COLOPHON_URL=http://localhost:YOUR_PORT

# 3. 防火墙是否阻止
netstat -an | grep 3000
```

### 问题 4：内存监控无数据

**Linux/macOS：**
```bash
# 检查 bc 是否安装（用于计算）
which bc || sudo apt install bc

# 检查进程名是否正确
ps aux | grep colophon
```

**Windows PowerShell：**
```powershell
# 检查进程是否存在
Get-Process -Name "colophon"
```

---

## 持续集成（CI）

**GitHub Actions 示例：**

```yaml
name: Performance Benchmarks

on:
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run Criterion benchmarks
        run: cargo bench --bench api_benchmarks
      
      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: criterion-report
          path: target/criterion/
```

---

## 相关资源

- **Criterion.rs 文档**: https://bheisler.github.io/criterion.rs/book/
- **wrk 文档**: https://github.com/wg/wrk
- **bombardier 文档**: https://github.com/codesenberg/bombardier
- **性能优化指南**: 参考 `docs/performance.md`（如果存在）

---

## 贡献

添加新的基准测试：

1. 在 `benches/api_benchmarks.rs` 中添加新函数
2. 添加到 `criterion_group!` 宏
3. 更新本 README 的基线指标表格

---

**最后更新：** YYYY-MM-DD
