pub(crate) mod entry;

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use dashmap::DashMap;

use entry::{EntryValue, StoreEntry};

type InternalStore = DashMap<String, StoreEntry>;

pub struct SnapshotEntry {
    pub key: String,
    pub value: EntryValue,
    pub expires_at: Option<Instant>,
}

pub struct Store {
    data: InternalStore,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: InternalStore::new(),
        }
    }

    pub fn from_snapshot(entries: Vec<SnapshotEntry>) -> Self {
        let data = InternalStore::new();
        for e in entries {
            let entry = match e.value {
                EntryValue::Str(s) => StoreEntry::str(s, e.expires_at),
                EntryValue::Int(n) => StoreEntry::int(n, e.expires_at),
            };
            data.insert(e.key, entry);
        }
        Store { data }
    }

    pub fn to_snapshot_entries(&self) -> Vec<SnapshotEntry> {
        self.data
            .iter()
            .filter(|r| !r.is_expired())
            .map(|r| SnapshotEntry {
                key: r.key().clone(),
                value: r.value().value.clone(),
                expires_at: r.value().expires_at,
            })
            .collect()
    }

    /// Returns `None` if the key is missing or expired.
    pub fn get(&self, key: &str) -> Option<String> {
        match self.data.get(key) {
            None => None,
            Some(r) if r.is_expired() => {
                drop(r);
                self.data.remove(key);
                None
            }
            Some(r) => match &r.value().value {
                EntryValue::Str(s) => Some(s.clone()),
                EntryValue::Int(n) => Some(n.to_string()),
            },
        }
    }

    /// Stores a string value, overwriting any previous type
    pub fn set(&self, key: String, value: String, ttl: Option<Duration>) {
        let expires_at = ttl.map(|d| Instant::now() + d);
        self.data.insert(key, StoreEntry::str(value, expires_at));
    }

    /// Removes a key. Returns `true` if the key existed and was not expired.
    pub fn del(&self, key: &str) -> bool {
        match self.data.remove(key) {
            Some((_, entry)) => !entry.is_expired(),
            None => false,
        }
    }

    /// Returns `true` if the key exists and has not expired.
    pub fn exists(&self, key: &str) -> bool {
        self.data.get(key).map(|r| !r.is_expired()).unwrap_or(false)
    }

    /// Increments the integer value of a key by `delta` (use -1 for DECR).
    /// Missing key is treated as 0. Returns `Err` if the value is not an integer.
    pub fn incrby(&self, key: &str, delta: i64) -> Result<i64> {
        let mut entry = self
            .data
            .entry(key.to_string())
            .or_insert_with(|| StoreEntry::int(0, None));

        if entry.is_expired() {
            entry.flush();
        }

        let current: i64 = match &entry.value {
            EntryValue::Int(n) => *n,
            EntryValue::Str(s) => s
                .parse()
                .map_err(|_| anyhow!("value is not an integer or out of range"))?,
        };

        let new_val = current
            .checked_add(delta)
            .ok_or_else(|| anyhow!("increment or decrement would overflow"))?;

        entry.value = EntryValue::Int(new_val);
        Ok(new_val)
    }

    /// Sets a TTL on a key. Returns `false` if the key does not exist or is expired.
    pub fn expire(&self, key: &str, seconds: u64) -> bool {
        match self.data.get_mut(key) {
            None => false,
            Some(r) if r.is_expired() => false,
            Some(mut r) => {
                r.expires_at = Some(Instant::now() + Duration::from_secs(seconds));
                true
            }
        }
    }

    /// Returns remaining TTL in seconds, -1 if no expiry, -2 if missing or expired.
    pub fn ttl(&self, key: &str) -> i64 {
        match self.data.get(key) {
            None => -2,
            Some(r) if r.is_expired() => -2,
            Some(r) => match r.expires_at {
                None => -1,
                Some(t) => t.saturating_duration_since(Instant::now()).as_secs() as i64,
            },
        }
    }

    /// Clears a TTL from a key. Returns `false` if the key does not exist or is expired.
    pub fn persist(&self, key: &str) -> bool {
        match self.data.get_mut(key) {
            None => false,
            Some(r) if r.is_expired() => false,
            Some(mut r) => {
                r.expires_at = None;
                true
            }
        }
    }

    /// Copies value (and TTL) from `source` to `destination`.
    /// Returns `false` if source is missing/expired, or destination exists and `replace` is false.
    pub fn copy(&self, source: &str, dest: &str, replace: bool) -> bool {
        let entry = match self.data.get(source) {
            None => return false,
            Some(r) if r.is_expired() => return false,
            Some(r) => r.clone(),
        };
        if !replace && self.exists(dest) {
            return false;
        }
        self.data.insert(dest.to_string(), entry);
        true
    }

    /// Deletes expired entries
    pub fn purge_expired(&self) {
        let mut expired_keys = Vec::<String>::new();
        self.data.iter().for_each(|item| {
            if item.value().is_expired() {
                expired_keys.push(item.key().clone());
            }
        });
        for key in expired_keys {
            self.del(&key);
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Store;
    use std::{thread, time::Duration};

    #[test]
    fn purge_expired_removes_expired_keys() {
        let store = Store::new();
        store.set(
            "temp".to_string(),
            "v".to_string(),
            Some(Duration::from_millis(20)),
        );

        thread::sleep(Duration::from_millis(40));
        store.purge_expired();

        assert_eq!(store.get("temp"), None);
        assert!(!store.exists("temp"));
    }

    #[test]
    fn purge_expired_keeps_non_expired_keys() {
        let store = Store::new();
        store.set("alive".to_string(), "v".to_string(), None);

        store.purge_expired();

        assert_eq!(store.get("alive"), Some("v".to_string()));
        assert!(store.exists("alive"));
    }

    #[test]
    fn purge_expired_removes_only_expired_keys() {
        let store = Store::new();
        store.set(
            "expired".to_string(),
            "a".to_string(),
            Some(Duration::from_millis(20)),
        );
        store.set(
            "long_lived".to_string(),
            "b".to_string(),
            Some(Duration::from_millis(500)),
        );
        store.set("persistent".to_string(), "c".to_string(), None);

        thread::sleep(Duration::from_millis(50));
        store.purge_expired();

        assert_eq!(store.get("expired"), None);
        assert_eq!(store.get("long_lived"), Some("b".to_string()));
        assert_eq!(store.get("persistent"), Some("c".to_string()));
    }
}
