#!/usr/bin/env bash
set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Colophon Performance Benchmark ===${NC}\n"

# 检查 wrk 是否安装
if ! command -v wrk &> /dev/null; then
    echo -e "${RED}Error: wrk not found${NC}"
    echo -e "${YELLOW}Install options:${NC}"
    echo -e "  • Linux: apt install wrk / yum install wrk"
    echo -e "  • macOS: brew install wrk"
    echo -e "  • Windows: Use WSL or download from https://github.com/wg/wrk${NC}"
    echo ""
    echo -e "${BLUE}Alternative: Install bombardier (cross-platform)${NC}"
    echo -e "  • Download: https://github.com/codesenberg/bombardier/releases${NC}"
    exit 1
fi

# 默认配置
BASE_URL="${COLOPHON_URL:-http://localhost:3000}"
DURATION="${DURATION:-30s}"
THREADS="${THREADS:-4}"
CONNECTIONS="${CONNECTIONS:-100}"

# 检查服务器是否运行
echo -e "${YELLOW}Checking if Colophon server is running at ${BASE_URL}...${NC}"
if ! curl -s --max-time 5 "${BASE_URL}/api/health" > /dev/null 2>&1; then
    echo -e "${RED}Error: Server not responding at ${BASE_URL}${NC}"
    echo -e "${YELLOW}Please start the server first:${NC}"
    echo -e "  cargo run --release"
    echo ""
    echo -e "${YELLOW}Or set custom URL:${NC}"
    echo -e "  export COLOPHON_URL=http://your-server:port"
    exit 1
fi

echo -e "${GREEN}✓ Server is running${NC}\n"

# 创建报告目录
REPORT_DIR="benches/reports/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$REPORT_DIR"

echo -e "${BLUE}Report will be saved to: ${REPORT_DIR}${NC}\n"

# 测试 1: Health Check (baseline)
echo -e "${YELLOW}Test 1: GET /api/health (warmup & baseline)${NC}"
wrk -t2 -c10 -d10s --latency "${BASE_URL}/api/health" | tee "${REPORT_DIR}/01_health.txt"

# 测试 2: 文章列表查询（公开 API）
echo -e "\n${YELLOW}Test 2: GET /api/posts (public list)${NC}"
wrk -t${THREADS} -c${CONNECTIONS} -d${DURATION} --latency "${BASE_URL}/api/posts?page=1&per_page=20" | tee "${REPORT_DIR}/02_posts_list.txt"

# 测试 3: 单个文章查询
echo -e "\n${YELLOW}Test 3: GET /api/posts/:slug (single resource)${NC}"
echo -e "${BLUE}Note: Replace 'hello-world' with an actual slug from your database${NC}"
wrk -t${THREADS} -c${CONNECTIONS} -d${DURATION} --latency "${BASE_URL}/api/posts/hello-world" | tee "${REPORT_DIR}/03_post_detail.txt" || echo "Skipped (post not found)"

# 测试 4: 静态资源（如果有）
echo -e "\n${YELLOW}Test 4: GET /static/* (static files)${NC}"
wrk -t${THREADS} -c${CONNECTIONS} -d15s --latency "${BASE_URL}/static/favicon.ico" | tee "${REPORT_DIR}/04_static.txt" || echo "Skipped (no static files)"

# 测试 5: 分页压力测试（深度分页）
echo -e "\n${YELLOW}Test 5: GET /api/posts?page=10 (pagination stress)${NC}"
wrk -t${THREADS} -c${CONNECTIONS} -d${DURATION} --latency "${BASE_URL}/api/posts?page=10&per_page=20" | tee "${REPORT_DIR}/05_pagination.txt"

echo -e "\n${GREEN}=== Benchmark Complete ===${NC}"
echo -e "${BLUE}Results saved to: ${REPORT_DIR}${NC}"
echo ""
echo -e "${YELLOW}Summary:${NC}"
echo -e "  View detailed reports in ${REPORT_DIR}/"
echo -e "  Criterion HTML report: target/criterion/report/index.html"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo -e "  1. Run: bash benches/analyze_results.sh ${REPORT_DIR}"
echo -e "  2. Compare with baseline: bash benches/compare_results.sh ${REPORT_DIR} baseline.txt"
