use stillrun::{cli, Result};

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
