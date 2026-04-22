// SET, GET, DEL, EXISTS, INCR, DECR

use crate::protocol::Reply;
use crate::server::Shared;

pub fn set(shared: &Shared, key: String, value: String) -> Reply {
    let store = &shared.store;
    store.set(key, value, None);
    Reply::Simple("OK".to_string())
}

pub fn get(shared: &Shared, key: String) -> Reply {
    let store = &shared.store;
    match store.get(&key) {
        Some(x) => Reply::Bulk(x),
        None => Reply::Nil,
    }
}

pub fn del(shared: &Shared, key: String) -> Reply {
    let store = &shared.store;
    Reply::Integer(store.del(&key) as i64)
}

pub fn exists(shared: &Shared, key: String) -> Reply {
    let store = &shared.store;
    Reply::Integer(store.exists(&key) as i64)
}

pub fn incr(shared: &Shared, key: String) -> Reply {
    apply_delta(shared, key, 1)
}

pub fn decr(shared: &Shared, key: String) -> Reply {
    apply_delta(shared, key, -1)
}

/// Used simultaneously for increment and decrement cmds, set `delta` to `1` or `-1`
fn apply_delta(shared: &Shared, key: String, delta: i64) -> Reply {
    let store = &shared.store;
    match store.incrby(&key, delta) {
        Ok(x) => Reply::Integer(x),
        Err(m) => Reply::Error(m.to_string()),
    }
}
