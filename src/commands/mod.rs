pub mod misc;
pub mod strings;
pub mod ttl;

use crate::protocol::{NormalCommand, RespValue};
use crate::server::Shared;
use std::sync::Arc;

pub fn execute(cmd: NormalCommand, shared: &Arc<Shared>) -> RespValue {
    match cmd {
        NormalCommand::Ping(msg) => misc::ping(msg),
        NormalCommand::Echo(msg) => misc::echo(msg),
        NormalCommand::Get { key } => strings::get(shared, key),
        NormalCommand::Set { key, value, ttl } => strings::set(shared, key, value, ttl),
        NormalCommand::Del { keys } => strings::del(shared, keys),
        NormalCommand::Exists { key } => strings::exists(shared, key),
        NormalCommand::Incr { key } => strings::incr(shared, key),
        NormalCommand::Decr { key } => strings::decr(shared, key),
        NormalCommand::Expire { key, seconds } => ttl::expire(shared, &key, seconds),
        NormalCommand::Ttl { key } => ttl::ttl(shared, &key),
        NormalCommand::Publish { channel, message } => {
            let delivered = shared.pubsub.publish(&channel, message);
            RespValue::Integer(delivered as i64)
        }
        NormalCommand::IncrBy { key, value } => strings::incrby(shared, key, value),
        NormalCommand::Persist { key } => ttl::persist(shared, &key),
        NormalCommand::Copy {
            source,
            destination,
            replace,
        } => strings::copy(shared, source, destination, replace),
    }
}
