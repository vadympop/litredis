use std::{collections::HashSet, sync::Arc};

use tokio::sync::mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender};

use crate::commands;
use crate::commands::misc;
use crate::protocol::{Command, NormalCommand, Reply, SessionCommand};
use crate::pubsub::{ClientId, PUBSUB_BUFFER_SIZE, PubSubMessage};
use crate::server::Shared;

pub struct ClientSession {
    is_authed: bool,
    client_id: ClientId,
    pubsub_tx: Sender<PubSubMessage>,
    slow_consumer_tx: UnboundedSender<()>,
    subscribed_channels: HashSet<String>,
}

pub struct CommandOutcome {
    pub replies: Vec<Reply>,
    pub close_connection: bool,
}

impl CommandOutcome {
    pub fn single(reply: Reply) -> Self {
        Self {
            replies: vec![reply],
            close_connection: false,
        }
    }

    pub fn close(reply: Reply) -> Self {
        Self {
            replies: vec![reply],
            close_connection: true,
        }
    }
}

impl ClientSession {
    pub fn new(shared: &Arc<Shared>) -> (Self, Receiver<PubSubMessage>, UnboundedReceiver<()>) {
        let client_id = shared.pubsub.next_client_id();
        let (pubsub_tx, pubsub_rx) = mpsc::channel(PUBSUB_BUFFER_SIZE);
        let (slow_consumer_tx, slow_consumer_rx) = mpsc::unbounded_channel();

        (
            Self {
                client_id,
                pubsub_tx,
                slow_consumer_tx,
                is_authed: shared.config.password.is_none(),
                subscribed_channels: HashSet::new(),
            },
            pubsub_rx,
            slow_consumer_rx,
        )
    }

    pub fn execute(&mut self, cmd: Command, shared: &Arc<Shared>) -> CommandOutcome {
        match cmd {
            Command::Auth { password } => self.execute_auth(password, shared),
            Command::Normal(NormalCommand::Ping(msg)) if !self.is_authed => {
                CommandOutcome::single(misc::ping(msg))
            }

            _ if !self.is_authed => {
                CommandOutcome::single(Reply::Error("NOAUTH Authentication required".into()))
            }

            Command::Normal(NormalCommand::Ping(msg)) if self.is_subscribed() => {
                CommandOutcome::single(subscribed_pong_reply(msg))
            }
            Command::Session(cmd) => self.execute_session(cmd, shared),
            Command::Normal(cmd) => {
                if self.is_subscribed() {
                    CommandOutcome::single(Reply::Error(
                        "Can't execute command in subscribed mode".into(),
                    ))
                } else {
                    CommandOutcome::single(commands::execute(cmd, shared))
                }
            }
        }
    }

    fn execute_auth(&mut self, password: String, shared: &Arc<Shared>) -> CommandOutcome {
        match &shared.config.password {
            None => {
                // Server has no password
                CommandOutcome::single(Reply::Error(
                    "ERR Client sent AUTH but no password is set".into(),
                ))
            }
            Some(expected) if *expected == password => {
                self.is_authed = true;
                log::debug!("client authenticated successfully");
                CommandOutcome::single(Reply::Simple("OK".into()))
            }
            Some(_) => {
                log::warn!("failed authentication attempt");
                CommandOutcome::single(Reply::Error("WRONGPASS invalid password".into()))
            }
        }
    }

    pub fn cleanup(&self, shared: &Arc<Shared>) {
        shared.pubsub.unsubscribe_all(self.client_id);
    }

    pub fn pubsub_message_reply(message: PubSubMessage) -> Reply {
        Reply::Array(vec![
            Reply::Bulk("message".into()),
            Reply::Bulk(message.channel),
            Reply::Bulk(message.message),
        ])
    }

    fn execute_session(&mut self, cmd: SessionCommand, shared: &Arc<Shared>) -> CommandOutcome {
        match cmd {
            SessionCommand::Subscribe { channels } => {
                let replies = channels
                    .into_iter()
                    .map(|channel| self.subscribe(channel, shared))
                    .collect();

                CommandOutcome {
                    replies,
                    close_connection: false,
                }
            }
            SessionCommand::Unsubscribe { channels } => {
                let replies = self.unsubscribe(channels, shared);

                CommandOutcome {
                    replies,
                    close_connection: false,
                }
            }
            SessionCommand::Quit => {
                self.cleanup(shared);
                CommandOutcome::close(Reply::Simple("OK".into()))
            }
            SessionCommand::Reset => {
                self.cleanup(shared);
                self.subscribed_channels.clear();

                CommandOutcome::single(Reply::Simple("RESET".into()))
            }
        }
    }

    fn subscribe(&mut self, channel: String, shared: &Arc<Shared>) -> Reply {
        self.subscribed_channels.insert(channel.clone());
        shared.pubsub.subscribe(
            self.client_id,
            channel.clone(),
            self.pubsub_tx.clone(),
            self.slow_consumer_tx.clone(),
        );

        subscribe_reply(channel, self.subscribed_channels.len())
    }

    fn unsubscribe(&mut self, channels: Vec<String>, shared: &Arc<Shared>) -> Vec<Reply> {
        let channels_to_remove = if channels.is_empty() {
            self.subscribed_channels.iter().cloned().collect::<Vec<_>>()
        } else {
            channels
        };

        if channels_to_remove.is_empty() {
            return vec![unsubscribe_reply(None, 0)];
        }

        channels_to_remove
            .into_iter()
            .map(|channel| {
                self.subscribed_channels.remove(&channel);
                shared.pubsub.unsubscribe(self.client_id, &channel);

                unsubscribe_reply(Some(channel), self.subscribed_channels.len())
            })
            .collect()
    }

    fn is_subscribed(&self) -> bool {
        !self.subscribed_channels.is_empty()
    }
}

fn subscribe_reply(channel: String, count: usize) -> Reply {
    Reply::Array(vec![
        Reply::Bulk("subscribe".into()),
        Reply::Bulk(channel),
        Reply::Integer(count as i64),
    ])
}

fn unsubscribe_reply(channel: Option<String>, count: usize) -> Reply {
    Reply::Array(vec![
        Reply::Bulk("unsubscribe".into()),
        channel.map(Reply::Bulk).unwrap_or(Reply::Nil),
        Reply::Integer(count as i64),
    ])
}

fn subscribed_pong_reply(message: Option<String>) -> Reply {
    Reply::Array(vec![
        Reply::Bulk("pong".into()),
        Reply::Bulk(message.unwrap_or_default()),
    ])
}
