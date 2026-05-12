use crate::protocol::RespValue;

pub fn ping(msg: Option<String>) -> RespValue {
    match msg {
        None => RespValue::Simple("PONG".into()),
        Some(m) => RespValue::Bulk(m),
    }
}

pub fn echo(msg: String) -> RespValue {
    RespValue::Bulk(msg)
}
