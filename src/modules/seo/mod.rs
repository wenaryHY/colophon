pub mod meta;
pub mod robots;
pub mod sitemap;

// 从 Host header 推断 site_url，用于空值兜底
pub use sitemap::infer_site_url_from_host_header;
