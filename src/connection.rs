use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::commands;
use crate::protocol::text::{encode_reply, parse_command};
use crate::protocol::{Command, Reply};
use crate::pubsub::PubSubMessage;
use crate::server::Shared;

pub async fn handle_connection(stream: TcpStream, shared: Arc<Shared>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();

    let client_id = shared.pubsub.next_client_id();
    let (pubsub_tx, mut pubsub_rx) = mpsc::unbounded_channel::<PubSubMessage>();
    let mut subscribed_channels = HashSet::<String>::new();

    let mut reader = BufReader::new(reader);
    let mut read_buf = Vec::new();

    loop {
        tokio::select! {
            read = reader.read_until(b'\n', &mut read_buf) => {
                let n = read?;
                if n == 0 {
                    break; // client disconnected
                }

                let mut close_after_reply = false;
                let replies = match std::str::from_utf8(&read_buf) {
                    Err(e) => vec![Reply::Error(e.to_string())],
                    Ok(line) => match parse_command(line) {
                        Err(e) => vec![Reply::Error(e.to_string())],
                        Ok(cmd) => match cmd {
                            Command::Subscribe { channels } => channels
                                .into_iter()
                                .map(|channel| {
                                    subscribed_channels.insert(channel.clone());
                                    shared.pubsub.subscribe(
                                        client_id,
                                        channel.clone(),
                                        pubsub_tx.clone(),
                                    );

                                    Reply::Array(vec![
                                        Reply::Bulk("subscribe".into()),
                                        Reply::Bulk(channel),
                                        Reply::Integer(subscribed_channels.len() as i64),
                                    ])
                                })
                                .collect(),
                            Command::Unsubscribe { channels } => {
                                let channels_to_remove = if channels.is_empty() {
                                    subscribed_channels.iter().cloned().collect::<Vec<_>>()
                                } else {
                                    channels
                                };

                                if channels_to_remove.is_empty() {
                                    vec![Reply::Array(vec![
                                        Reply::Bulk("unsubscribe".into()),
                                        Reply::Nil,
                                        Reply::Integer(0),
                                    ])]
                                } else {
                                    channels_to_remove
                                        .into_iter()
                                        .map(|channel| {
                                            subscribed_channels.remove(&channel);
                                            shared.pubsub.unsubscribe(client_id, &channel);

                                            Reply::Array(vec![
                                                Reply::Bulk("unsubscribe".into()),
                                                Reply::Bulk(channel),
                                                Reply::Integer(subscribed_channels.len() as i64),
                                            ])
                                        })
                                        .collect()
                                }
                            }
                            Command::Ping(msg) if !subscribed_channels.is_empty() => {
                                vec![Reply::Array(vec![
                                    Reply::Bulk("pong".into()),
                                    Reply::Bulk(msg.unwrap_or_default()),
                                ])]
                            }
                            Command::Quit => {
                                shared.pubsub.unsubscribe_all(client_id);
                                close_after_reply = true;

                                vec![Reply::Simple("OK".into())]
                            }
                            Command::Reset => {
                                shared.pubsub.unsubscribe_all(client_id);
                                subscribed_channels.clear();

                                vec![Reply::Simple("RESET".into())]
                            }
                            other => {
                                if !subscribed_channels.is_empty() {
                                    vec![Reply::Error(
                                        "Can't execute command in subscribed mode".into(),
                                    )]
                                } else {
                                    vec![commands::execute(other, &shared)]
                                }
                            }
                        },
                    },
                };

                let response = replies.iter().map(encode_reply).collect::<String>();
                writer.write_all(response.as_bytes()).await?;
                read_buf.clear();

                if close_after_reply {
                    break;
                }
            }
            Some(message) = pubsub_rx.recv() => {
                let reply = Reply::Array(vec![
                    Reply::Bulk("message".into()),
                    Reply::Bulk(message.channel),
                    Reply::Bulk(message.message),
                ]);

                writer.write_all(encode_reply(&reply).as_bytes()).await?;
            }
        }
    }

    shared.pubsub.unsubscribe_all(client_id);

    Ok(())
}
