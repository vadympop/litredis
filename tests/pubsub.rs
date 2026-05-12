mod common;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};

async fn read_lines(r: &mut (impl AsyncBufReadExt + Unpin), count: usize) -> String {
    let mut out = String::new();

    for _ in 0..count {
        let mut line = String::new();
        r.read_line(&mut line).await.unwrap();
        out.push_str(&line);
    }

    out
}

fn array_subscribe(channel: &str, count: i64) -> String {
    format!(
        "*3{0}$9{0}subscribe{0}${1}{0}{2}{0}:{3}{0}",
        common::LINE_ENDING,
        channel.len(),
        channel,
        count
    )
}

fn array_unsubscribe(channel: &str, count: i64) -> String {
    format!(
        "*3{0}$11{0}unsubscribe{0}${1}{0}{2}{0}:{3}{0}",
        common::LINE_ENDING,
        channel.len(),
        channel,
        count
    )
}

fn array_message(channel: &str, message: &str) -> String {
    format!(
        "*3{0}$7{0}message{0}${1}{0}{2}{0}${3}{0}{4}{0}",
        common::LINE_ENDING,
        channel.len(),
        channel,
        message.len(),
        message
    )
}

#[tokio::test]
async fn subscribe_and_publish_delivers_message() {
    let port = common::spawn_server().await;
    let (mut sub_r, mut sub_w) = common::connect(port).await;
    let (mut pub_r, mut pub_w) = common::connect(port).await;

    common::write_command(&mut sub_w, &["SUBSCRIBE", "news"]).await;
    assert_eq!(read_lines(&mut sub_r, 6).await, array_subscribe("news", 1));

    common::write_command(&mut pub_w, &["PUBLISH", "news", "hello"]).await;
    assert_eq!(common::read_reply(&mut pub_r).await, common::integer(1));
    assert_eq!(
        read_lines(&mut sub_r, 7).await,
        array_message("news", "hello")
    );
}

#[tokio::test]
async fn unsubscribe_stops_future_messages() {
    let port = common::spawn_server().await;
    let (mut sub_r, mut sub_w) = common::connect(port).await;
    let (mut pub_r, mut pub_w) = common::connect(port).await;

    common::write_command(&mut sub_w, &["SUBSCRIBE", "news"]).await;
    read_lines(&mut sub_r, 6).await;

    common::write_command(&mut sub_w, &["UNSUBSCRIBE", "news"]).await;
    assert_eq!(
        read_lines(&mut sub_r, 6).await,
        array_unsubscribe("news", 0)
    );

    common::write_command(&mut pub_w, &["PUBLISH", "news", "hello"]).await;
    assert_eq!(common::read_reply(&mut pub_r).await, common::integer(0));

    let no_message = timeout(Duration::from_millis(100), read_lines(&mut sub_r, 1)).await;
    assert!(no_message.is_err());
}

#[tokio::test]
async fn subscribed_mode_rejects_kv_commands_until_unsubscribed() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "key", "value"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::simple("OK"));

    common::write_command(&mut w, &["SUBSCRIBE", "news"]).await;
    assert_eq!(read_lines(&mut r, 6).await, array_subscribe("news", 1));

    common::write_command(&mut w, &["GET", "key"]).await;
    assert!(common::read_reply(&mut r).await.starts_with("-ERR"));

    common::write_command(&mut w, &["UNSUBSCRIBE", "news"]).await;
    assert_eq!(read_lines(&mut r, 6).await, array_unsubscribe("news", 0));

    common::write_command(&mut w, &["GET", "key"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::bulk("value"));
}

#[tokio::test]
async fn pubsub_message_does_not_drop_partial_client_command() {
    let port = common::spawn_server().await;
    let (mut sub_r, mut sub_w) = common::connect(port).await;
    let (mut pub_r, mut pub_w) = common::connect(port).await;

    common::write_command(&mut sub_w, &["SUBSCRIBE", "news"]).await;
    assert_eq!(read_lines(&mut sub_r, 6).await, array_subscribe("news", 1));

    sub_w.write_all(b"*2\r\n$4\r\nPIN").await.unwrap();

    common::write_command(&mut pub_w, &["PUBLISH", "news", "hello"]).await;
    assert_eq!(common::read_reply(&mut pub_r).await, common::integer(1));
    assert_eq!(
        read_lines(&mut sub_r, 7).await,
        array_message("news", "hello")
    );

    sub_w.write_all(b"G\r\n$2\r\nhi\r\n").await.unwrap();
    assert_eq!(
        read_lines(&mut sub_r, 5).await,
        format!("*2{0}$4{0}pong{0}$2{0}hi{0}", common::LINE_ENDING)
    );
}
