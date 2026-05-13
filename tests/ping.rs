mod common;

use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn ping_bare() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["PING"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::simple("PONG"));
}

#[tokio::test]
async fn ping_with_message() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["PING", "hello"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::bulk("hello"));
}

#[tokio::test]
async fn echo_plain() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["ECHO", "rust"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::bulk("rust"));
}

#[tokio::test]
async fn echo_quoted() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["ECHO", "hello world"]).await;
    assert_eq!(
        common::read_reply(&mut r).await,
        common::bulk("hello world")
    );
}

#[tokio::test]
async fn unknown_command_returns_error() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["BLAH"]).await;
    assert!(common::read_reply(&mut r).await.starts_with("-ERR"));
}

#[tokio::test]
async fn inline_command_is_rejected() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"PING\r\n").await.unwrap();
    assert!(common::read_reply(&mut r).await.starts_with("-ERR"));
}

#[tokio::test]
async fn multiple_commands_same_connection() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(&common::commands(&[&["PING"], &["PING"]]))
        .await
        .unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::simple("PONG"));
    assert_eq!(common::read_reply(&mut r).await, common::simple("PONG"));
}
