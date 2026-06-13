#!/usr/bin/env bash
set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

if [ $# -eq 0 ]; then
    echo -e "${RED}Usage: $0 <report_directory>${NC}"
    echo -e "${YELLOW}Example: $0 benches/reports/20260613_120000${NC}"
    exit 1
fi

REPORT_DIR="$1"

if [ ! -d "$REPORT_DIR" ]; then
    echo -e "${RED}Error: Directory not found: $REPORT_DIR${NC}"
    exit 1
fi

echo -e "${GREEN}=== Analyzing Benchmark Results ===${NC}\n"
echo -e "${BLUE}Report Directory: $REPORT_DIR${NC}\n"

# 分析函数：从 wrk 输出提取关键指标
analyze_wrk() {
    local file=$1
    local test_name=$2
    
    if [ ! -f "$file" ]; then
        echo -e "${YELLOW}  Skipped (file not found)${NC}"
        return
    fi
    
    echo -e "${YELLOW}$test_name${NC}"
    
    # 提取请求/秒
    local rps=$(grep "Requests/sec:" "$file" | awk '{print $2}')
    
    # 提取延迟
    local latency_avg=$(grep "Latency" "$file" | head -n 1 | awk '{print $2}')
    local latency_stdev=$(grep "Latency" "$file" | head -n 1 | awk '{print $3}')
    local latency_max=$(grep "Latency" "$file" | head -n 1 | awk '{print $4}')
    
    # 提取百分位
    local p50=$(grep "50%" "$file" | awk '{print $2}')
    local p75=$(grep "75%" "$file" | awk '{print $2}')
    local p90=$(grep "90%" "$file" | awk '{print $2}')
    local p99=$(grep "99%" "$file" | awk '{print $2}')
    
    # 提取总请求数和错误
    local total_requests=$(grep "requests in" "$file" | awk '{print $1}')
    local errors=$(grep "Socket errors:" "$file" | wc -l)
    
    echo -e "  Throughput:     ${GREEN}$rps req/s${NC}"
    echo -e "  Latency (avg):  $latency_avg"
    echo -e "  Latency (max):  $latency_max"
    echo -e "  Latency p50:    ${GREEN}$p50${NC}"
    echo -e "  Latency p95:    ${YELLOW}$(grep "95%" "$file" | awk '{print $2}' || echo 'N/A')${NC}"
    echo -e "  Latency p99:    ${RED}$p99${NC}"
    echo -e "  Total requests: $total_requests"
    
    if [ "$errors" -gt 0 ]; then
        echo -e "  ${RED}Errors detected!${NC}"
    fi
    
    echo ""
}

# 分析函数：从 bombardier 输出提取关键指标
analyze_bombardier() {
    local file=$1
    local test_name=$2
    
    if [ ! -f "$file" ]; then
        echo -e "${YELLOW}  Skipped (file not found)${NC}"
        return
    fi
    
    echo -e "${YELLOW}$test_name${NC}"
    
    # 提取关键指标
    local rps=$(grep "Reqs/sec" "$file" | awk '{print $2}')
    local latency_avg=$(grep "Latency" "$file" | grep "Avg" | awk '{print $2}')
    local latency_50=$(grep "50%" "$file" | awk '{print $2}')
    local latency_95=$(grep "95%" "$file" | awk '{print $2}')
    local latency_99=$(grep "99%" "$file" | awk '{print $2}')
    
    echo -e "  Throughput:     ${GREEN}$rps req/s${NC}"
    echo -e "  Latency (avg):  $latency_avg"
    echo -e "  Latency p50:    ${GREEN}$latency_50${NC}"
    echo -e "  Latency p95:    ${YELLOW}$latency_95${NC}"
    echo -e "  Latency p99:    ${RED}$latency_99${NC}"
    echo ""
}

# 自动检测使用的工具
if grep -q "Requests/sec:" "$REPORT_DIR"/*.txt 2>/dev/null; then
    TOOL="wrk"
    ANALYZE_FUNC=analyze_wrk
elif grep -q "Reqs/sec" "$REPORT_DIR"/*.txt 2>/dev/null; then
    TOOL="bombardier"
    ANALYZE_FUNC=analyze_bombardier
else
    echo -e "${RED}Error: Unable to detect benchmark tool${NC}"
    exit 1
fi

echo -e "${BLUE}Detected tool: $TOOL${NC}\n"

# 分析所有测试
for file in "$REPORT_DIR"/*.txt; do
    if [ -f "$file" ]; then
        filename=$(basename "$file" .txt)
        $ANALYZE_FUNC "$file" "$filename"
    fi
done

echo -e "${GREEN}=== Analysis Complete ===${NC}"

# 生成 Markdown 报告
SUMMARY_FILE="$REPORT_DIR/SUMMARY.md"

echo "# Colophon Performance Benchmark Summary" > "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"
echo "**Date:** $(date)" >> "$SUMMARY_FILE"
echo "**Tool:** $TOOL" >> "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"
echo "## Results" >> "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"
echo "| Test | Throughput (req/s) | p50 | p95 | p99 |" >> "$SUMMARY_FILE"
echo "|------|-------------------|-----|-----|-----|" >> "$SUMMARY_FILE"

for file in "$REPORT_DIR"/*.txt; do
    if [ -f "$file" ]; then
        filename=$(basename "$file" .txt)
        
        if [ "$TOOL" = "wrk" ]; then
            rps=$(grep "Requests/sec:" "$file" | awk '{print $2}' || echo "N/A")
            p50=$(grep "50%" "$file" | awk '{print $2}' || echo "N/A")
            p95=$(grep "95%" "$file" | awk '{print $2}' || echo "N/A")
            p99=$(grep "99%" "$file" | awk '{print $2}' || echo "N/A")
        else
            rps=$(grep "Reqs/sec" "$file" | awk '{print $2}' || echo "N/A")
            p50=$(grep "50%" "$file" | awk '{print $2}' || echo "N/A")
            p95=$(grep "95%" "$file" | awk '{print $2}' || echo "N/A")
            p99=$(grep "99%" "$file" | awk '{print $2}' || echo "N/A")
        fi
        
        echo "| $filename | $rps | $p50 | $p95 | $p99 |" >> "$SUMMARY_FILE"
    fi
done

echo "" >> "$SUMMARY_FILE"
echo "## Criterion Benchmarks" >> "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"
echo "See: \`target/criterion/report/index.html\`" >> "$SUMMARY_FILE"

echo -e "${BLUE}Summary saved to: $SUMMARY_FILE${NC}"
