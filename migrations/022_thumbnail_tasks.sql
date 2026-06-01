CREATE TABLE IF NOT EXISTS thumbnail_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    media_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | processing | done | failed
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 1,
    last_error TEXT,
    width INTEGER,      -- 原图宽度（处理成功后记录）
    height INTEGER,     -- 原图高度
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (media_id) REFERENCES media(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_thumbnail_tasks_status ON thumbnail_tasks(status, created_at);
CREATE INDEX IF NOT EXISTS idx_thumbnail_tasks_media_id ON thumbnail_tasks(media_id);
