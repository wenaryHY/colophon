use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "colophon", about = "A personal blog engine")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 将数据库内容导出为 JSON 文件，供静态站点生成器使用
    Export {
        /// 导出目标目录（默认 ./export-data）
        #[arg(long, default_value = "./export-data")]
        output: PathBuf,

        /// SQLite 数据库文件路径
        #[arg(long, default_value = "colophon.db")]
        database: PathBuf,

        /// 上传文件目录，用于复制媒体文件（默认 ./uploads）
        #[arg(long, default_value = "uploads")]
        upload_dir: PathBuf,
    },
    /// Schema-as-Code 工具
    Schema {
        #[command(subcommand)]
        action: SchemaAction,
    },
}

#[derive(Subcommand)]
enum SchemaAction {
    /// 从 schemas/*.toml 生成代码和迁移 SQL
    Generate {
        /// Schema 文件目录（默认 ./schemas）
        #[arg(long, default_value = "schemas")]
        schema_dir: PathBuf,

        /// 项目根目录（默认当前目录）
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },
}

/// 初始化 tracing 日志系统。
///
/// 优先级：
/// 1. `RUST_LOG` 环境变量（完全自定义）
/// 2. `COLOPHON_LOG_FORMAT` 环境变量控制输出格式（json / pretty，默认 pretty）
/// 3. 默认过滤级别：colophon=info
fn init_tracing() {
    let log_format = std::env::var("COLOPHON_LOG_FORMAT").unwrap_or_else(|_| "pretty".into());
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "colophon=info,axum=info,tower_http=info".into());

    if log_format == "json" {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(false)
                    .flatten_event(true),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_target(false),
            )
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Export {
            output,
            database,
            upload_dir,
        }) => {
            colophon::cli::export::run(database, output, upload_dir).await
        }
        Some(Commands::Schema { action }) => {
            match action {
                SchemaAction::Generate {
                    schema_dir,
                    project_root,
                } => {
                    colophon::cli::schema::generate::run(&project_root, &schema_dir).await
                }
            }
        }
        None => {
            colophon::serve().await
        }
    }
}
