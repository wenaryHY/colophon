DROP TABLE IF EXISTS posts_fts;
CREATE VIRTUAL TABLE posts_fts USING fts5(
    title, content_md,
    content='posts', content_rowid='rowid',
    tokenize='trigram'
);

-- 旧触发器绑定在 posts 表上，需显式删除
DROP TRIGGER IF EXISTS posts_fts_insert;
DROP TRIGGER IF EXISTS posts_fts_delete;
DROP TRIGGER IF EXISTS posts_fts_update;

-- Rebuild triggers
CREATE TRIGGER posts_fts_insert AFTER INSERT ON posts BEGIN
    INSERT INTO posts_fts(rowid, title, content_md) VALUES (new.rowid, new.title, new.content_md);
END;

CREATE TRIGGER posts_fts_delete AFTER DELETE ON posts BEGIN
    INSERT INTO posts_fts(posts_fts, rowid, title, content_md) VALUES('delete', old.rowid, old.title, old.content_md);
END;

CREATE TRIGGER posts_fts_update AFTER UPDATE ON posts BEGIN
    INSERT INTO posts_fts(posts_fts, rowid, title, content_md) VALUES('delete', old.rowid, old.title, old.content_md);
    INSERT INTO posts_fts(rowid, title, content_md) VALUES (new.rowid, new.title, new.content_md);
END;
