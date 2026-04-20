pub mod misc;
pub mod strings;
pub mod ttl;

use std::sync::Arc;

use crate::protocol::text::{Command, Reply};
use crate::server::Shared;

pub fn execute(cmd: Command, _shared: &Arc<Shared>) -> Reply {
    match cmd {
        Command::Ping(msg) => misc::ping(msg),
        Command::Echo(msg) => misc::echo(msg),
    }
}
