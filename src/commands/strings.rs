// SET, GET, DEL, EXISTS, INCR, DECR

use std::time::Duration;

use crate::protocol::RespValue;
use crate::server::Shared;

pub fn set(shared: &Shared, key: String, value: String, ttl: Option<u64>) -> RespValue {
    let store = &shared.store;
    store.set(key, value, ttl.map(Duration::from_secs));
    RespValue::Simple("OK".to_string())
}

pub fn get(shared: &Shared, key: String) -> RespValue {
    let store = &shared.store;
    match store.get(&key) {
        Some(x) => RespValue::Bulk(x),
        None => RespValue::Nil,
    }
}

pub fn del(shared: &Shared, keys: Vec<String>) -> RespValue {
    let store = &shared.store;
    let count: i64 = keys.iter().map(|k| store.del(k) as i64).sum();
    RespValue::Integer(count)
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

pub fn copy(shared: &Shared, source: String, destination: String, replace: bool) -> RespValue {
    let store = &shared.store;
    RespValue::Integer(store.copy(&source, &destination, replace) as i64)
}

/// Used simultaneously for increment, decrement and increment by cmds, set `delta` to `1` or `-1`
fn apply_delta(shared: &Shared, key: String, delta: i64) -> RespValue {
    let store = &shared.store;
    match store.incrby(&key, delta) {
        Ok(x) => RespValue::Integer(x),
        Err(m) => RespValue::Error(m.to_string()),
    }
}
