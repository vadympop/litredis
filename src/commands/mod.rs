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
        Command::Incr { key } => strings::incr(shared, key, 1),
        Command::Decr { key } => strings::incr(shared, key, -1),
        Command::Expire { .. } => Reply::Nil,
        Command::Ttl { .. } => Reply::Nil,
    }
}
