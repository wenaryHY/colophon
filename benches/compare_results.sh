#!/usr/bin/env bash
set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

if [ $# -ne 2 ]; then
    echo -e "${RED}Usage: $0 <new_report_dir> <baseline_report_dir>${NC}"
    echo -e "${YELLOW}Example: $0 benches/reports/20260613_140000 benches/reports/baseline${NC}"
    exit 1
fi

NEW_DIR="$1"
BASELINE_DIR="$2"

if [ ! -d "$NEW_DIR" ]; then
    echo -e "${RED}Error: New report directory not found: $NEW_DIR${NC}"
    exit 1
fi

if [ ! -d "$BASELINE_DIR" ]; then
    echo -e "${RED}Error: Baseline directory not found: $BASELINE_DIR${NC}"
    exit 1
fi

echo -e "${GREEN}=== Performance Comparison ===${NC}\n"
echo -e "${BLUE}New:      $NEW_DIR${NC}"
echo -e "${BLUE}Baseline: $BASELINE_DIR${NC}\n"

# 函数：提取指标
extract_metric() {
    local file=$1
    local metric=$2
    
    if [ ! -f "$file" ]; then
        echo "N/A"
        return
    fi
    
    case $metric in
        "rps")
            grep "Requests/sec:" "$file" | awk '{print $2}' || grep "Reqs/sec" "$file" | awk '{print $2}' || echo "N/A"
            ;;
        "p50")
            grep "50%" "$file" | awk '{print $2}' || echo "N/A"
            ;;
        "p95")
            grep "95%" "$file" | awk '{print $2}' || echo "N/A"
            ;;
        "p99")
            grep "99%" "$file" | awk '{print $2}' || echo "N/A"
            ;;
    esac
}

# 函数：计算百分比变化
calc_change() {
    local new=$1
    local old=$2
    
    # 移除单位（如 ms, s）
    new=$(echo "$new" | sed 's/[^0-9.]//g')
    old=$(echo "$old" | sed 's/[^0-9.]//g')
    
    if [ -z "$new" ] || [ -z "$old" ] || [ "$old" = "0" ]; then
        echo "N/A"
        return
    fi
    
    # 使用 bc 计算百分比变化
    echo "scale=2; (($new - $old) / $old) * 100" | bc
}

# 函数：格式化变化（带颜色）
format_change() {
    local change=$1
    local metric_type=$2  # "latency" 或 "throughput"
    
    if [ "$change" = "N/A" ]; then
        echo "N/A"
        return
    fi
    
    local is_positive=$(echo "$change >= 0" | bc)
    
    if [ "$metric_type" = "latency" ]; then
        # 延迟：降低是好的
        if [ "$is_positive" -eq 1 ]; then
            echo -e "${RED}+${change}%${NC} (worse)"
        else
            echo -e "${GREEN}${change}%${NC} (better)"
        fi
    else
        # 吞吐量：增加是好的
        if [ "$is_positive" -eq 1 ]; then
            echo -e "${GREEN}+${change}%${NC} (better)"
        else
            echo -e "${RED}${change}%${NC} (worse)"
        fi
    fi
}

# 对比所有测试
echo -e "${YELLOW}Test Comparisons:${NC}\n"

for new_file in "$NEW_DIR"/*.txt; do
    if [ -f "$new_file" ]; then
        test_name=$(basename "$new_file")
        baseline_file="$BASELINE_DIR/$test_name"
        
        echo -e "${BLUE}$test_name${NC}"
        
        # 提取指标
        new_rps=$(extract_metric "$new_file" "rps")
        old_rps=$(extract_metric "$baseline_file" "rps")
        rps_change=$(calc_change "$new_rps" "$old_rps")
        
        new_p50=$(extract_metric "$new_file" "p50")
        old_p50=$(extract_metric "$baseline_file" "p50")
        p50_change=$(calc_change "$new_p50" "$old_p50")
        
        new_p95=$(extract_metric "$new_file" "p95")
        old_p95=$(extract_metric "$baseline_file" "p95")
        p95_change=$(calc_change "$new_p95" "$old_p95")
        
        new_p99=$(extract_metric "$new_file" "p99")
        old_p99=$(extract_metric "$baseline_file" "p99")
        p99_change=$(calc_change "$new_p99" "$old_p99")
        
        # 输出对比
        echo -e "  Throughput: $old_rps → $new_rps ($(format_change "$rps_change" "throughput"))"
        echo -e "  Latency p50: $old_p50 → $new_p50 ($(format_change "$p50_change" "latency"))"
        echo -e "  Latency p95: $old_p95 → $new_p95 ($(format_change "$p95_change" "latency"))"
        echo -e "  Latency p99: $old_p99 → $new_p99 ($(format_change "$p99_change" "latency"))"
        echo ""
    fi
done

echo -e "${GREEN}=== Comparison Complete ===${NC}"
