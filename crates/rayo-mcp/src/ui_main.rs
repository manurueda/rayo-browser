#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rayo_ui::run().await
}
