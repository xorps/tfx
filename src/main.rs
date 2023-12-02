#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tfx::Cli { command } = tfx::Cli::parse();

    match command {
        tfx::Command::Validate { max_concurrency } => tfx::read_dir(max_concurrency).await,
    }
}
