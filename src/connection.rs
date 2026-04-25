use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::protocol::text::{encode_reply, parse_command};
use crate::protocol::{Command, Reply};
use crate::server::Shared;
use crate::session::{ClientSession, CommandOutcome};

pub async fn handle_connection(stream: TcpStream, shared: Arc<Shared>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut read_buf = Vec::new();
    let (mut session, mut pubsub_rx) = ClientSession::new(&shared);

    loop {
        tokio::select! {
            read = reader.read_until(b'\n', &mut read_buf) => {
                let n = read?;
                if n == 0 {
                    break; // client disconnected
                }

                let outcome = match parse_line(&read_buf) {
                    Ok(cmd) => session.execute(cmd, &shared),
                    Err(reply) => CommandOutcome::single(reply),
                };

                let response = encode_replies(&outcome.replies);
                writer.write_all(response.as_bytes()).await?;
                read_buf.clear();

                if outcome.close_connection {
                    break;
                }
            }
            Some(message) = pubsub_rx.recv() => {
                let reply = ClientSession::pubsub_message_reply(message);

                writer.write_all(encode_reply(&reply).as_bytes()).await?;
            }
        }
    }

    session.cleanup(&shared);

    Ok(())
}

fn parse_line(read_buf: &[u8]) -> Result<Command, Reply> {
    let line = std::str::from_utf8(read_buf).map_err(|e| Reply::Error(e.to_string()))?;
    parse_command(line).map_err(|e| Reply::Error(e.to_string()))
}

fn encode_replies(replies: &[Reply]) -> String {
    replies.iter().map(encode_reply).collect()
}
