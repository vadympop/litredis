mod common;

use tokio::io::AsyncWriteExt;

// ── helpers ──────────────────────────────────────────────────────────────────

async fn guarded_server() -> u16 {
    common::spawn_server_with_password("secret").await
}

// ── unauthenticated behaviour ─────────────────────────────────────────────────

#[tokio::test]
async fn ping_allowed_before_auth() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["PING"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::simple("PONG"));
}

#[tokio::test]
async fn ping_with_message_allowed_before_auth() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["PING", "hello"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::bulk("hello"));
}

#[tokio::test]
async fn command_blocked_before_auth() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["SET", "foo", "bar"]).await;
    let reply = common::read_reply(&mut r).await;
    assert!(reply.contains("NOAUTH"), "expected NOAUTH, got: {reply}");
}

#[tokio::test]
async fn get_blocked_before_auth() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["GET", "foo"]).await;
    let reply = common::read_reply(&mut r).await;
    assert!(reply.contains("NOAUTH"), "expected NOAUTH, got: {reply}");
}

// ── wrong password ────────────────────────────────────────────────────────────

#[tokio::test]
async fn wrong_password_rejected() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["AUTH", "wrongpass"]).await;
    let reply = common::read_reply(&mut r).await;
    assert!(reply.starts_with("-ERR"), "expected error, got: {reply}");
    assert!(
        reply.contains("WRONGPASS"),
        "expected WRONGPASS, got: {reply}"
    );
}

#[tokio::test]
async fn wrong_password_does_not_unlock() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(&common::commands(&[
        &["AUTH", "wrongpass"],
        &["SET", "foo", "bar"],
    ]))
    .await
    .unwrap();
    let _auth_reply = common::read_reply(&mut r).await;
    let cmd_reply = common::read_reply(&mut r).await;
    assert!(
        cmd_reply.contains("NOAUTH"),
        "expected NOAUTH after bad AUTH, got: {cmd_reply}"
    );
}

// ── correct password ──────────────────────────────────────────────────────────

#[tokio::test]
async fn correct_password_accepted() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["AUTH", "secret"]).await;
    assert_eq!(common::read_reply(&mut r).await, common::simple("OK"));
}

#[tokio::test]
async fn commands_work_after_auth() {
    let port = guarded_server().await;
    let (mut r, mut w) = common::connect(port).await;

    w.write_all(&common::commands(&[
        &["AUTH", "secret"],
        &["SET", "foo", "bar"],
        &["GET", "foo"],
    ]))
    .await
    .unwrap();

    assert_eq!(common::read_reply(&mut r).await, common::simple("OK")); // AUTH
    assert_eq!(common::read_reply(&mut r).await, common::simple("OK")); // SET
    assert_eq!(common::read_reply(&mut r).await, common::bulk("bar")); // GET
}

// ── no-password server ────────────────────────────────────────────────────────

#[tokio::test]
async fn auth_on_open_server_returns_error() {
    let port = common::spawn_server().await;
    let (mut r, mut w) = common::connect(port).await;

    common::write_command(&mut w, &["AUTH", "anything"]).await;
    let reply = common::read_reply(&mut r).await;
    assert!(reply.starts_with("-ERR"), "expected error, got: {reply}");
}
