use std::time::Instant;

#[derive(Clone)]
pub enum EntryValue {
    Str(String),
    Int(i64),
}

#[derive(Clone)]
pub struct StoreEntry {
    pub value: EntryValue,
    pub expires_at: Option<Instant>,
}

impl StoreEntry {
    pub fn str(value: String, expires_at: Option<Instant>) -> Self {
        StoreEntry {
            value: EntryValue::Str(value),
            expires_at,
        }
    }

    pub fn int(value: i64, expires_at: Option<Instant>) -> Self {
        StoreEntry {
            value: EntryValue::Int(value),
            expires_at,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|t| t <= Instant::now())
            .unwrap_or(false)
    }

    pub fn flush(&mut self) -> &mut Self {
        self.value = match self.value {
            EntryValue::Str(_) => EntryValue::Str(String::new()),
            EntryValue::Int(_) => EntryValue::Int(0),
        };
        self.expires_at = None;
        self
    }
}
