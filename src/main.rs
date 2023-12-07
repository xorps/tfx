#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tfx::Cli { command } = tfx::Cli::parse();

    match command {
        tfx::Command::Validate(v) => tfx::validate(v).await,
    }
}
