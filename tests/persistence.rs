mod common;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use redis_app::persistence;

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
    common::write_command(&mut w, &["SET", "foo", "bar"]).await;
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    common::write_command(&mut w2, &["GET", "foo"]).await;
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("bar"));

    std::fs::remove_file(&snapshot).ok();
}

#[tokio::test]
async fn multiple_keys_survive_restart() {
    let snapshot = tmp_snapshot();

    let (port, shared) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r, mut w) = common::connect(port).await;
    common::write_command(&mut w, &["SET", "k1", "v1"]).await;
    common::read_reply(&mut r).await;
    common::write_command(&mut w, &["SET", "k2", "v2"]).await;
    common::read_reply(&mut r).await;
    common::write_command(&mut w, &["INCR", "counter"]).await;
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    common::write_command(&mut w2, &["GET", "k1"]).await;
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("v1"));
    common::write_command(&mut w2, &["GET", "k2"]).await;
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("v2"));
    common::write_command(&mut w2, &["GET", "counter"]).await;
    assert_eq!(common::read_reply(&mut r2).await, common::bulk("1"));

    std::fs::remove_file(&snapshot).ok();
}

#[tokio::test]
async fn expired_key_not_restored_after_restart() {
    let snapshot = tmp_snapshot();

    let (port, shared) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r, mut w) = common::connect(port).await;
    common::write_command(&mut w, &["SET", "temp", "val"]).await;
    common::read_reply(&mut r).await;
    common::write_command(&mut w, &["EXPIRE", "temp", "1"]).await;
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    tokio::time::sleep(Duration::from_millis(1100)).await;

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    common::write_command(&mut w2, &["GET", "temp"]).await;
    assert_eq!(common::read_reply(&mut r2).await, common::nil());

    std::fs::remove_file(&snapshot).ok();
}

#[tokio::test]
async fn ttl_preserved_after_restart() {
    let snapshot = tmp_snapshot();

    let (port, shared) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r, mut w) = common::connect(port).await;
    common::write_command(&mut w, &["SET", "session", "abc"]).await;
    common::read_reply(&mut r).await;
    common::write_command(&mut w, &["EXPIRE", "session", "30"]).await;
    common::read_reply(&mut r).await;
    persistence::save(&shared.store, &snapshot).unwrap();
    drop(shared);

    let (port2, _) = common::spawn_with_snapshot(snapshot.clone()).await;
    let (mut r2, mut w2) = common::connect(port2).await;
    common::write_command(&mut w2, &["TTL", "session"]).await;
    let reply = common::read_reply(&mut r2).await;
    let ttl: i64 = reply.trim_start_matches(':').trim().parse().unwrap();
    assert!(ttl > 25 && ttl <= 30, "expected TTL ~30s, got {ttl}");

    std::fs::remove_file(&snapshot).ok();
}
