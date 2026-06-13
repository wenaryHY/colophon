#!/usr/bin/env bash
set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}╔═══════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║   Colophon Performance Benchmark Suite          ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════╝${NC}\n"

# 创建报告目录
REPORT_DIR="benches/reports/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$REPORT_DIR"

echo -e "${BLUE}Report Directory: $REPORT_DIR${NC}\n"

# ========================================
# 第 1 步：Criterion 微基准测试
# ========================================
echo -e "${YELLOW}═══ Step 1/3: Running Criterion Micro-Benchmarks ═══${NC}\n"

if cargo bench --bench api_benchmarks; then
    echo -e "\n${GREEN}✓ Criterion benchmarks completed${NC}"
    echo -e "${BLUE}  HTML Report: target/criterion/report/index.html${NC}\n"
else
    echo -e "\n${RED}✗ Criterion benchmarks failed${NC}\n"
    exit 1
fi

# ========================================
# 第 2 步：询问是否运行负载测试
# ========================================
echo -e "${YELLOW}═══ Step 2/3: Load Testing (Optional) ═══${NC}\n"

echo -e "${BLUE}Do you want to run load tests?${NC}"
echo -e "  This requires a running Colophon server"
echo -e "  ${YELLOW}(Start with: cargo run --release)${NC}\n"

read -p "Run load tests? [y/N]: " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    # 检测可用工具
    if command -v wrk &> /dev/null; then
        echo -e "${GREEN}Using wrk for load testing${NC}\n"
        bash benches/load_test.sh
    elif command -v bombardier &> /dev/null; then
        echo -e "${GREEN}Using bombardier for load testing${NC}\n"
        bash benches/load_test_bombardier.sh
    else
        echo -e "${RED}No load testing tool found${NC}"
        echo -e "${YELLOW}Install wrk or bombardier to run load tests${NC}"
        echo -e "  wrk: https://github.com/wg/wrk"
        echo -e "  bombardier: https://github.com/codesenberg/bombardier${NC}\n"
    fi
else
    echo -e "${YELLOW}Skipping load tests${NC}\n"
fi

# ========================================
# 第 3 步：生成汇总报告
# ========================================
echo -e "${YELLOW}═══ Step 3/3: Generating Summary Report ═══${NC}\n"

# 检查是否有负载测试结果
LATEST_REPORT=$(find benches/reports -maxdepth 1 -type d -name "20*" | sort -r | head -n 1)

if [ -n "$LATEST_REPORT" ] && [ "$LATEST_REPORT" != "benches/reports" ]; then
    echo -e "${BLUE}Analyzing results from: $LATEST_REPORT${NC}\n"
    bash benches/analyze_results.sh "$LATEST_REPORT" || true
else
    echo -e "${YELLOW}No load test results found. Run load tests manually:${NC}"
    echo -e "  bash benches/load_test.sh"
    echo -e "  bash benches/analyze_results.sh <report_dir>${NC}\n"
fi

# ========================================
# 完成
# ========================================
echo -e "${GREEN}╔═══════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║            Benchmark Suite Complete              ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════╝${NC}\n"

echo -e "${BLUE}Next Steps:${NC}"
echo -e "  1. View Criterion report:"
echo -e "     ${YELLOW}open target/criterion/report/index.html${NC}"
echo -e ""
echo -e "  2. Run memory monitoring (in separate terminal):"
echo -e "     ${YELLOW}bash benches/monitor_memory.sh > memory.csv${NC}"
echo -e "     ${YELLOW}# Or on Windows: .\\benches\\monitor_memory.ps1${NC}"
echo -e ""
echo -e "  3. Compare with baseline:"
echo -e "     ${YELLOW}bash benches/compare_results.sh <new_report> <baseline_report>${NC}"
echo -e ""
echo -e "  4. Document baseline metrics:"
echo -e "     ${YELLOW}Edit benches/README.md with your baseline numbers${NC}"
