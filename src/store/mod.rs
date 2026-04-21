mod entry;

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use dashmap::DashMap;

use entry::{EntryValue, StoreEntry};

pub struct Store {
    data: DashMap<String, StoreEntry>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            data: DashMap::new(),
        }
    }

    /// Returns `None` if the key is missing or expired.
    /// Returns `Err` if the key holds a non-string type.
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        match self.data.get(key) {
            None => Ok(None),
            Some(r) if r.is_expired() => {
                drop(r);
                self.data.remove(key);
                Ok(None)
            }
            Some(r) => match &r.value().value {
                EntryValue::Str(s) => Ok(Some(s.clone())),
                EntryValue::Int(n) => Ok(Some(n.to_string())),
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
    pub fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        let mut entry = self
            .data
            .entry(key.to_string())
            .or_insert_with(|| StoreEntry::int(0, None));

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
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}
