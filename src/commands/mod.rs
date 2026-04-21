pub mod misc;
pub mod strings;
pub mod ttl;

use std::sync::Arc;
use crate::protocol::{Command, Reply};
use crate::server::Shared;

pub fn execute(cmd: Command, _shared: &Arc<Shared>) -> Reply {
    match cmd {
        Command::Ping(msg) => misc::ping(msg),
        Command::Echo(msg) => misc::echo(msg),
        Command::Get { key } => strings::get(_shared, key),
        Command::Set { key, value } => strings::set(_shared, key, value),
        Command::Del { key } => strings::del(_shared, key),
        Command::Exists { key } => strings::exists(_shared, key),
        Command::Incr { key } => strings::incr(_shared, key, 1),
        Command::Decr { key } => strings::incr(_shared, key, -1),
        Command::Expire { .. } => Reply::Nil,
        Command::Ttl { .. } => Reply::Nil
    }
}
