# Colophon 主题开发指南

本文档面向希望为 Colophon 开发自定义主题的第三方开发者。阅读本文前，你需要了解基础的 HTML 和 CSS 知识，无需任何后端或 Rust 经验。

---

## 目录

1. [快速开始](#1-快速开始)
2. [theme.toml 配置](#2-themetoml-配置)
3. [模板文件约定](#3-模板文件约定)
4. [模板可用变量](#4-模板可用变量)
5. [目录结构规范](#5-目录结构规范)
6. [上传和测试](#6-上传和测试)
7. [MiniJinja 语法速查](#7-minijinja-语法速查)
8. [常见问题](#8-常见问题)

---

## 1. 快速开始

### 最小主题的目录结构

一个可用的主题最少只需要三个文件：

```
my-theme/
├── theme.toml
└── templates/
    ├── index.html
    └── post.html
```

### 复制 default 主题作为起点

推荐从内置的 `default` 主题开始修改。将 `themes/default/` 整个目录复制一份：

```
cp -r themes/default themes/my-theme
```

然后修改 `themes/my-theme/theme.toml` 中的 `name` 和 `slug` 为你自己的值。

### 第一条命令

启动开发服务器后，在后台「主题」页面将你的主题上传或激活即可预览。

---

## 2. theme.toml 配置

`theme.toml` 是主题的身份证，放置于主题根目录，系统扫描主题目录时读取它。

### 基础字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | 字符串 | 是 | 主题的显示名称，如 `"My Theme"` |
| `slug` | 字符串 | 是 | 主题唯一标识符，只能包含小写字母、数字、连字符。如 `"my-theme"` |
| `version` | 字符串 | 是 | 语义化版本号，如 `"1.0.0"` |
| `author` | 字符串 | 是 | 作者名称，如 `"Your Name"` |
| `description` | 字符串 | 是 | 简短描述，在后台主题列表中展示 |
| `min_colophon_version` | 字符串 | 是 | 主题要求的最低 Colophon 版本，如 `"0.2.0"` |
| `preview_image` | 字符串 | 否 | 预览图文件名，指向 `static/` 下的图片。如 `"screenshot.png"` |

### 基础示例

```toml
name = "My Blog Theme"
slug = "my-theme"
version = "1.0.0"
author = "Your Name"
description = "一个简洁的博客主题"
min_colophon_version = "0.2.0"
preview_image = "screenshot.png"
```

### [config] 自定义配置

如果你的主题需要让用户在前台或后台调整某些参数（如主色调、字体大小、首页布局等），可以定义一个 `[config]` section。系统会在后台为该主题生成对应的配置面板。

`[config]` 是一个键值映射，每个键是一个配置项的 `slug`，值是一个包含 `type`、`label` 等属性的对象。

#### 四种配置类型

##### text —— 文本输入

```toml
[config.hero_title]
type = "text"
label = "首页标题"
default = "欢迎来到我的博客"
```

##### color —— 颜色选择器

```toml
[config.accent_color]
type = "color"
label = "主题色"
default = "#FF6D00"
```

##### select —— 下拉选择

```toml
[config.layout]
type = "select"
label = "首页布局"

[[config.layout.options]]
label = "卡片布局"
value = "card"

[[config.layout.options]]
label = "列表布局"
value = "list"
```

##### number —— 数字输入

```toml
[config.posts_per_page]
type = "number"
label = "每页文章数"
default = 10
```

#### 完整示例

```toml
name = "My Blog Theme"
slug = "my-theme"
version = "1.0.0"
author = "Your Name"
description = "一个简洁的博客主题"
min_colophon_version = "0.2.0"

[config.hero_title]
type = "text"
label = "首页标题"
default = "欢迎来到我的博客"

[config.accent_color]
type = "color"
label = "主题色"
default = "#FF6D00"

[config.layout]
type = "select"
label = "首页布局"
default = "card"

[[config.layout.options]]
label = "卡片布局"
value = "card"

[[config.layout.options]]
label = "列表布局"
value = "list"

[config.posts_per_page]
type = "number"
label = "每页文章数"
default = 10
```

#### 在模板中使用配置

用户保存的自定义配置通过 `{{ theme_config }}` 全局变量注入模板。它是一个键值对象：

```html
<h1>{{ theme_config.hero_title }}</h1>
<style>
:root {
  --accent: {{ theme_config.accent_color }};
}
</style>
```

配置值在 MiniJinja 模板中**默认经过 HTML 转义**，可以安全使用。不要在配置值上使用 `| safe` 过滤器。

---

## 3. 模板文件约定

所有模板文件放在 `templates/` 目录下，Colophon 使用 [MiniJinja](https://github.com/mitsuhiko/minijinja) 作为模板引擎。

### 模板文件清单

| 文件名 | 是否强制 | 触发的路由 | 用途 |
|--------|---------|-----------|------|
| `index.html` | 强制 | `/` | 首页 |
| `post.html` | 强制 | `/posts/{slug}` | 文章详情页 |
| `tag.html` | 可选 | `/tags/{slug}` | 标签归档页。缺失时回退到 `index.html` |
| `category.html` | 可选 | `/categories/{slug}` | 分类归档页。缺失时回退到 `index.html` |
| `search.html` | 可选 | `/search` | 搜索页。缺失时回退到 `index.html` |
| `author.html` | 可选 | `/author/{username}` | 作者归档页。缺失时回退到 `index.html` |
| `page.html` | 可选 | `/pages/{slug}` | 页面详情页。缺失时回退到 `post.html` |
| `404.html` | 推荐 | 任何未匹配的路径 | 404 错误页。缺失时显示纯文本 "404 - 页面未找到" |
| `500.html` | 推荐 | 服务器错误时 | 500 错误页。缺失时显示默认错误信息 |
| `profile.html` | 可选 | `/profile` | 用户个人中心页 |
| `login.html` | 可选 | `/login` | 登录页 |
| `register.html` | 可选 | `/register` | 注册页 |

### 关于可选模板的说明

- **`page.html`**：Colophon 支持两种内容类型——文章（post）和页面（page）。页面通常用于「关于」「友情链接」等独立页面。如果主题没有 `page.html`，页面会使用 `post.html` 渲染。
- **`tag.html` / `category.html`**：如果需要为标签归档或分类归档定制不同的视觉样式，可以提供这两个模板。缺失时统一使用 `index.html` 渲染。
- **`profile.html` / `login.html` / `register.html`**：提供这些模板可以定制认证相关页面的外观。缺失时 Colophon 使用内置的默认页面。
- **`404.html` / `500.html`**：强烈建议提供，以减少跳出率。

### 模板 include

可以把公共部分抽取为子模板，用 `{% include %}` 引入。子模板建议以下划线开头，表明是部分模板：

```
templates/
├── _header.html
├── _footer.html
├── _pagination.html
├── _tag_cloud.html
├── index.html
├── post.html
└── ...
```

在模板中引用：

```html
{% include "_header.html" %}
```

---

## 4. 模板可用变量

### 全局变量

所有模板页面均可使用的变量：

| 变量 | 类型 | 说明 |
|------|------|------|
| `{{ site_title }}` | 字符串 | 站点名称（后台设置中配置） |
| `{{ site_description }}` | 字符串 | 站点描述 |
| `{{ site_url }}` | 字符串 | 站点完整 URL，如 `https://example.com` |
| `{{ admin_url }}` | 字符串 | 后台管理地址，默认 `/admin` |
| `{{ current_lang }}` | 字符串 | 当前语言代码，`"zh"` 或 `"en"` |
| `{{ theme_config }}` | 对象 | 主题自定义配置（见 [theme.toml 配置](#config-自定义配置)） |
| `{{ plugin_head }}` | HTML 字符串 | 插件注入到 `<head>` 的内容。**必须使用 `\| safe` 过滤器** |
| `{{ plugin_body }}` | HTML 字符串 | 插件注入到 `</body>` 前的内容。**必须使用 `\| safe` 过滤器** |

用法示例：

```html
<html lang="{{ current_lang }}">
<head>
    <title>{{ site_title }}</title>
    <meta name="description" content="{{ site_description }}">
    <meta property="og:site_name" content="{{ site_title }}">
    {{ plugin_head|safe }}
</head>
<body>
    ...
    {{ plugin_body|safe }}
</body>
</html>
```

### 页面专属变量

不同模板页面会额外注入不同的上下文变量。

#### index.html —— 首页

| 变量 | 类型 | 说明 |
|------|------|------|
| `{{ seo_meta }}` | 对象 | SEO 元数据，包含 `title`、`description`、`keywords`、`canonical_url`、`og_title`、`og_description`、`og_url`、`og_type`、`og_image`、`twitter_card`、`twitter_title`、`twitter_description`、`twitter_image` |
| `{{ json_ld }}` | HTML 字符串 | JSON-LD 结构化数据。**使用 `\| safe` 过滤器** |
| `{{ posts }}` | 数组 | 最近 20 篇公开文章列表 |
| `{{ current_user }}` | 对象 / 空 | 当前登录用户信息，未登录时为空 |

每篇文章 (`posts` 中每个元素) 包含以下字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | 字符串 | 文章 ID |
| `title` | 字符串 | 标题 |
| `slug` | 字符串 | URL 标识 |
| `excerpt` | 字符串 / null | 摘要 |
| `content_type` | 字符串 | `"post"` 或 `"page"` |
| `published_at` | 字符串 / null | 发布时间（ISO 8601 格式） |
| `created_at` | 字符串 | 创建时间 |
| `updated_at` | 字符串 | 更新时间 |
| `author_display_name` | 字符串 | 作者显示名 |
| `category_name` | 字符串 / null | 分类名称 |
| `category_id` | 字符串 / null | 分类 ID |

文章列表基本用法：

```html
{% if posts|length > 0 %}
  {% for post in posts %}
    <a href="/posts/{{ post.slug }}">
      <h2>{{ post.title }}</h2>
      <span>{{ post.author_display_name }}</span>
      <span>{{ (post.published_at or post.created_at)[:10] }}</span>
      {% if post.excerpt %}
        <p>{{ post.excerpt }}</p>
      {% endif %}
    </a>
  {% endfor %}
{% else %}
  <p>暂无文章。</p>
{% endif %}
```

#### post.html —— 文章详情页

| 变量 | 类型 | 说明 |
|------|------|------|
| `{{ post }}` | 对象 | 文章完整信息 |
| `{{ seo_meta }}` | 对象 | SEO 元数据 |
| `{{ json_ld }}` | HTML 字符串 | JSON-LD 结构化数据。**使用 `\| safe` 过滤器** |
| `{{ image }}` | 字符串 | 封面图完整 URL，无封面时为空字符串 |
| `{{ comments }}` | 数组 | 已审核通过的评论列表 |
| `{{ current_user }}` | 对象 / 空 | 当前登录用户 |
| `{{ plugins }}` | 对象 | 插件额外数据（由 `post.before_render` 钩子注入） |

文章对象 (`post`) 包含以下字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | 字符串 | 文章 ID |
| `title` | 字符串 | 标题 |
| `slug` | 字符串 | URL 标识 |
| `excerpt` | 字符串 / null | 摘要 |
| `content_html` | HTML 字符串 | 正文（Markdown 转 HTML 后的内容）。**渲染时必须使用 `\| safe`** |
| `content_type` | 字符串 | `"post"` 或 `"page"` |
| `allow_comment` | 布尔值 | 是否允许评论 |
| `published_at` | 字符串 / null | 发布时间 |
| `created_at` | 字符串 | 创建时间 |
| `updated_at` | 字符串 | 更新时间 |
| `author_display_name` | 字符串 | 作者显示名 |
| `category_name` | 字符串 / null | 分类名称 |
| `cover_media_id` | 字符串 / null | 封面资源 ID |

文章详情页基本用法：

```html
<article>
  <h1>{{ post.title }}</h1>
  <div>
    <span>{{ post.author_display_name }}</span>
    <span>{{ post.published_at or post.created_at }}</span>
  </div>
  {% if post.excerpt %}
    <p>{{ post.excerpt }}</p>
  {% endif %}
  <div>
    {{ post.content_html | safe }}
  </div>
</article>
```

#### tag.html —— 标签归档页

| 变量 | 类型 | 说明 |
|------|------|------|
| `{{ tag }}` | 对象 | 标签信息，包含 `name` 和 `slug` |
| `{{ posts }}` | 数组 | 该标签下的文章列表 |
| `{{ page }}` | 数字 | 当前页码 |
| `{{ page_size }}` | 数字 | 每页数量（固定 20） |
| `{{ total }}` | 数字 | 文章总数 |
| `{{ total_pages }}` | 数字 | 总页数 |
| `{{ seo_meta }}` | 对象 | SEO 元数据 |
| `{{ json_ld }}` | HTML 字符串 | JSON-LD |
| `{{ current_user }}` | 对象 / 空 | 当前登录用户 |

#### category.html —— 分类归档页

| 变量 | 类型 | 说明 |
|------|------|------|
| `{{ category }}` | 对象 | 分类信息，包含 `name`、`slug`、`description` |
| `{{ posts }}` | 数组 | 该分类下的文章列表 |
| `{{ page }}` | 数字 | 当前页码 |
| `{{ page_size }}` | 数字 | 每页数量（固定 20） |
| `{{ total }}` | 数字 | 文章总数 |
| `{{ total_pages }}` | 数字 | 总页数 |
| `{{ seo_meta }}` | 对象 | SEO 元数据 |
| `{{ json_ld }}` | HTML 字符串 | JSON-LD |
| `{{ current_user }}` | 对象 / 空 | 当前登录用户 |

#### 404.html / 500.html —— 错误页

仅注入全局变量，无额外的上下文变量。

#### profile.html —— 个人中心

| 变量 | 类型 | 说明 |
|------|------|------|
| `{{ current_user }}` | 对象 | 当前登录用户的完整信息 |

`current_user` 对象包含：`id`、`username`、`display_name`、`email`、`role`、`language`、`theme_preference`、`bio`、`created_at` 等。

#### login.html / register.html

| 变量 | 类型 | 说明 |
|------|------|------|
| `{{ redirect_to }}` | 字符串 | 登录 / 注册成功后的跳转地址 |

### 模板函数

Colophon 向模板引擎注入了以下函数，可在任意模板中调用：

| 函数 | 说明 |
|------|------|
| `{{ theme_assets_url('css/style.css') }}` | 生成主题静态资源的 URL，带有内容哈希用于缓存失效 |
| `{{ get_recent_posts(5) }}` | 获取最近 N 篇文章，返回值与首页 `posts` 结构相同。参数可选，不传则返回所有 |
| `{{ get_tags() }}` | 获取所有标签列表，每个元素包含 `name`、`slug` |
| `{{ get_categories() }}` | 获取所有分类列表，每个元素包含 `name`、`slug` |

函数使用示例：

```html
<!-- 引入主题 CSS -->
<link rel="stylesheet" href="{{ theme_assets_url('css/theme.css') }}">
<link rel="stylesheet" href="{{ theme_assets_url('js/theme.js') }}">

<!-- 渲染标签云 -->
<div class="tag-cloud">
  {% for tag in get_tags() %}
    <a href="/tags/{{ tag.slug }}">{{ tag.name }}</a>
  {% endfor %}
</div>

<!-- 侧边栏显示最近 5 篇文章 -->
<aside>
  <h3>最近文章</h3>
  <ul>
  {% for post in get_recent_posts(5) %}
    <li><a href="/posts/{{ post.slug }}">{{ post.title }}</a></li>
  {% endfor %}
  </ul>
</aside>
```

### 自定义过滤器

Colophon 额外注册了两个过滤器：

| 过滤器 | 说明 |
|--------|------|
| `\| tojson_script` | 将值序列化为 JSON 字符串，并转义 `<`、`>`、`&` 字符，安全嵌入 `<script>` 标签 |

```html
<script>
  // 将后端数据安全地传递给前端 JS
  const posts = {{ get_recent_posts(100) | tojson_script | safe }};
  const user = {{ current_user | tojson_script | safe }};
</script>
```

### 密钥变量注入（auth 相关模板专用）

登录、注册、个人中心页面渲染时会通过 MiniJinja context 注入额外的认证相关变量，如 `redirect_to`。这些变量由对应的 handler 注入，不通过全局变量提供。

---

## 5. 目录结构规范

```
themes/my-theme/
├── theme.toml              ← 主题配置（强制）
├── screenshot.png          ← 预览图（可选，大小不限）
├── templates/              ← 模板目录
│   ├── index.html          ← 首页（强制）
│   ├── post.html           ← 文章详情页（强制）
│   ├── tag.html            ← 标签归档（可选）
│   ├── category.html       ← 分类归档（可选）
│   ├── search.html         ← 搜索页（可选）
│   ├── author.html         ← 作者归档（可选）
│   ├── page.html           ← 页面详情（可选）
│   ├── 404.html            ← 404 错误页（推荐）
│   ├── 500.html            ← 500 错误页（推荐）
│   ├── profile.html        ← 个人中心（可选）
│   ├── login.html          ← 登录页（可选）
│   ├── register.html       ← 注册页（可选）
│   ├── _header.html        ← 头部 partial（推荐）
│   ├── _footer.html        ← 底部 partial（推荐）
│   └── _pagination.html    ← 分页 partial（按需）
└── static/                 ← 静态资源
    ├── css/
    │   └── theme.css
    ├── js/
    │   └── theme.js
    ├── fonts/
    │   └── custom-font.woff2
    └── images/
        └── logo.png
```

### 静态资源引用

所有 `static/` 下的文件通过 `theme_assets_url()` 函数引用，该函数会自动附加文件内容哈希，实现生产环境的永久缓存：

```html
<link rel="stylesheet" href="{{ theme_assets_url('css/theme.css') }}">
<script src="{{ theme_assets_url('js/theme.js') }}"></script>
```

运行时实际生成的 URL 类似：

```
/static/themes/my-theme/css/theme.css?v=a3f2b1c
```

当静态文件内容变化时哈希自动更新，浏览器无需手动刷新缓存。

> 注意：不要在 URL 中写死 `v` 查询参数。`theme_assets_url()` 已经内置了缓存失效机制。

### 静态资源的安全路径约束

系统对静态资源请求做了路径遍历保护。URL 中不得包含 `..`、反斜杠 `\`、或以 `/` 开头。合法的静态资源请求格式为：

```
/static/themes/{theme_slug}/{file_path}
```

---

## 6. 上传和测试

### 打包主题

将你的主题目录打包为 ZIP 压缩包。确保 `theme.toml` 在 ZIP 的**根目录**下，而非嵌套在子文件夹中。

正确的 ZIP 结构：

```
my-theme.zip
├── theme.toml
├── templates/
│   ├── index.html
│   └── post.html
└── static/
    └── css/
        └── theme.css
```

错误的 ZIP 结构（多了一层文件夹）：

```
my-theme.zip
└── my-theme/          ← 这层不应该有
    ├── theme.toml
    └── ...
```

打包命令（在主题根目录下执行）：

```
zip -r my-theme.zip . -x "*.zip"
```

### 通过后台上传

1. 访问 Colophon 管理后台
2. 进入「主题」页面
3. 点击「上传主题」按钮
4. 选择打包好的 ZIP 文件
5. 上传成功后，在主题列表中可以看到你的主题

### 激活主题

1. 在主题列表中找到你的主题
2. 点击「激活」按钮
3. 系统立即切换并刷新所有页面缓存

### 配置主题

如果你的主题定义了 `[config]` section，在后台点击主题可进入配置页面，填写各项参数后保存。

### 本地开发测试建议

在开发过程中，可以直接将主题文件夹放在 `themes/` 目录下（与 `default` 并列），无需每次都打包上传。Colophon 启动时会扫描 `themes/` 目录下的所有子目录。

---

## 7. MiniJinja 语法速查

Colophon 使用 MiniJinja 作为模板引擎，语法与 Jinja2/Django 模板高度相似。

### 变量输出

```django
{{ variable }}
{{ post.title }}
{{ theme_config.accent_color }}
```

### 条件判断

```django
{% if post.excerpt %}
  <p>{{ post.excerpt }}</p>
{% endif %}

{% if current_user %}
  你好，{{ current_user.display_name }}
{% else %}
  <a href="/login">请先登录</a>
{% endif %}
```

### 循环

```django
{% for post in posts %}
  <h2>{{ post.title }}</h2>
{% endfor %}

<!-- 带索引的循环 -->
{% for post in posts %}
  <span>{{ loop.index }}. {{ post.title }}</span>
{% endfor %}
```

loop 对象可用属性：`loop.index`（从 1 开始）、`loop.index0`（从 0 开始）、`loop.first`、`loop.last`。

### 包含子模板

```django
{% include "_header.html" %}
{% include "_footer.html" %}
```

### 注释

```django
{# 这是注释，不会出现在输出中 #}
```

```html
<!-- 这是 HTML 注释，会出现在输出中 -->
```

### 常用过滤器

| 过滤器 | 说明 | 示例 |
|--------|------|------|
| `\| safe` | 标记内容为安全的 HTML，不转义 | `{{ post.content_html \| safe }}` |
| `\| lower` | 转为小写 | `{{ "HELLO" \| lower }}` → `hello` |
| `\| upper` | 转为大写 | `{{ "hello" \| upper }}` → `HELLO` |
| `\| length` | 获取长度 | `{{ posts \| length }}` → 数组元素个数 |
| `\| default("值")` | 变量未定义时使用默认值 | `{{ site_title \| default("Colophon") }}` |

### 字符串切片

MiniJinja 支持 Python 风格的字符串切片：

```django
<!-- 截取日期前 10 个字符（YYYY-MM-DD） -->
{{ post.created_at[:10] }}

<!-- display_name 的首字母 -->
{{ current_user.display_name[:1] }}
```

### or 运算符

可以用 `or` 为可能为空的变量提供回退值：

```django
{{ post.published_at or post.created_at }}
```

### 身份验证变量

```django
<!-- 检查是否为管理员 -->
{% if current_user and current_user.role == "admin" %}
  <a href="/admin">管理后台</a>
{% endif %}
```

---

## 8. 常见问题

### Q: 为什么我的主题上传后激活了还是看不到？

可能的原因：

1. **`theme.toml` 的 `slug` 字段**与 ZIP 包内的目录路径不符。系统以 `slug` 字段为唯一标识，确保它与你预期的主题目录名一致。
2. **缺少强制模板** `index.html` 或 `post.html`。检查这两文件是否存在于 ZIP 包的 `templates/` 下。
3. **缓存未刷新**。激活主题后如果仍显示旧样式，尝试硬刷新浏览器（Ctrl+F5）。
4. **`theme.toml` 语法错误**。确保是合法的 TOML 格式，字段名严格区分大小写。

### Q: 模板中某些变量显示为空或 "undefined" 怎么办？

这说明该变量在当前页面的上下文中不存在。解决方案：

1. **先确认变量的作用域**。例如 `{{ post }}` 只在 `post.html` 和 `page.html` 中可用，在 `index.html` 中不存在。
2. **使用 `or` 或 `default()` 提供回退值**：

   ```django
   {{ theme_config.hero_title or "默认标题" }}
   {{ post.category_name | default("未分类") }}
   ```

3. **用 `{% if %}` 做条件判断**，避免打印空标签：

   ```django
   {% if post.excerpt %}
     <p>{{ post.excerpt }}</p>
   {% endif %}
   ```

### Q: 如何调试模板错误？

1. **查看服务器日志**。模板渲染错误会记录详细的堆栈信息，通常包含具体的模板文件名和行号。
2. **从简单开始**。新建主题时先用最简模板验证核心流程（仅 `index.html` + `post.html`），确认全局变量能正常输出后再逐步添加样式和功能。
3. **检查 MiniJinja 语法**。最常见的错误：
   - `{% endif %}` 或 `{% endfor %}` 缺失
   - `{{ }}` 和 `{% %}` 混用
   - 字符串引号不匹配
4. **使用 Colophon 的预览功能**。后台编辑器中可以实时预览文章在主题中的渲染效果，帮助你快速迭代模板。

### Q: 静态资源（CSS/JS）修改后浏览器不更新怎么办？

Colophon 的 `theme_assets_url()` 函数自动基于文件内容生成哈希版本号。只要文件内容确实发生了变化，URL 就会更新，浏览器会自动加载新版本。如果修改后仍未生效：

1. 确认修改的是**正确主题**的 static 目录（已激活的那个）。
2. 硬刷新浏览器（Ctrl+F5）。
3. 重启 Colophon 服务，让系统重新扫描文件哈希。

### Q: 可以引用外部 CDN 资源吗？

可以。在模板的 `<head>` 中直接使用 `<link>` 或 `<script>` 标签引用外部 URL 即可：

```html
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;700&display=swap" rel="stylesheet">
```

### Q: 主题的配置数据存储在哪里？

用户在后台填写的主题配置值存储在 Colophon 数据库的 `theme_configs` 表中，与主题文件分离。升级主题文件不会覆盖用户的配置数据。
