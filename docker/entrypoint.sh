#!/bin/sh
set -eu

# 如果启用 Litestream 且 S3 配置齐全，用 envsubst 替换配置文件中的环境变量后启动
if [ "${LITESTREAM_ENABLED:-1}" = "1" ]; then
  if [ -n "${LITESTREAM_S3_BUCKET:-}" ] && [ -n "${AWS_REGION:-}" ] && \
     [ -n "${AWS_ACCESS_KEY_ID:-}" ] && [ -n "${AWS_SECRET_ACCESS_KEY:-}" ]; then
    echo "Litestream enabled: replicating to S3 bucket ${LITESTREAM_S3_BUCKET}"
    # 用 envsubst 替换 litestream.yml 中的 ${VAR} 占位符
    envsubst < /app/config/litestream.yml > /tmp/litestream.yml
    exec litestream replicate -config /tmp/litestream.yml -exec /app/colophon
  fi
  echo "Litestream disabled: missing S3 environment variables. Starting Colophon directly."
fi

exec /app/colophon
