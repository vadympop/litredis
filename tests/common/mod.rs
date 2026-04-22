#![allow(dead_code)]

use std::{path::PathBuf, time::Duration};

use redis_app::{
    config::Config,
    server::{Shared, connections_loop},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

#[cfg(windows)]
pub const LINE_ENDING: &str = "\r\n";
#[cfg(not(windows))]
pub const LINE_ENDING: &str = "\n";

// ── Reply builders (mirror encode_reply so tests stay in sync with the protocol) ──

pub fn simple(s: &str) -> String {
    format!("+{}{}", s, LINE_ENDING)
}

pub fn error(s: &str) -> String {
    format!("-ERR {}{}", s, LINE_ENDING)
}

pub fn integer(n: i64) -> String {
    format!(":{}{}", n, LINE_ENDING)
}

pub fn bulk(s: &str) -> String {
    format!("${}{}{}{}", s.len(), LINE_ENDING, s, LINE_ENDING)
}

pub fn nil() -> String {
    format!("$-1{}", LINE_ENDING)
}

// ── Server / connection helpers ───────────────────────────────────────────────

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

/// Reads a full reply. For bulk strings joins the header and body lines.
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
