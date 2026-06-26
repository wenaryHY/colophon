#!/bin/bash
# 清理测试产生的临时数据库文件
find . -maxdepth 1 -name "test_backup_*.db" -delete
find . -maxdepth 1 -name "test_backup_*.db.bak" -delete
find . -maxdepth 1 -name "test_backups_*" -type d -exec rm -rf {} + 2>/dev/null
echo "cleaned test backup databases"
