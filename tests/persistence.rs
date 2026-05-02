mod common;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use redis_app::persistence;
use tokio::io::AsyncWriteExt;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn tmp_snapshot() -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "{}/integ_snap_{}_{n}.json",
        std::env::temp_dir().display(),
        std::process::id()
    )
}

#[tokio::test]
async fn data_survives_restart() {
    let snapshot = tmp_snapshot();

    let (port, shared) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r, mut w) = common::connect(port).await;
    w.write_all(b"SET foo bar\n").await.unwrap();
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    w2.write_all(b"GET foo\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("bar"));

    std::fs::remove_file(&snapshot).ok();
}

#[tokio::test]
async fn multiple_keys_survive_restart() {
    let snapshot = tmp_snapshot();

    let (port, shared) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r, mut w) = common::connect(port).await;
    w.write_all(b"SET k1 v1\n").await.unwrap();
    common::read_reply(&mut r).await;
    w.write_all(b"SET k2 v2\n").await.unwrap();
    common::read_reply(&mut r).await;
    w.write_all(b"INCR counter\n").await.unwrap();
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    w2.write_all(b"GET k1\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("v1"));
    w2.write_all(b"GET k2\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("v2"));
    w2.write_all(b"GET counter\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("1"));

    std::fs::remove_file(&snapshot).ok();
}

#[tokio::test]
async fn expired_key_not_restored_after_restart() {
    let snapshot = tmp_snapshot();

    let (port, shared) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r, mut w) = common::connect(port).await;
    w.write_all(b"SET temp val\n").await.unwrap();
    common::read_reply(&mut r).await;
    w.write_all(b"EXPIRE temp 1\n").await.unwrap();
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    tokio::time::sleep(Duration::from_millis(1100)).await;

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    w2.write_all(b"GET temp\n").await.unwrap();
    assert_eq!(common::read_reply(&mut r2).await, common::nil());

    std::fs::remove_file(&snapshot).ok();
}

#[tokio::test]
async fn ttl_preserved_after_restart() {
    let snapshot = tmp_snapshot();

    let (port, shared) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r, mut w) = common::connect(port).await;
    w.write_all(b"SET session abc\n").await.unwrap();
    common::read_reply(&mut r).await;
    w.write_all(b"EXPIRE session 30\n").await.unwrap();
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    w2.write_all(b"TTL session\n").await.unwrap();
    let reply = common::read_reply(&mut r2).await;
    let ttl: i64 = reply.trim_start_matches(':').trim().parse().unwrap();
    assert!(ttl > 25 && ttl <= 30, "expected TTL ~30s, got {ttl}");

    std::fs::remove_file(&snapshot).ok();
}
