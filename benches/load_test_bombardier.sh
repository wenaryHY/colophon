#!/usr/bin/env bash
set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}=== Colophon Performance Benchmark (Bombardier) ===${NC}\n"

# 检查 bombardier 是否安装
if ! command -v bombardier &> /dev/null; then
    echo -e "${RED}Error: bombardier not found${NC}"
    echo -e "${YELLOW}Install:${NC}"
    echo -e "  • Download: https://github.com/codesenberg/bombardier/releases"
    echo -e "  • Windows: Download bombardier-windows-amd64.exe, rename to bombardier.exe"
    echo -e "  • Add to PATH or place in current directory${NC}"
    exit 1
fi

# 默认配置
BASE_URL="${COLOPHON_URL:-http://localhost:3000}"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-100}"

# 检查服务器是否运行
echo -e "${YELLOW}Checking server at ${BASE_URL}...${NC}"
if ! curl -s --max-time 5 "${BASE_URL}/api/health" > /dev/null 2>&1; then
    echo -e "${RED}Error: Server not responding${NC}"
    echo -e "${YELLOW}Start server: cargo run --release${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Server is running${NC}\n"

# 创建报告目录
REPORT_DIR="benches/reports/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$REPORT_DIR"

echo -e "${BLUE}Report: ${REPORT_DIR}${NC}\n"

# 测试 1: Health Check
echo -e "${YELLOW}Test 1: GET /api/health${NC}"
bombardier -c 10 -d 10s -l "${BASE_URL}/api/health" | tee "${REPORT_DIR}/01_health.txt"

# 测试 2: 文章列表
echo -e "\n${YELLOW}Test 2: GET /api/posts${NC}"
bombardier -c ${CONNECTIONS} -d ${DURATION} -l "${BASE_URL}/api/posts?page=1&per_page=20" | tee "${REPORT_DIR}/02_posts_list.txt"

# 测试 3: 单个文章
echo -e "\n${YELLOW}Test 3: GET /api/posts/:slug${NC}"
bombardier -c ${CONNECTIONS} -d ${DURATION} -l "${BASE_URL}/api/posts/hello-world" | tee "${REPORT_DIR}/03_post_detail.txt" || echo "Skipped"

# 测试 4: 分页
echo -e "\n${YELLOW}Test 4: Pagination stress${NC}"
bombardier -c ${CONNECTIONS} -d ${DURATION} -l "${BASE_URL}/api/posts?page=10&per_page=20" | tee "${REPORT_DIR}/04_pagination.txt"

echo -e "\n${GREEN}=== Complete ===${NC}"
echo -e "${BLUE}Results: ${REPORT_DIR}${NC}"
