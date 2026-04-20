use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "litredis", about = "Async in-memory key-value server")]
pub struct Config {
    /// TCP port to listen on
    #[arg(long, default_value = "9736")]
    pub port: u16,

    /// TCP host to listen on
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    /// Path to the JSON snapshot file
    #[arg(long, default_value = "dump.json")]
    pub snapshot_path: PathBuf,

    /// Milliseconds between periodic background snapshots
    #[arg(long, default_value = "300")]
    pub flush_interval: u64,

    /// Require clients to authenticate with this password
    #[arg(long)]
    pub password: Option<String>,
}
