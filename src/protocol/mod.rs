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
    // pub sub
    Subscribe { channels: Vec<String> },
    Unsubscribe { channels: Vec<String> },
    Publish { channel: String, message: String },
    Quit,
    Reset,
}

#[derive(Debug)]
pub enum Reply {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(String),
    Array(Vec<Reply>),
    Nil,
}
