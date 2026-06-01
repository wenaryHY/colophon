CREATE TABLE IF NOT EXISTS media_thumbnails (
    id          TEXT PRIMARY KEY NOT NULL,
    media_id    TEXT NOT NULL,
    size_label  TEXT NOT NULL,
    width       INTEGER NOT NULL,
    height      INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    public_url  TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (media_id) REFERENCES media(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_media_thumbnails_media_id ON media_thumbnails(media_id);
