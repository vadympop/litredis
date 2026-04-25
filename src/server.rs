use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::net::TcpListener;

use crate::config::Config;
use crate::connection::handle_connection;
use crate::pubsub::PubSub;
use crate::store::Store;

pub struct Shared {
    pub config: Config,
    pub store: Store,
    pub pubsub: PubSub,
}

impl Shared {
    pub fn create(config: Config) -> Arc<Shared> {
        Arc::new(Shared {
            config,
            store: Store::new(),
            pubsub: PubSub::new(),
        })
    }
}

/// Binds the TCP listener and starts the connections loop
pub async fn run(config: Config) -> Result<()> {
    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr).await?;
    log::info!("listening on {}", listener.local_addr()?);
    let shared = Shared::create(config);
    tokio::spawn(clean_expired_loop(shared.clone()));
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

async fn clean_expired_loop(shared: Arc<Shared>) {
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    loop {
        ticker.tick().await;
        shared.store.purge_expired();
    }
}
