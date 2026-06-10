#[tokio::main]
async fn main() -> anyhow::Result<()> {
    colophon::serve().await
}
