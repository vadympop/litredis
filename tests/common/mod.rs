#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use redis_app::{
    config::Config,
    protocol::text::encode_command,
    server::{Shared, connections_loop},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

pub const LINE_ENDING: &str = "\r\n";

// ── RespValue builders (mirror encode_resp_value so tests stay in sync with the protocol) ──

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

// ── Request builders ─────────────────────────────────────────────────────────

pub fn command(args: &[&str]) -> Vec<u8> {
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    encode_command(&args)
}

pub fn commands(commands: &[&[&str]]) -> Vec<u8> {
    let mut encoded = Vec::new();
    for args in commands {
        encoded.extend(command(args));
    }
    encoded
}

pub async fn write_command(writer: &mut (impl AsyncWrite + Unpin), args: &[&str]) {
    writer.write_all(&command(args)).await.unwrap();
}

// ── Server / connection helpers ───────────────────────────────────────────────

pub async fn spawn_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Config {
        port,
        host: "127.0.0.1".into(),
        snapshot_path: None,
        flush_interval: 300,
        password: None,
    };
    tokio::spawn(connections_loop(listener, Shared::create(config)));
    tokio::time::sleep(Duration::from_millis(10)).await;
    port
}

pub async fn spawn_server_with_password(password: &str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Config {
        port,
        host: "127.0.0.1".into(),
        snapshot_path: None,
        flush_interval: 300,
        password: Some(password.to_owned()),
    };
    tokio::spawn(connections_loop(listener, Shared::create(config)));
    tokio::time::sleep(Duration::from_millis(10)).await;
    port
}

pub async fn spawn_with_snapshot(snapshot: String) -> (u16, Arc<Shared>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Config {
        port,
        host: "127.0.0.1".into(),
        snapshot_path: Some(snapshot),
        flush_interval: 3600,
        password: None,
    };
    let shared = Shared::create(config);
    tokio::spawn(connections_loop(listener, shared.clone()));
    tokio::time::sleep(Duration::from_millis(10)).await;
    (port, shared)
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
