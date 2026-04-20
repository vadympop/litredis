use redis_app::{config::Config, server};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let config = Config::load()?;
    log::info!("litredis starting on port {}", config.port);
    server::run(config).await
}
