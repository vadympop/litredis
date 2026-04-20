use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Parser, Serialize, Deserialize, Default)]
#[command(name = "litredis", about = "Async in-memory key-value server")]
struct PartialConfig {
    /// TCP port to listen on
    #[arg(long)]
    port: Option<u16>,

    /// TCP host to listen on
    #[arg(long)]
    host: Option<String>,

    /// Path to the JSON snapshot file
    #[arg(long)]
    snapshot_path: Option<PathBuf>,

    /// Milliseconds between periodic background snapshots
    #[arg(long)]
    flush_interval: Option<u64>,

    /// Require clients to authenticate with this password
    #[arg(long)]
    password: Option<String>,

    /// Path to JSON config file
    #[arg(long, value_name = "FILE")]
    #[serde(skip)]
    config_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub snapshot_path: PathBuf,
    pub flush_interval: u64,
    pub password: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: 9736,
            host: "0.0.0.0".into(),
            snapshot_path: PathBuf::from("dump.json"),
            flush_interval: 300,
            password: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let cli = PartialConfig::parse();
        let file = match &cli.config_file {
            Some(path) => {
                let text = std::fs::read_to_string(path)?;
                serde_json::from_str::<PartialConfig>(&text)?
            }
            None => PartialConfig::default(),
        };
        build_config(cli, file)
    }
}

fn build_config(cli: PartialConfig, file: PartialConfig) -> Result<Config> {
    let mut merged = serde_json::to_value(Config::default())?;
    json_merge(&mut merged, serde_json::to_value(&file)?);
    json_merge(&mut merged, serde_json::to_value(&cli)?);
    Ok(serde_json::from_value(merged)?)
}

fn json_merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                if !v.is_null() {
                    json_merge(b.entry(k).or_insert(Value::Null), v);
                }
            }
        }
        (b, o) if !o.is_null() => *b = o,
        _ => {}
    }
}
