use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;

use crate::config::Config;

pub struct Shared {
    pub config: Config,
}

impl Shared {
    pub fn create(config: Config) -> Arc<Shared> {
        Arc::new(Shared { config })
    }
}

/// Binds the TCP listener and starts the connections loop
pub async fn run(config: Config) -> Result<()> {
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;
    log::info!("listening on {}", listener.local_addr()?);
    let shared = Shared::create(config);
    connections_loop(listener, shared).await
}

/// Accepts connections forever, spawning a task per client
pub async fn connections_loop(listener: TcpListener, shared: Arc<Shared>) -> Result<()> {
    loop {
        let (socket, peer) = listener.accept().await?;
        log::debug!("accepted connection from {}", peer);
        tokio::spawn(async move {
            todo!()
        });
    }
}
