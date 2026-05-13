mod common;

use std::time::Duration;

// ── SET / GET ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn set_and_get() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "foo", "bar"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::simple("OK"));

    common::write_command(&mut w, &["GET", "foo"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::bulk("bar"));
}

#[tokio::test]
async fn get_missing_key_returns_nil() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["GET", "nosuchkey"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::nil());
}

#[tokio::test]
async fn set_overwrites_existing_value() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "k", "v1"]).await;
    common::read_reply(&mut r).await;
    common::write_command(&mut w, &["SET", "k", "v2"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["GET", "k"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::bulk("v2"));
}

#[tokio::test]
async fn set_value_with_spaces() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "msg", "hello world"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["GET", "msg"]).await;
    assert_eq!(
        common::read_reply(&mut r).await,
        common::bulk("hello world")
    );
}

// ── DEL ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn del_existing_key() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "x", "1"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["DEL", "x"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));

    common::write_command(&mut w, &["GET", "x"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::nil());
}

#[tokio::test]
async fn del_missing_key_returns_zero() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["DEL", "ghost"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(0));
}

// ── EXISTS ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn exists_present_key() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "e", "1"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["EXISTS", "e"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));
}

#[tokio::test]
async fn exists_missing_key() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["EXISTS", "ghost"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(0));
}

#[tokio::test]
async fn exists_returns_zero_after_del() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "k", "v"]).await;
    common::read_reply(&mut r).await;
    common::write_command(&mut w, &["DEL", "k"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["EXISTS", "k"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(0));
}

// ── INCR / DECR ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn incr_missing_key_starts_at_one() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["INCR", "counter"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));
}

#[tokio::test]
async fn incr_increments_sequentially() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    for expected in 1..=5 {
        common::write_command(&mut w, &["INCR", "n"]).await;
        assert_eq!(common::read_reply(&mut r).await, common::integer(expected));
    }
}

#[tokio::test]
async fn decr_decrements_sequentially() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "n", "3"]).await;
    common::read_reply(&mut r).await;

    for expected in [2, 1, 0, -1] {
        common::write_command(&mut w, &["DECR", "n"]).await;
        assert_eq!(common::read_reply(&mut r).await, common::integer(expected));
    }
}

#[tokio::test]
async fn incr_on_non_integer_returns_error() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "k", "notanumber"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["INCR", "k"]).await;
    assert!(common::read_reply(&mut r).await.starts_with("-ERR"));
}

#[tokio::test]
async fn incr_on_numeric_string() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "n", "10"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["INCR", "n"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(11));
}

// ── EXPIRE ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn expire_existing_key_returns_one() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "session", "abc"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["EXPIRE", "session", "10"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));
}

#[tokio::test]
async fn expire_missing_key_returns_zero() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["EXPIRE", "ghost", "10"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(0));
}

#[tokio::test]
async fn expire_removes_key_after_timeout() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "temp", "value"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["EXPIRE", "temp", "1"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));

    tokio::time::sleep(Duration::from_millis(1200)).await;

    common::write_command(&mut w, &["GET", "temp"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::nil());
}

// ── TTL ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn ttl_missing_key_returns_minus_two() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["TTL", "ghost"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(-2));
}

#[tokio::test]
async fn ttl_key_without_expiry_returns_minus_one() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "persistent", "value"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["TTL", "persistent"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(-1));
}

#[tokio::test]
async fn ttl_expired_key_returns_minus_two() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "short", "value"]).await;
    common::read_reply(&mut r).await;

    common::write_command(&mut w, &["EXPIRE", "short", "1"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));

    tokio::time::sleep(Duration::from_millis(1200)).await;

    common::write_command(&mut w, &["TTL", "short"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::integer(-2));
}
