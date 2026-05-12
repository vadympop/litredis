// SET, GET, DEL, EXISTS, INCR, DECR

use crate::protocol::RespValue;
use crate::server::Shared;

pub fn set(shared: &Shared, key: String, value: String) -> RespValue {
    let store = &shared.store;
    store.set(key, value, None);
    RespValue::Simple("OK".to_string())
}

pub fn get(shared: &Shared, key: String) -> RespValue {
    let store = &shared.store;
    match store.get(&key) {
        Some(x) => RespValue::Bulk(x),
        None => RespValue::Nil,
    }
}

pub fn del(shared: &Shared, key: String) -> RespValue {
    let store = &shared.store;
    RespValue::Integer(store.del(&key) as i64)
}

pub fn exists(shared: &Shared, key: String) -> RespValue {
    let store = &shared.store;
    RespValue::Integer(store.exists(&key) as i64)
}

pub fn incr(shared: &Shared, key: String) -> RespValue {
    apply_delta(shared, key, 1)
}

pub fn decr(shared: &Shared, key: String) -> RespValue {
    apply_delta(shared, key, -1)
}

pub fn incrby(shared: &Shared, key: String, value: i64) -> RespValue {
    apply_delta(shared, key, value)
}

/// Used simultaneously for increment, decrement and increment by cmds, set `delta` to `1` or `-1`
fn apply_delta(shared: &Shared, key: String, delta: i64) -> RespValue {
    let store = &shared.store;
    match store.incrby(&key, delta) {
        Ok(x) => RespValue::Integer(x),
        Err(m) => RespValue::Error(m.to_string()),
    }
}
