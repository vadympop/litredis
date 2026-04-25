use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::mpsc::{Sender, UnboundedSender, error::TrySendError};

pub type ClientId = u64;
pub const PUBSUB_BUFFER_SIZE: usize = 1024;

#[derive(Clone)]
pub struct PubSubMessage {
    pub channel: String,
    pub message: String,
}

pub struct PubSub {
    next_client_id: AtomicU64,
    channels: Mutex<HashMap<String, HashMap<ClientId, Subscriber>>>,
}

#[derive(Clone)]
pub struct Subscriber {
    messages: Sender<PubSubMessage>,
    disconnect: UnboundedSender<()>,
}

impl PubSub {
    pub fn new() -> Self {
        Self {
            next_client_id: AtomicU64::new(1),
            channels: Mutex::new(HashMap::new()),
        }
    }

    pub fn next_client_id(&self) -> ClientId {
        self.next_client_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn subscribe(
        &self,
        client_id: ClientId,
        channel: String,
        messages: Sender<PubSubMessage>,
        disconnect: UnboundedSender<()>,
    ) {
        let mut channels = self.channels.lock().unwrap();
        channels.entry(channel).or_default().insert(
            client_id,
            Subscriber {
                messages,
                disconnect,
            },
        );
    }

    pub fn unsubscribe(&self, client_id: ClientId, channel: &str) {
        let mut channels = self.channels.lock().unwrap();
        if let Some(subs) = channels.get_mut(channel) {
            subs.remove(&client_id);
            if subs.is_empty() {
                channels.remove(channel);
            }
        };
    }

    pub fn unsubscribe_all(&self, client_id: ClientId) {
        let mut channels = self.channels.lock().unwrap();
        channels.retain(|_, subs| {
            subs.remove(&client_id);
            !subs.is_empty()
        });
    }

    pub fn publish(&self, channel: &str, message: String) -> usize {
        let subscribers = {
            let channels = self.channels.lock().unwrap();

            match channels.get(channel) {
                Some(subs) => subs
                    .iter()
                    .map(|(client_id, subscriber)| (*client_id, subscriber.clone()))
                    .collect::<Vec<_>>(),
                None => return 0,
            }
        };

        let msg = PubSubMessage {
            channel: channel.to_string(),
            message,
        };

        let mut delivered = 0;
        let mut disconnected_clients = Vec::new();

        for (client_id, subscriber) in subscribers {
            match subscriber.messages.try_send(msg.clone()) {
                Ok(()) => delivered += 1,
                Err(TrySendError::Full(_)) => {
                    let _ = subscriber.disconnect.send(());
                    disconnected_clients.push(client_id);
                }
                Err(TrySendError::Closed(_)) => disconnected_clients.push(client_id),
            }
        }

        if !disconnected_clients.is_empty() {
            let mut channels = self.channels.lock().unwrap();

            if let Some(subs) = channels.get_mut(channel) {
                for client_id in disconnected_clients {
                    subs.remove(&client_id);
                }

                if subs.is_empty() {
                    channels.remove(channel);
                }
            }
        }

        delivered
    }
}

impl Default for PubSub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn publish_removes_subscriber_when_queue_is_full() {
        let pubsub = PubSub::new();
        let client_id = pubsub.next_client_id();
        let (messages_tx, _messages_rx) = mpsc::channel(1);
        let (disconnect_tx, mut disconnect_rx) = mpsc::unbounded_channel();

        pubsub.subscribe(client_id, "news".into(), messages_tx, disconnect_tx);

        assert_eq!(pubsub.publish("news", "one".into()), 1);
        assert_eq!(pubsub.publish("news", "two".into()), 0);
        assert!(disconnect_rx.try_recv().is_ok());
        assert_eq!(pubsub.publish("news", "three".into()), 0);
    }
}
