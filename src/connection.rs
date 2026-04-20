use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::commands;
use crate::protocol::text::{Reply, encode_reply, parse_command};
use crate::server::Shared;

pub async fn handle_connection(stream: TcpStream, shared: Arc<Shared>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // client disconnected
        }

        let reply = match parse_command(&line) {
            Ok(cmd) => commands::execute(cmd, &shared),
            Err(e) => Reply::Error(e.to_string()),
        };

        writer.write_all(encode_reply(&reply).as_bytes()).await?;
    }

    Ok(())
}
