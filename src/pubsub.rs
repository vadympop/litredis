use std::{collections::HashMap, sync::atomic::AtomicU64};
use tokio::sync::{Mutex, mpsc::UnboundedSender};

pub type ClientId = u64;

#[derive(Clone)]
pub struct PubSubMessage {
    pub channel: String,
    pub message: String,
}

pub struct PubSub {
    next_client_id: AtomicU64,
    channels: Mutex<HashMap<String, HashMap<ClientId, UnboundedSender<PubSubMessage>>>>,
}

impl PubSub {
    pub fn new() -> Self {
        Self {
            next_client_id: AtomicU64::new(1),
            channels: Mutex::new(HashMap::new()),
        }
    }

    pub fn next_client_id(&self) -> ClientId {
        self.next_client_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn subscribe(
        &self,
        client_id: ClientId,
        channel: String,
        sender: UnboundedSender<PubSubMessage>,
    ) {
        let mut channels = self.channels.lock().await;
        channels
            .entry(channel)
            .or_default()
            .insert(client_id, sender);
    }

    pub async fn unsubscribe(&self, client_id: ClientId, channel: &str) {
        let mut channels = self.channels.lock().await;
        if let Some(subs) = channels.get_mut(channel) {
            subs.remove(&client_id);
            if subs.is_empty() {
                channels.remove(channel);
            }
        };
    }

    pub async fn unsubscribe_all(&self, client_id: ClientId) {
        let mut channels = self.channels.lock().await;
        channels.retain(|_, subs| {
            subs.remove(&client_id);
            !subs.is_empty()
        });
    }

    pub async fn publish(&self, channel: &str, message: String) -> usize {
        let subscribers = {
            let channels = self.channels.lock().await;

            match channels.get(channel) {
                Some(subs) => subs
                    .iter()
                    .map(|(client_id, sender)| (*client_id, sender.clone()))
                    .collect::<Vec<_>>(),
                None => return 0,
            }
        };

        let msg = PubSubMessage {
            channel: channel.to_string(),
            message,
        };

        let mut count = 0;
        let mut dead_clients = Vec::new();

        for (client_id, sender) in subscribers {
            if sender.send(msg.clone()).is_ok() {
                count += 1;
            } else {
                dead_clients.push(client_id);
            }
        }

        // remove dead clients
        if !dead_clients.is_empty() {
            let mut channels = self.channels.lock().await;

            if let Some(subs) = channels.get_mut(channel) {
                for client_id in dead_clients {
                    subs.remove(&client_id);
                }

                if subs.is_empty() {
                    channels.remove(channel);
                }
            }
        }

        count
    }
}

impl Default for PubSub {
    fn default() -> Self {
        Self::new()
    }
}
