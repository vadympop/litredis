use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::connection::handle_connection;
use crate::store::Store;

pub struct Shared {
    pub config: Config,
    pub store: Store
}

impl Shared {
    pub fn create(config: Config) -> Arc<Shared> {
        Arc::new(Shared { config, store: Store::new() })
    }
}

/// Binds the TCP listener and starts the connections loop
pub async fn run(config: Config) -> Result<()> {
    let addr = format!("{}:{}", config.host, config.port);
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
        let shared = Arc::clone(&shared);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, shared).await {
                log::error!("connection error from {}: {}", peer, e);
            }
        });
    }
}
