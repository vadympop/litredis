use std::{path::PathBuf, time::Duration};

use redis_app::{
    config::Config,
    server::{Shared, connections_loop},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

async fn spawn_server() -> u16 {
    let test_host = "127.0.0.1".to_string(); 
    let listener = TcpListener::bind(format!("{}:0", test_host)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Config {
        port,
        host: test_host,
        snapshot_path: PathBuf::from("dump.json"),
        flush_interval: 300,
        password: None,
    };
    tokio::spawn(connections_loop(listener, Shared::create(config)));
    // Yield so the spawned task can start accepting before we connect.
    tokio::time::sleep(Duration::from_millis(10)).await;
    port
}

async fn connect(port: u16) -> (impl AsyncBufReadExt + Unpin, impl AsyncWriteExt + Unpin) {
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let (r, w) = stream.into_split();
    (BufReader::new(r), w)
}

#[tokio::test]
async fn ping_bare() {
    let port = spawn_server().await;
    let (mut r, mut w) = connect(port).await;

    w.write_all(b"PING\n").await.unwrap();

    let mut line = String::new();
    r.read_line(&mut line).await.unwrap();
    assert_eq!(line, "+PONG\r\n");
}

#[tokio::test]
async fn ping_with_message() {
    let port = spawn_server().await;
    let (mut r, mut w) = connect(port).await;

    w.write_all(b"PING hello\n").await.unwrap();

    let mut header = String::new();
    let mut body = String::new();
    r.read_line(&mut header).await.unwrap();
    r.read_line(&mut body).await.unwrap();
    assert_eq!(header, "$5\r\n");
    assert_eq!(body, "hello\r\n");
}

#[tokio::test]
async fn echo_plain() {
    let port = spawn_server().await;
    let (mut r, mut w) = connect(port).await;

    w.write_all(b"ECHO rust\n").await.unwrap();

    let mut header = String::new();
    let mut body = String::new();
    r.read_line(&mut header).await.unwrap();
    r.read_line(&mut body).await.unwrap();
    assert_eq!(header, "$4\r\n");
    assert_eq!(body, "rust\r\n");
}

#[tokio::test]
async fn echo_quoted() {
    let port = spawn_server().await;
    let (mut r, mut w) = connect(port).await;

    w.write_all(b"ECHO \"hello world\"\n").await.unwrap();

    let mut header = String::new();
    let mut body = String::new();
    r.read_line(&mut header).await.unwrap();
    r.read_line(&mut body).await.unwrap();
    assert_eq!(header, "$11\r\n");
    assert_eq!(body, "hello world\r\n");
}

#[tokio::test]
async fn unknown_command_returns_error() {
    let port = spawn_server().await;
    let (mut r, mut w) = connect(port).await;

    w.write_all(b"BLAH\n").await.unwrap();

    let mut line = String::new();
    r.read_line(&mut line).await.unwrap();
    assert!(line.starts_with("-ERR"));
}

#[tokio::test]
async fn multiple_commands_same_connection() {
    let port = spawn_server().await;
    let (mut r, mut w) = connect(port).await;

    w.write_all(b"PING\nPING\n").await.unwrap();

    let mut l1 = String::new();
    let mut l2 = String::new();
    r.read_line(&mut l1).await.unwrap();
    r.read_line(&mut l2).await.unwrap();
    assert_eq!(l1, "+PONG\r\n");
    assert_eq!(l2, "+PONG\r\n");
}
