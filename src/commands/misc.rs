use crate::protocol::Reply;

pub fn ping(msg: Option<String>) -> Reply {
    match msg {
        None => Reply::Simple("PONG".into()),
        Some(m) => Reply::Bulk(m),
    }
}

pub fn echo(msg: String) -> Reply {
    Reply::Bulk(msg)
}
