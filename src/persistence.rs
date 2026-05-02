use crate::store::entry::{EntryValue, StoreEntry};
use crate::store::{InternalStore, Store};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PersistedValue {
    Str(String),
    Int(i64),
}

#[derive(Serialize, Deserialize)]
struct PersistenceDataNode {
    key: String,
    value: PersistedValue,
    expires_at: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct PersistenceData {
    entries: Vec<PersistenceDataNode>,
    timestamp: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn tmp_path(base: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{base}.{}.{n}.tmp", std::process::id())
}

pub fn load(path: &str) -> Store {
    let internal_store = InternalStore::new();
    let Ok(file_content) = fs::read_to_string(path) else {
        return Store::new();
    };
    let Ok(parsed_data): Result<PersistenceData, _> = serde_json::from_str(&file_content) else {
        return Store::new();
    };

    let now_unix = now_secs();
    let now_instant = Instant::now();

    for entry in parsed_data.entries {
        let expires_at = match entry.expires_at {
            Some(x) if x <= now_unix => continue,
            Some(x) => Some(now_instant + Duration::from_secs(x - now_unix)),
            None => None,
        };
        let to_insert = match entry.value {
            PersistedValue::Str(x) => StoreEntry::str(x, expires_at),
            PersistedValue::Int(x) => StoreEntry::int(x, expires_at),
        };

        internal_store.insert(entry.key, to_insert);
    }

    Store::with_data(internal_store)
}

pub fn save(store: &Store, path: &str) -> anyhow::Result<()> {
    let now_unix = now_secs();
    let now_instant = Instant::now();

    let entries: Vec<PersistenceDataNode> = store
        .get_raw_data()
        .iter()
        .filter(|x| !x.is_expired())
        .map(|x| {
            let value = match &x.value {
                EntryValue::Str(x) => PersistedValue::Str(x.clone()),
                EntryValue::Int(x) => PersistedValue::Int(*x),
            };
            let expires_at = x.expires_at.map(|t| {
                let remaining = t.saturating_duration_since(now_instant);
                now_unix + remaining.as_secs()
            });
            PersistenceDataNode {
                key: x.key().clone(),
                value,
                expires_at,
            }
        })
        .collect();

    let to_save = PersistenceData {
        entries,
        timestamp: now_unix,
    };
    let tmp = tmp_path(path);
    fs::write(&tmp, serde_json::to_string(&to_save)?)?;
    fs::rename(tmp, path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> String {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!(
            "{}/persist_unit_{}_{n}.json",
            std::env::temp_dir().display(),
            std::process::id()
        )
    }

    #[test]
    fn missing_file_returns_empty_store() {
        let store = load("/no/such/file/dump.json");
        assert_eq!(store.get("k"), None);
    }

    #[test]
    fn corrupt_file_returns_empty_store() {
        let p = tmp();
        fs::write(&p, b"not json").unwrap();
        let store = load(&p);
        assert_eq!(store.get("k"), None);
        fs::remove_file(&p).ok();
    }

    #[test]
    fn roundtrip_str() {
        let p = tmp();
        let store = Store::new();
        store.set("hello".into(), "world".into(), None);
        save(&store, &p).unwrap();
        let loaded = load(&p);
        assert_eq!(loaded.get("hello"), Some("world".into()));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn roundtrip_int() {
        let p = tmp();
        let store = Store::new();
        store.incrby("n", 99).unwrap();
        save(&store, &p).unwrap();
        let loaded = load(&p);
        assert_eq!(loaded.get("n"), Some("99".into()));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn expired_entry_discarded_on_load() {
        let p = tmp();
        let store = Store::new();
        store.set("x".into(), "y".into(), Some(Duration::from_millis(50)));
        save(&store, &p).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let loaded = load(&p);
        assert_eq!(loaded.get("x"), None);
        fs::remove_file(&p).ok();
    }

    #[test]
    fn ttl_approximately_preserved() {
        let p = tmp();
        let store = Store::new();
        store.set("k".into(), "v".into(), Some(Duration::from_secs(30)));
        save(&store, &p).unwrap();
        let loaded = load(&p);
        let ttl = loaded.ttl("k");
        assert!(ttl > 25 && ttl <= 30, "expected TTL ~30s, got {ttl}");
        fs::remove_file(&p).ok();
    }
}
