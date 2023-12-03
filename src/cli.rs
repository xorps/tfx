use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Validate(Validate),
}

#[derive(Debug, Parser)]
pub struct Validate {
    /// maximum concurrency for filesystem traversal
    #[arg(long, default_value_t = 64)]
    pub max_concurrency_fs: usize,

    /// maximum concurrency for processes
    #[arg(long, default_value_t = 64)]
    pub max_concurrency_process: usize,
}

impl Cli {
    pub fn parse() -> Self {
        // so consumers don't need to import trait
        <Self as Parser>::parse()
    }
}
