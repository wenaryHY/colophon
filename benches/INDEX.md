# Colophon 性能基准测试框架

## 📁 目录结构

```
benches/
├── api_benchmarks.rs           # Criterion 微基准测试
├── load_test.sh                # wrk 负载测试脚本
├── load_test_bombardier.sh     # Bombardier 负载测试（跨平台）
├── post_script.lua             # wrk POST 请求脚本
├── monitor_memory.sh           # Bash 内存监控
├── monitor_memory.ps1          # PowerShell 内存监控
├── analyze_results.sh          # 结果分析工具
├── compare_results.sh          # 性能对比工具
├── run_all.sh                  # 一键运行（Bash）
├── run_benchmarks.ps1          # 一键运行（PowerShell）
├── README.md                   # 完整文档
├── QUICKSTART.md               # 快速开始
├── BASELINE_TEMPLATE.md        # 基线数据模板
├── IMPLEMENTATION.md           # 实施总结
└── reports/                    # 测试报告目录
```

## 🚀 快速开始

### Windows 用户

```powershell
# PowerShell（推荐）
.\benches\run_benchmarks.ps1

# 或 Git Bash
bash benches/run_all.sh
```

### Linux/macOS 用户

```bash
bash benches/run_all.sh
```

### 单独运行 Criterion 测试

```bash
cargo bench --bench api_benchmarks
```

## 📊 测试类型

1. **微基准测试（Criterion）** - 数据库查询、JSON 序列化
2. **负载测试（wrk/bombardier）** - 端到端 API 性能
3. **内存监控** - 实时内存占用跟踪

## 📖 文档导航

- **新手？** 阅读 `QUICKSTART.md`
- **详细文档？** 阅读 `README.md`
- **记录基线？** 参考 `BASELINE_TEMPLATE.md`
- **实施细节？** 阅读 `IMPLEMENTATION.md`

## ✅ 验收标准

- [x] Criterion 微基准测试可运行
- [x] wrk/bombardier 负载测试脚本可用
- [x] 内存监控支持 Windows/Linux/macOS
- [x] 结果分析和对比工具
- [x] 完整文档和快速指南

## 🎯 性能目标

| 指标 | 目标 |
|------|------|
| API p95 延迟 | < 100ms |
| 吞吐量 | > 1000 req/s |
| 内存（idle） | < 30 MB |
| 内存（load） | < 50 MB |

## 🔧 下一步

1. 运行基准测试：`cargo bench --bench api_benchmarks`
2. 查看结果：`target/criterion/report/index.html`
3. 建立基线：保存报告到 `benches/reports/baseline/`
4. 更新文档：填写 `benches/README.md` 的基线指标

---

**框架状态：** ✅ 就绪  
**最后更新：** 2026-06-13
