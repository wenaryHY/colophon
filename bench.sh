#!/usr/bin/env bash
# Quick benchmark launcher - place in project root
# Usage: ./bench.sh

cd "$(dirname "$0")"

if [ ! -d "benches" ]; then
    echo "Error: benches directory not found"
    exit 1
fi

bash benches/run_all.sh
