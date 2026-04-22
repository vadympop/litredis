mod common;

use tokio::io::AsyncWriteExt;

// ── SET / GET ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn set_and_get() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET foo bar\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::simple("OK"));

    w.write_all(b"GET foo\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::bulk("bar"));
}

#[tokio::test]
async fn get_missing_key_returns_nil() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"GET nosuchkey\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::nil());
}

#[tokio::test]
async fn set_overwrites_existing_value() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET k v1\n").await.unwrap();
    common::read_reply(&mut r).await;
    w.write_all(b"SET k v2\n").await.unwrap();
    common::read_reply(&mut r).await;

    w.write_all(b"GET k\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::bulk("v2"));
}

#[tokio::test]
async fn set_value_with_spaces() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET msg \"hello world\"\n").await.unwrap();
    common::read_reply(&mut r).await;

    w.write_all(b"GET msg\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::bulk("hello world"));
}

// ── DEL ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn del_existing_key() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET x 1\n").await.unwrap();
    common::read_reply(&mut r).await;

    w.write_all(b"DEL x\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));

    w.write_all(b"GET x\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::nil());
}

#[tokio::test]
async fn del_missing_key_returns_zero() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"DEL ghost\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::integer(0));
}

// ── EXISTS ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn exists_present_key() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET e 1\n").await.unwrap();
    common::read_reply(&mut r).await;

    w.write_all(b"EXISTS e\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));
}

#[tokio::test]
async fn exists_missing_key() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"EXISTS ghost\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::integer(0));
}

#[tokio::test]
async fn exists_returns_zero_after_del() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET k v\n").await.unwrap();
    common::read_reply(&mut r).await;
    w.write_all(b"DEL k\n").await.unwrap();
    common::read_reply(&mut r).await;

    w.write_all(b"EXISTS k\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::integer(0));
}

// ── INCR / DECR ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn incr_missing_key_starts_at_one() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"INCR counter\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::integer(1));
}

#[tokio::test]
async fn incr_increments_sequentially() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    for expected in 1..=5 {
        w.write_all(b"INCR n\n").await.unwrap();
        assert_eq!(common::read_reply(&mut r).await, common::integer(expected));
    }
}

#[tokio::test]
async fn decr_decrements_sequentially() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET n 3\n").await.unwrap();
    common::read_reply(&mut r).await;

    for expected in [2, 1, 0, -1] {
        w.write_all(b"DECR n\n").await.unwrap();
        assert_eq!(common::read_reply(&mut r).await, common::integer(expected));
    }
}

#[tokio::test]
async fn incr_on_non_integer_returns_error() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET k notanumber\n").await.unwrap();
    common::read_reply(&mut r).await;

    w.write_all(b"INCR k\n").await.unwrap();
    assert!(common::read_reply(&mut r).await.starts_with("-ERR"));
}

#[tokio::test]
async fn incr_on_numeric_string() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(b"SET n 10\n").await.unwrap();
    common::read_reply(&mut r).await;

    w.write_all(b"INCR n\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r).await, common::integer(11));
}
