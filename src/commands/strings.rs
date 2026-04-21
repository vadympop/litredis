// SET, GET, DEL, EXISTS, INCR, DECR

use crate::protocol::Reply;
use crate::server::Shared;

pub fn set(_shared: &Shared, key: String, value: String) -> Reply {
    let store = &_shared.store;
    store.set(key, value, None);
    Reply::Simple("OK".to_string())
}

pub fn get(_shared: &Shared, key: String) -> Reply {
    let store = &_shared.store;
    match store.get(&key) {
        Ok(x) => match x {
            Some(x) => Reply::Bulk(x),
            None => Reply::Nil,
        },
        Err(m) => Reply::Error(m.to_string()),
    }
}

pub fn del(_shared: &Shared, key: String) -> Reply {
    let store = &_shared.store;
    Reply::Integer(store.del(&key) as i64)
}

pub fn exists(_shared: &Shared, key: String) -> Reply {
    let store = &_shared.store;
    Reply::Integer(store.exists(&key) as i64)
}

/// Used simultaneously for increment and decrement cmds, set `delta` to `1` or `-1`
pub fn incr(_shared: &Shared, key: String, delta: i64) -> Reply {
    let store = &_shared.store;
    match store.incr(&key, delta) {
        Ok(x) => Reply::Integer(x),
        Err(m) => Reply::Error(m.to_string()),
    }
}
