use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::protocol::text::{command_from_resp_value, encode_resp_value, read_resp_value};
use crate::protocol::{Command, RespValue};
use crate::server::Shared;
use crate::session::{ClientSession, CommandOutcome};

pub async fn handle_connection(stream: TcpStream, shared: Arc<Shared>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let (command_tx, mut command_rx) = mpsc::channel(16);
    let reader_task = tokio::spawn(read_commands(BufReader::new(reader), command_tx));
    let (mut session, mut pubsub_rx, mut slow_consumer_rx) = ClientSession::new(&shared);

    let result = loop {
        tokio::select! {
            biased;

            Some(()) = slow_consumer_rx.recv() => break Ok(()),

            read = command_rx.recv() => {
                let Some(read) = read else {
                    break Ok(());
                };

                let outcome = match read {
                    Ok(cmd) => session.execute(cmd, &shared),
                    Err(ReadError { reply, close_connection }) => CommandOutcome {
                        replies: vec![reply],
                        close_connection,
                    },
                };

                let response = encode_resp_values(&outcome.replies);
                if let Err(e) = writer.write_all(response.as_bytes()).await {
                    break Err(e.into());
                }

                if outcome.close_connection {
                    break Ok(());
                }
            }
            Some(message) = pubsub_rx.recv() => {
                let reply = ClientSession::pubsub_message_reply(message);

                if let Err(e) = writer.write_all(encode_resp_value(&reply).as_bytes()).await {
                    break Err(e.into());
                }
            }
        }
    };

    session.cleanup(&shared);
    reader_task.abort();

    result
}

struct ReadError {
    reply: RespValue,
    close_connection: bool,
}

async fn read_commands<R>(mut reader: R, command_tx: mpsc::Sender<Result<Command, ReadError>>)
where
    R: tokio::io::AsyncBufRead + tokio::io::AsyncRead + Unpin,
{
    loop {
        let read = match read_resp_value(&mut reader).await {
            Ok(value) => command_from_resp_value(value).map_err(|e| ReadError {
                reply: RespValue::Error(e.to_string()),
                close_connection: false,
            }),
            Err(e) => {
                let message = e.to_string();
                if message.contains("unexpected EOF") || message.contains("early eof") {
                    break;
                }

                Err(ReadError {
                    reply: RespValue::Error(message),
                    close_connection: true,
                })
            }
        };

        let close_connection = read
            .as_ref()
            .err()
            .map(|err| err.close_connection)
            .unwrap_or(false);
        if command_tx.send(read).await.is_err() || close_connection {
            break;
        }
    }
}

fn encode_resp_values(replies: &[RespValue]) -> String {
    replies.iter().map(encode_resp_value).collect()
}
