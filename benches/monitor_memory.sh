#!/usr/bin/env bash
set -euo pipefail

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}=== Colophon Memory Monitor ===${NC}\n"

# 查找 Colophon 进程
PID=$(pgrep -f "colophon" | head -n 1)

if [ -z "$PID" ]; then
    echo -e "${RED}Error: Colophon process not found${NC}"
    echo -e "${YELLOW}Start server first: cargo run --release${NC}"
    exit 1
fi

echo -e "${GREEN}Monitoring PID: $PID${NC}"
echo -e "${YELLOW}Press Ctrl+C to stop${NC}\n"

# 输出 CSV 头
echo "Timestamp,Elapsed(s),RSS(MB),VSZ(MB),CPU(%)"

START_TIME=$(date +%s)

# 捕获 Ctrl+C 生成报告
trap 'echo -e "\n${BLUE}Generating summary...${NC}"; exit 0' INT

while true; do
    # 检查进程是否还在运行
    if ! kill -0 $PID 2>/dev/null; then
        echo -e "\n${RED}Process $PID terminated${NC}"
        exit 1
    fi
    
    # 获取内存和 CPU 使用情况
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux
        STATS=$(ps -p $PID -o rss=,vsz=,pcpu= 2>/dev/null || echo "0 0 0.0")
        RSS_KB=$(echo $STATS | awk '{print $1}')
        VSZ_KB=$(echo $STATS | awk '{print $2}')
        CPU=$(echo $STATS | awk '{print $3}')
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        STATS=$(ps -p $PID -o rss=,vsz=,pcpu= 2>/dev/null || echo "0 0 0.0")
        RSS_KB=$(echo $STATS | awk '{print $1}')
        VSZ_KB=$(echo $STATS | awk '{print $2}')
        CPU=$(echo $STATS | awk '{print $3}')
    else
        # Windows (Git Bash / WSL)
        STATS=$(ps -p $PID -o rss=,vsz= 2>/dev/null || echo "0 0")
        RSS_KB=$(echo $STATS | awk '{print $1}')
        VSZ_KB=$(echo $STATS | awk '{print $2}')
        CPU="0.0"
    fi
    
    # 转换为 MB
    RSS_MB=$(echo "scale=2; $RSS_KB / 1024" | bc)
    VSZ_MB=$(echo "scale=2; $VSZ_KB / 1024" | bc)
    
    # 计算运行时间
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - START_TIME))
    
    # 输出数据
    echo "$CURRENT_TIME,$ELAPSED,$RSS_MB,$VSZ_MB,$CPU"
    
    sleep 1
done
