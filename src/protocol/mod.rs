mod error;
pub mod text;

#[derive(Debug)]
pub enum Command {
    Normal(NormalCommand),
    Session(SessionCommand),
}

#[derive(Debug)]
pub enum NormalCommand {
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
    Publish { channel: String, message: String },
}

#[derive(Debug)]
pub enum SessionCommand {
    Subscribe { channels: Vec<String> },
    Unsubscribe { channels: Vec<String> },
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
