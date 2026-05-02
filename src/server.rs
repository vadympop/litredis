use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio::signal;
use crate::config::Config;
use crate::connection::handle_connection;
use crate::persistence;
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
    tokio::spawn(save_snapshots_loop(shared.clone()));
    tokio::select! {
        result = connections_loop(listener, shared.clone()) => result,
        _ = signal::ctrl_c() => {
            if let Some(path) = &shared.config.snapshot_path {
                if let Err(e) = persistence::save(&shared.store, path) {
                    log::error!("shutdown snapshot flush failed: {e}");
                }
            }
            Ok(())
        }
    }
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

async fn save_snapshots_loop(shared: Arc<Shared>) {
    let mut ticker = tokio::time::interval(Duration::from_millis(shared.config.flush_interval));
    loop {
        ticker.tick().await;
        if let Some(path) = &shared.config.snapshot_path {
            if let Err(e) = persistence::save(&shared.store, path) {
                log::error!("periodic snapshot flush failed: {e}");
            }
        }
    }
}
