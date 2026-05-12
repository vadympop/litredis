use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::protocol::text::{encode_resp_value, parse_command};
use crate::protocol::{Command, RespValue};
use crate::server::Shared;
use crate::session::{ClientSession, CommandOutcome};

pub async fn handle_connection(stream: TcpStream, shared: Arc<Shared>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut read_buf = Vec::new();
    let (mut session, mut pubsub_rx, mut slow_consumer_rx) = ClientSession::new(&shared);

    let result = loop {
        tokio::select! {
            biased;

            Some(()) = slow_consumer_rx.recv() => break Ok(()),

            read = reader.read_until(b'\n', &mut read_buf) => {
                let n = match read {
                    Ok(n) => n,
                    Err(e) => break Err(e.into()),
                };

                if n == 0 {
                    break Ok(()); // client disconnected
                }

                let outcome = match parse_line(&read_buf) {
                    Ok(cmd) => session.execute(cmd, &shared),
                    Err(reply) => CommandOutcome::single(reply),
                };

                let response = encode_resp_values(&outcome.replies);
                if let Err(e) = writer.write_all(response.as_bytes()).await {
                    break Err(e.into());
                }

                read_buf.clear();

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

    result
}

fn parse_line(read_buf: &[u8]) -> Result<Command, RespValue> {
    let line = std::str::from_utf8(read_buf).map_err(|e| RespValue::Error(e.to_string()))?;
    parse_command(line).map_err(|e| RespValue::Error(e.to_string()))
}

fn encode_resp_values(replies: &[RespValue]) -> String {
    replies.iter().map(encode_resp_value).collect()
}
