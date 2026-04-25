pub mod misc;
pub mod strings;
pub mod ttl;

use crate::protocol::{Command, Reply};
use crate::server::Shared;
use std::sync::Arc;

pub fn execute(cmd: Command, shared: &Arc<Shared>) -> Reply {
    match cmd {
        Command::Ping(msg) => misc::ping(msg),
        Command::Echo(msg) => misc::echo(msg),
        Command::Get { key } => strings::get(shared, key),
        Command::Set { key, value } => strings::set(shared, key, value),
        Command::Del { key } => strings::del(shared, key),
        Command::Exists { key } => strings::exists(shared, key),
        Command::Incr { key } => strings::incr(shared, key),
        Command::Decr { key } => strings::decr(shared, key),
        Command::Expire { key, seconds } => ttl::expire(shared, &key, seconds),
        Command::Ttl { key } => ttl::ttl(shared, &key),
        Command::Publish { channel, message } => {
            let delivered = shared.pubsub.publish(&channel, message);
            Reply::Integer(delivered as i64)
        }
        Command::Subscribe { .. } | Command::Unsubscribe { .. } => {
            Reply::Error("subscription commands must be handled by the connection".into())
        }
        Command::Quit | Command::Reset => {
            Reply::Error("connection commands must be handled by the connection".into())
        }
    }
}
