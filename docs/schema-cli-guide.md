# Schema-as-Code CLI Guide

## Overview
Schema-as-Code CLI generates CRUD modules from TOML config files.

## Quick Start

### 1. Create Schema File
Create a toml file in schemas/ directory:

    [collection]
    name = "Article"
    table = "articles"
    display_name = "Article"

    [features]
    soft_delete = true
    timestamps = true
    sort_order = true

    [[fields]]
    name = "title"
    type = "text"
    required = true

    [[fields]]
    name = "slug"
    type = "text"
    required = true
    unique = true

    [[fields]]
    name = "content"
    type = "richtext"
    required = true

    [[fields]]
    name = "category_id"
    type = "relation"
    required = false

## 2. Run Generate Command

    cargo run -- schema generate

Output:

    [INFO] Processing collection: Article
    [INFO]   Generated: src/modules/article/domain.rs
    [INFO]   Generated: src/modules/article/dto.rs
    [INFO]   Generated: src/modules/article/repository.rs
    [INFO]   Generated: src/modules/article/handler.rs
    [INFO]   Generated: src/modules/article/service.rs
    [INFO]   Generated: src/modules/article/mod.rs
    [INFO]   Migration: migrations/027_create_articles.sql
    [INFO]   Lock file: .colophon.lock

### 3. Register Module
Add to src/modules/mod.rs:

    pub mod article;

### 4. Register Routes
Add routes in src/bootstrap/router.rs.

### 5. Run Tests

    cargo test --lib modules::article

## TOML Format

### [collection]

 Field | Required | Description
-------|---------|------------
 name | Yes | PascalCase struct name (e.g. Category)
 table | Yes | snake_case table name (e.g. categories)
 display_name | No | Display name for error messages

### [features]

 Field | Default | Description
-------|---------|------------
 soft_delete | false | Add deleted_at TEXT field
 timestamps | false | Add created_at / updated_at fields
 sort_order | false | Add sort_order INTEGER field

### [[fields]]

 Field | Required | Description
-------|---------|------------
 name | Yes | snake_case field name
 type | Yes | Field type (see below)
 required | false | Generate NOT NULL constraint
 unique | false | Generate UNIQUE constraint
 computed | false | Exclude from CreateDTo
 references | false | Foreign key table name

### Type Mapping

 TOML type | Rust type | SQLite type
----------|-----------|-------------
 text | String | TEXT
 richtext | String | TEXT
 boolean | bool | INTEGER
 integer | i64 | INTEGER
 timestamp | String | TEXT
 relation | Option<String> | TEXT

## Generated Files

    src/modules/{name}/
      domain.rs      # Database entity struct
      dto.rs        # Create/Update request structs
      repository.rs  # CRUD database operations
      handler.rs     # HTTP route handlers
      service.rs     # Business logic layer
      mod.rs         # Module declaration

## Incremental Updates
When you modify a TOML file and run generate again:

- New fields: Auto-generates ALTER TABLE migration
- Deleted fields: Error (manual handling required)
  Type changes: Error (manual handling required)
  No changes: Skips generation
## Lock File
.colophon.lock tracks generated schema state. Do not edit manually

## Custom Logic
Generated service.rs is standard CRUD. Edit the file directly for custom business logic. The CLI will not overwrite existing files on subsequent runs.
## Full Example

    [collection]
    name = "Product"
    table = "products"
    display_name = "Product"

    [features]
    soft_delete = true
    timestamps = true
    sort_order = true

    [[fields]]
    name = "name"
    type = "text"
    required = true
    unique = true

    [[fields]]
    name = "slug"
    type = "text"
    required = true
    unique = true

    [[fields]]
    name = "price"
    type = "integer"
    required = true

    [[fields]]
    name = "description"
    type = "richtext"
    required = false

    [[fields]]
    name = "category_id"
    type = "relation"
    required = false

Run:

    cargo run -- schema generate
    # Output: 6 files + 1 migration SQL