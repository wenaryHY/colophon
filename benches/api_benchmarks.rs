use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

// 基准测试 1: 数据库查询性能 - 列表查询
fn bench_query_posts_list(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(async {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        
        // 创建测试表结构
        sqlx::query(
            r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                content TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                visibility TEXT NOT NULL DEFAULT 'public',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                published_at INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        
        // 插入测试数据
        for i in 0..100 {
            sqlx::query(
                "INSERT INTO posts (uuid, title, slug, content, status, visibility, created_at, updated_at) 
                 VALUES (?, ?, ?, ?, 'published', 'public', ?, ?)"
            )
            .bind(format!("uuid-{}", i))
            .bind(format!("Post Title {}", i))
            .bind(format!("post-title-{}", i))
            .bind(format!("Content for post {}. This is some sample content that simulates a real blog post with sufficient length.", i))
            .bind(1700000000i64 + i)
            .bind(1700000000i64 + i)
            .execute(&pool)
            .await
            .unwrap();
        }
        
        pool
    });

    c.bench_function("query_posts_list_20", |b| {
        b.to_async(&rt).iter(|| {
            let pool = pool.clone();
            async move {
                let result = sqlx::query("SELECT * FROM posts WHERE status = 'published' ORDER BY published_at DESC LIMIT 20")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
                black_box(result)
            }
        });
    });
}

// 基准测试 2: 单个资源查询（通过 ID）
fn bench_query_post_by_id(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(async {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        
        sqlx::query(
            r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                content TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                visibility TEXT NOT NULL DEFAULT 'public',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                published_at INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        
        // 插入测试数据
        for i in 1..=50 {
            sqlx::query(
                "INSERT INTO posts (uuid, title, slug, content, status, visibility, created_at, updated_at) 
                 VALUES (?, ?, ?, ?, 'published', 'public', ?, ?)"
            )
            .bind(format!("uuid-{}", i))
            .bind(format!("Post Title {}", i))
            .bind(format!("post-title-{}", i))
            .bind(format!("Content {}", i))
            .bind(1700000000i64 + i)
            .bind(1700000000i64 + i)
            .execute(&pool)
            .await
            .unwrap();
        }
        
        pool
    });

    c.bench_function("query_post_by_id", |b| {
        b.to_async(&rt).iter(|| {
            let pool = pool.clone();
            async move {
                let result = sqlx::query("SELECT * FROM posts WHERE id = ?")
                    .bind(25)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                black_box(result)
            }
        });
    });
}

// 基准测试 3: 通过 slug 查询（模拟实际路由）
fn bench_query_post_by_slug(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(async {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        
        sqlx::query(
            r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                content TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                visibility TEXT NOT NULL DEFAULT 'public',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                published_at INTEGER
            );
            CREATE INDEX idx_posts_slug ON posts(slug);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        
        for i in 1..=100 {
            sqlx::query(
                "INSERT INTO posts (uuid, title, slug, content, status, visibility, created_at, updated_at) 
                 VALUES (?, ?, ?, ?, 'published', 'public', ?, ?)"
            )
            .bind(format!("uuid-{}", i))
            .bind(format!("Post Title {}", i))
            .bind(format!("post-title-{}", i))
            .bind(format!("Content {}", i))
            .bind(1700000000i64 + i)
            .bind(1700000000i64 + i)
            .execute(&pool)
            .await
            .unwrap();
        }
        
        pool
    });

    c.bench_function("query_post_by_slug", |b| {
        b.to_async(&rt).iter(|| {
            let pool = pool.clone();
            async move {
                let result = sqlx::query("SELECT * FROM posts WHERE slug = ?")
                    .bind("post-title-50")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                black_box(result)
            }
        });
    });
}

// 基准测试 4: JSON 序列化性能
fn bench_json_serialization(c: &mut Criterion) {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone)]
    struct PostSummary {
        id: i64,
        uuid: String,
        title: String,
        slug: String,
        excerpt: String,
        status: String,
        visibility: String,
        created_at: i64,
        updated_at: i64,
    }

    let posts: Vec<PostSummary> = (0..100)
        .map(|i| PostSummary {
            id: i,
            uuid: format!("uuid-{}", i),
            title: format!("Post Title {}", i),
            slug: format!("post-title-{}", i),
            excerpt: format!("Excerpt for post {}...", i),
            status: "published".to_string(),
            visibility: "public".to_string(),
            created_at: 1700000000 + i,
            updated_at: 1700000000 + i,
        })
        .collect();

    let mut group = c.benchmark_group("json_serialization");

    // 测试不同大小的数据集
    for size in [1, 10, 50, 100].iter() {
        let subset: Vec<PostSummary> = posts.iter().take(*size).cloned().collect();

        group.bench_with_input(BenchmarkId::new("serialize", size), &subset, |b, data| {
            b.iter(|| {
                let json = serde_json::to_string(data).unwrap();
                black_box(json)
            });
        });

        // 测试反序列化
        let json_data = serde_json::to_string(&subset).unwrap();
        group.bench_with_input(
            BenchmarkId::new("deserialize", size),
            &json_data,
            |b, data| {
                b.iter(|| {
                    let posts: Vec<PostSummary> = serde_json::from_str(data).unwrap();
                    black_box(posts)
                });
            },
        );
    }

    group.finish();
}

// 基准测试 5: 插入操作性能
fn bench_insert_post(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let pool = rt.block_on(async {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query(
            r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                content TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                visibility TEXT NOT NULL DEFAULT 'public',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                published_at INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    });

    let mut counter = 0;

    c.bench_function("insert_post", |b| {
        b.to_async(&rt).iter(|| {
            counter += 1;
            let count = counter;
            let pool = pool.clone();
            async move {
                let result = sqlx::query(
                    "INSERT INTO posts (uuid, title, slug, content, status, visibility, created_at, updated_at) 
                     VALUES (?, ?, ?, ?, 'draft', 'public', ?, ?)"
                )
                .bind(format!("uuid-bench-{}", count))
                .bind(format!("Benchmark Post {}", count))
                .bind(format!("benchmark-post-{}", count))
                .bind("Benchmark content")
                .bind(1700000000i64)
                .bind(1700000000i64)
                .execute(&pool)
                .await
                .unwrap();
                black_box(result)
            }
        });
    });
}

criterion_group!(
    benches,
    bench_query_posts_list,
    bench_query_post_by_id,
    bench_query_post_by_slug,
    bench_json_serialization,
    bench_insert_post
);
criterion_main!(benches);
