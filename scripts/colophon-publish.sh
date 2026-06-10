#!/bin/bash
# colophon-publish — 从命令行发布 Markdown 文件到 Colophon
# 用法: export COLOPHON_PASSWORD="your-password"; bash colophon-publish.sh article.md [--draft]

set -e

if [ -z "$COLOPHON_PASSWORD" ]; then
    echo "错误: 请先设置环境变量 COLOPHON_PASSWORD"
    exit 1
fi

FILE="$1"
DRY=false
[ "$2" = "--draft" ] && DRY=true

if [ ! -f "$FILE" ]; then
    echo "用法: bash colophon-publish.sh article.md [--draft]"
    exit 1
fi

# 提取第一行 # 标题，其余为正文
TITLE=$(head -1 "$FILE" | sed '"'"'s/^# //'"'"')
CONTENT=$(tail -n +2 "$FILE" | sed '"'"'/^$/d; 1{/^$/d}'"'"')

if [ -z "$TITLE" ]; then
    echo "错误: 文件第一行必须是 # 标题"
    exit 1
fi

SLUG=$(echo "$TITLE" | tr '"'"'[:upper:]'"'"' '"'"'[:lower:]'"'"' | sed '"'"'s/[^a-z0-9]/-/g'"'"' | sed '"'"'s/-\+/-/g'"'"' | sed '"'"'s/^-//;s/-$//'"'"')
STATUS="published"
$DRY && STATUS="draft"

echo "标题: $TITLE"
echo "Slug: $SLUG"
echo "状态: $STATUS"
echo "---"

# 登录
echo "登录..."
RESP=$(curl -s -X POST https://wenary.me/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"login\":\"Wenary\",\"password\":\"$COLOPHON_PASSWORD\"}")

TOKEN=$(echo "$RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)[\"data\"][\"access_token\"])" 2>/dev/null)

if [ -z "$TOKEN" ]; then
    echo "登录失败: $RESP"
    exit 1
fi

# 查找已有文章
echo "查找已有文章..."
POST_ID=$(curl -s "https://wenary.me/api/v1/admin/posts" \
  -H "Authorization: Bearer $TOKEN" | \
  python3 -c "
import sys, json
d = json.load(sys.stdin)
items = d.get('data', {}).get('items', [])
target = next((i for i in items if i.get('slug') == '$SLUG'), None)
print(target['id'] if target else '')
" 2>/dev/null)

API_URL="https://wenary.me/api/v1/admin/posts"
if [ -n "$POST_ID" ]; then
    API_URL="$API_URL/$POST_ID"
    METHOD="PATCH"
    echo "更新已有文章 $POST_ID..."
else
    METHOD="POST"
    echo "创建新文章..."
fi

RESULT=$(curl -s -X $METHOD "$API_URL" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "$(python3 -c "
import json, sys
print(json.dumps({
    'title': '$TITLE',
    'slug': '$SLUG',
    'content_md': '''$CONTENT''',
    'status': '$STATUS',
    'content_type': 'post'
}))
")")

echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); print('发布成功!' if d['code']==0 else '失败: '+d['message'])"