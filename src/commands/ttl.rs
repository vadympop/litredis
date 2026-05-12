// EXPIRE, TTL

use crate::{protocol::RespValue, server::Shared};

pub fn expire(shared: &Shared, key: &str, seconds: u64) -> RespValue {
    let store = &shared.store;
    RespValue::Integer(store.expire(key, seconds) as i64)
}

pub fn ttl(shared: &Shared, key: &str) -> RespValue {
    let store = &shared.store;
    RespValue::Integer(store.ttl(key))
}
