use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Export {
            output,
            database,
            upload_dir,
        }) => {
            colophon::cli::export::run(database, output, upload_dir).await
        }
        None => {
            colophon::serve().await
        }
    }
}
