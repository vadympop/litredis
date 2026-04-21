mod common;

use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn ping_bare() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"PING\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, "+PONG\r\n");
}

#[tokio::test]
async fn ping_with_message() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"PING hello\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, "$5\r\nhello\r\n");
}

#[tokio::test]
async fn echo_plain() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"ECHO rust\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, "$4\r\nrust\r\n");
}

#[tokio::test]
async fn echo_quoted() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"ECHO \"hello world\"\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, "$11\r\nhello world\r\n");
}

#[tokio::test]
async fn unknown_command_returns_error() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"BLAH\n").await.unwrap();
    assert!(common::read_reply(&mut r).await.starts_with("-ERR"));
}

#[tokio::test]
async fn multiple_commands_same_connection() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"PING\nPING\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, "+PONG\r\n");
    assert_eq!(common::read_reply(&mut r).await, "+PONG\r\n");
}
