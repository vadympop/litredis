use std::{path::PathBuf, time::Duration};

use redis_app::{
    config::Config,
    server::{Shared, connections_loop},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

pub async fn spawn_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Config {
        port,
        host: "127.0.0.1".into(),
        snapshot_path: PathBuf::from("dump.json"),
        flush_interval: 300,
        password: None,
    };
    tokio::spawn(connections_loop(listener, Shared::create(config)));
    tokio::time::sleep(Duration::from_millis(10)).await;
    port
}

pub async fn connect(port: u16) -> (impl AsyncBufReadExt + Unpin, impl AsyncWriteExt + Unpin) {
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let (r, w) = stream.into_split();
    (BufReader::new(r), w)
}

/// Reads a full reply from the server.
/// For bulk strings reads both the header and body line and returns them joined.
pub async fn read_reply(r: &mut (impl AsyncBufReadExt + Unpin)) -> String {
    let mut line = String::new();
    r.read_line(&mut line).await.unwrap();
    if line.starts_with('$') && !line.starts_with("$-") {
        let mut body = String::new();
        r.read_line(&mut body).await.unwrap();
        return format!("{}{}", line, body);
    }
    line
}
