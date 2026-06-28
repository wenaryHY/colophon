# 性能基准测试框架 - 实施总结

## 已创建文件

### 核心基准测试
- ✅ `benches/api_benchmarks.rs` - Criterion 微基准测试
- ✅ `Cargo.toml` - 添加 Criterion 依赖

### 负载测试脚本
- ✅ `benches/load_test.sh` - wrk 负载测试
- ✅ `benches/load_test_bombardier.sh` - Bombardier 负载测试（跨平台）
- ✅ `benches/post_script.lua` - wrk POST 请求脚本

### 监控脚本
- ✅ `benches/monitor_memory.sh` - Bash 内存监控
- ✅ `benches/monitor_memory.ps1` - PowerShell 内存监控

### 分析工具
- ✅ `benches/analyze_results.sh` - 结果分析脚本
- ✅ `benches/compare_results.sh` - 性能对比脚本
- ✅ `benches/run_all.sh` - 一键运行所有测试
- ✅ `benches/run_benchmarks.ps1` - PowerShell 运行脚本

### 文档
- ✅ `benches/README.md` - 完整使用文档
- ✅ `benches/QUICKSTART.md` - 快速入门指南
- ✅ `benches/BASELINE_TEMPLATE.md` - 基线数据模板
- ✅ `benches/.gitignore` - Git 忽略配置

### 辅助文件
- ✅ `benches/reports/.gitkeep` - 报告目录占位
- ✅ `bench.sh` - 项目根目录快速启动脚本

## 验收标准完成情况

### ✅ Criterion 微基准测试
- [x] 数据库查询性能测试（列表、ID、slug）
- [x] JSON 序列化/反序列化性能测试
- [x] 插入操作性能测试
- [x] 支持 `cargo bench` 运行
- [x] 生成 HTML 报告

### ✅ 负载测试
- [x] wrk 脚本（Linux/macOS/Git Bash）
- [x] Bombardier 脚本（跨平台替代）
- [x] 支持自定义配置（环境变量）
- [x] 测试多个端点（health, posts, pagination）
- [x] POST 请求支持（通过 Lua 脚本）

### ✅ 内存监控
- [x] Bash 脚本（Linux/macOS/Git Bash）
- [x] PowerShell 脚本（Windows 原生）
- [x] 输出 CSV 格式
- [x] 实时显示内存占用

### ✅ 结果分析
- [x] 自动分析 wrk/bombardier 输出
- [x] 提取关键指标（p50/p95/p99, req/s）
- [x] 生成 Markdown 汇总报告
- [x] 支持性能对比（新 vs 基线）

### ✅ 文档
- [x] 完整 README（工具要求、使用指南、指标解读）
- [x] 快速入门指南（分平台说明）
- [x] 基线数据模板（可直接填写）
- [x] 脚本说明表格

## 使用方式

### 方式 1：一键运行（推荐）

**Linux/macOS/Git Bash:**
```bash
bash benches/run_all.sh
```

**Windows PowerShell:**
```powershell
.\benches\run_benchmarks.ps1
```

### 方式 2：分步运行

```bash
# 1. 微基准测试
cargo bench --bench api_benchmarks

# 2. 负载测试（需要启动服务器）
cargo run --release                    # 终端 1
bash benches/load_test.sh              # 终端 2

# 3. 内存监控
bash benches/monitor_memory.sh > mem.csv  # 终端 3

# 4. 分析结果
bash benches/analyze_results.sh benches/reports/20260613_120000
```

## 下一步操作

### 1. 运行首次基准测试

```bash
# 编译基准测试（首次需要几分钟）
cargo bench --bench api_benchmarks

# 查看 HTML 报告
open target/criterion/report/index.html
```

### 2. 建立基线

```bash
# 启动服务器
cargo run --release

# 运行负载测试
bash benches/load_test.sh  # 或 load_test_bombardier.sh

# 保存为基线
LATEST=$(ls -td benches/reports/20* | head -1)
cp -r "$LATEST" benches/reports/baseline
```

### 3. 记录基线指标

编辑 `benches/README.md` 中的"基线指标"部分，填入：
- Criterion 测试结果（从 HTML 报告获取）
- 负载测试结果（从 reports/*.txt 获取）
- 内存占用（从 memory.csv 获取）

参考模板：`benches/BASELINE_TEMPLATE.md`

### 4. 性能回归检测

每次重构后：

```bash
# 运行测试
bash benches/run_all.sh

# 对比基线
LATEST=$(ls -td benches/reports/20* | head -1)
bash benches/compare_results.sh "$LATEST" benches/reports/baseline
```

## 工具安装（可选）

### wrk（推荐，Linux/macOS）

```bash
# Ubuntu/Debian
sudo apt install wrk

# macOS
brew install wrk

# Windows: 使用 WSL
wsl sudo apt install wrk
```

### Bombardier（跨平台替代）

下载：https://github.com/codesenberg/bombardier/releases

```bash
# Linux
wget https://github.com/codesenberg/bombardier/releases/latest/download/bombardier-linux-amd64
chmod +x bombardier-linux-amd64
sudo mv bombardier-linux-amd64 /usr/local/bin/bombardier

# Windows: 下载 bombardier-windows-amd64.exe，重命名为 bombardier.exe
```

## 性能目标

| 指标 | 目标值 | 对标 |
|------|--------|------|
| API p95 延迟 | < 100ms | Strapi/Directus |
| 吞吐量 | > 1000 req/s | 单实例 |
| 内存占用（idle） | < 30 MB | 对标 ~200 MB |
| 内存占用（load） | < 50 MB | 10k 数据量 |
| 启动时间 | < 1s | - |

## 故障排查

### Criterion 编译失败

```bash
cargo clean
cargo bench --bench api_benchmarks
```

### wrk 在 Windows 不可用

使用以下任一方法：
1. WSL: `wsl bash benches/load_test.sh`
2. Bombardier: `bash benches/load_test_bombardier.sh`
3. PowerShell 脚本: `.\benches\run_benchmarks.ps1`

### 服务器连接失败

```bash
# 检查服务器
curl http://localhost:3000/api/health

# 设置自定义 URL
export COLOPHON_URL=http://localhost:YOUR_PORT

# 检查端口
netstat -an | grep 3000
```

## 技术细节

### Criterion 测试内容

1. **query_posts_list_20** - 查询 20 条文章列表
2. **query_post_by_id** - 通过 ID 查询单条
3. **query_post_by_slug** - 通过 slug 查询（有索引）
4. **insert_post** - 插入单条记录
5. **json_serialization** - JSON 序列化（1/10/50/100 条）

### 负载测试配置

- **线程数**: 4（可通过 `THREADS` 环境变量修改）
- **并发连接**: 100（可通过 `CONNECTIONS` 修改）
- **持续时间**: 30s（可通过 `DURATION` 修改）

### 内存监控指标

- **RSS**: 实际物理内存占用
- **VSZ**: 虚拟内存大小
- **WorkingSet** (Windows): 工作集大小
- **PrivateMemory** (Windows): 私有内存

## 集成到 CI/CD

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
      - uses: actions-rs/toolchain@v1
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

## 维护清单

- [ ] 每次主要重构后运行基准测试
- [ ] 更新 README 中的基线指标（每季度或大版本）
- [ ] 对比新旧基线，记录性能趋势
- [ ] 在 PR 中添加性能影响说明（如有）

---

**框架版本:** v1.0.0  
**创建日期:** 2026-06-13  
**适用项目:** Colophon
