# Colophon FTS5 全文搜索功能

## 概述

Colophon 已完整实现 SQLite FTS5 全文搜索功能，支持中英文混合搜索。

## 技术实现

### 数据库层

- **FTS5 虚拟表**：`posts_fts`（使用 `trigram` tokenizer）
- **触发器自动同步**：insert/update/delete 时自动更新 FTS 索引
- **降级策略**：FTS5 无结果时自动降级到 LIKE 查询

**Migration**: `migrations/018_trigram_fts5.sql`

```sql
CREATE VIRTUAL TABLE posts_fts USING fts5(
    title, content_md,
    content='posts', content_rowid='rowid',
    tokenize='trigram'
);
```

### 仓库层

**文件**: `src/modules/post/repository.rs`

```rust
pub async fn search_posts<'e, E>(
    executor: E,
    keyword: &str,
    category_id: Option<&str>,
    tag_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<PublicPostSummary>, sqlx::Error>
```

- 先尝试 FTS5 trigram 搜索
- FTS5 无结果则降级到 LIKE 查询
- 支持按分类 ID、标签 ID 过滤
- 支持分页

### 服务层

**文件**: `src/modules/post/service.rs`

```rust
pub async fn search_posts(
    state: Arc<AppState>,
    query: SearchQuery,
) -> AppResult<PaginatedResponse<PublicPostSummary>>
```

### 路由层

**API 端点**: `GET /api/v1/search`

**Query 参数**:
- `keyword` (必填): 搜索关键词
- `category_id` (可选): 按分类过滤
- `tag_id` (可选): 按标签过滤
- `page` (可选, 默认 1): 页码
- `page_size` (可选, 默认 10, 最大 100): 每页数量

## 使用示例

### 基本搜索

```bash
# 搜索包含 "rust" 的文章
curl "http://localhost:2000/api/v1/search?keyword=rust"
```

### 中文搜索

```bash
# 搜索包含 "性能" 的文章
curl "http://localhost:2000/api/v1/search?keyword=性能"
```

### 带过滤条件的搜索

```bash
# 搜索特定分类下的文章
curl "http://localhost:2000/api/v1/search?keyword=rust&category_id=tech"

# 搜索带特定标签的文章
curl "http://localhost:2000/api/v1/search?keyword=rust&tag_id=programming"
```

### 分页

```bash
# 第 2 页，每页 20 条
curl "http://localhost:2000/api/v1/search?keyword=rust&page=2&page_size=20"
```

## 响应格式

```json
{
  "code": 0,
  "message": "ok",
  "data": {
    "items": [
      {
        "id": "uuid",
        "title": "Rust 性能优化",
        "slug": "rust-performance",
        "excerpt": "深入理解 Rust 性能优化技巧",
        "content_type": "post",
        "published_at": "2024-01-01T12:00:00Z",
        "created_at": "2024-01-01T10:00:00Z",
        "updated_at": "2024-01-01T12:00:00Z",
        "author_display_name": "作者名",
        "category_name": "技术",
        "category_id": "tech"
      }
    ],
    "pagination": {
      "page": 1,
      "page_size": 10,
      "total": 42
    }
  },
  "request_id": "..."
}
```

## 搜索特性

### 1. 仅搜索已发布内容

- 只搜索 `status=published` 且 `visibility=public` 的文章
- 草稿和私有文章不会出现在搜索结果中

### 2. 排除软删除内容

- 软删除（`deleted_at IS NOT NULL`）的文章不会出现在搜索结果中

### 3. Trigram Tokenizer

- 支持中文、日文、韩文等 CJK 字符
- 支持部分匹配（如搜索 "SQL" 可以匹配 "SQLite"）
- 对短词和长词都有良好支持

### 4. 降级策略

当 FTS5 无结果时，自动降级到 LIKE 查询：

```sql
WHERE p.title LIKE '%keyword%' OR p.content_md LIKE '%keyword%'
```

这确保即使在边缘情况下也能返回相关结果。

### 5. BM25 相关性排序

FTS5 使用 BM25 算法排序搜索结果，按相关性从高到低返回。

## 测试覆盖

**测试文件**: `src/modules/post/search_tests.rs`

包含 11 个测试用例：

1. ✅ 英文关键词搜索
2. ✅ 中文关键词搜索
3. ✅ 中英文混合搜索
4. ✅ 内容匹配（搜索正文）
5. ✅ 无结果处理
6. ✅ 分页功能
7. ✅ 结果计数
8. ✅ 仅搜索已发布文章
9. ✅ 排除软删除文章
10. ✅ Trigram 部分匹配
11. ✅ 更新文章后 FTS 索引同步

运行测试：

```bash
export DATABASE_URL="sqlite:colophon.db"
cargo test modules::post::search_tests
```

## 性能特点

### 优势

- **快速**：FTS5 为全文搜索优化，比 LIKE 查询快得多
- **中文友好**：trigram tokenizer 对 CJK 字符有良好支持
- **自动同步**：触发器确保 FTS 索引始终与数据同步
- **零运行时开销**：索引维护由 SQLite 触发器处理

### 注意事项

- FTS5 虚拟表会占用额外存储空间（约为原表的 50-100%）
- 更新文章时会触发 FTS 索引更新（略微增加写入延迟）

## 未来优化方向

1. **高亮显示**：返回搜索结果时高亮匹配的关键词
2. **同义词**：支持中文同义词搜索
3. **拼音搜索**：支持拼音搜索中文内容
4. **搜索建议**：根据历史搜索提供自动补全
5. **搜索分析**：记录搜索词统计，分析用户需求

## 相关文件

- **Migration**: `migrations/018_trigram_fts5.sql`
- **Repository**: `src/modules/post/repository.rs`
- **Service**: `src/modules/post/service.rs`
- **Handler**: `src/modules/post/handler.rs`
- **Router**: `src/bootstrap/router.rs` (line 290)
- **Tests**: `src/modules/post/search_tests.rs`
- **DTO**: `src/modules/post/dto.rs` (`SearchQuery` struct)

## 维护指南

### 重建 FTS 索引

如果 FTS 索引损坏或不同步，可以手动重建：

```sql
DELETE FROM posts_fts;
INSERT INTO posts_fts(rowid, title, content_md)
SELECT rowid, title, content_md FROM posts;
```

### 检查 FTS 索引大小

```sql
SELECT 
    (SELECT COUNT(*) FROM posts) as posts_count,
    (SELECT COUNT(*) FROM posts_fts) as fts_count;
```

### 验证触发器

```sql
SELECT name, sql FROM sqlite_master 
WHERE type='trigger' AND tbl_name='posts';
```

应该看到 3 个触发器：
- `posts_fts_insert`
- `posts_fts_update`
- `posts_fts_delete`
