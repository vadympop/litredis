use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::protocol::text::{encode_reply, parse_command, Command, Reply};
use crate::server::Shared;

pub async fn handle_connection(stream: TcpStream, _shared: Arc<Shared>) -> Result<()> {
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
            Ok(cmd) => dispatch(cmd),
            Err(e) => Reply::Error(e.to_string()),
        };

        writer.write_all(encode_reply(&reply).as_bytes()).await?;
    }

    Ok(())
}

fn dispatch(cmd: Command) -> Reply {
    match cmd {
        Command::Ping(None) => Reply::Simple("PONG".into()),
        Command::Ping(Some(msg)) => Reply::Bulk(msg),
        Command::Echo(msg) => Reply::Bulk(msg),
    }
}
