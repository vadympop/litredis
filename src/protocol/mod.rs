pub mod text;

#[derive(Debug)]
pub enum Command {
    Ping(Option<String>),
    Echo(String),
    Get { key: String },
    Set { key: String, value: String },
    Del { key: String },
    Exists { key: String },
    Incr { key: String },
    Decr { key: String },
    Expire { key: String, seconds: u64 },
    Ttl { key: String },
}

#[derive(Debug)]
pub enum Reply {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(String),
    Nil,
}
