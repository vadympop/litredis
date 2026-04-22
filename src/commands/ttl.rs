// EXPIRE, TTL

use crate::{protocol::Reply, server::Shared};

pub fn expire(shared: &Shared, key: &str, seconds: u64) -> Reply {
    let store = &shared.store;
    Reply::Integer(store.expire(key, seconds) as i64)
}

pub fn ttl(shared: &Shared, key: &str) -> Reply {
    let store = &shared.store;
    Reply::Integer(store.ttl(key))
}
