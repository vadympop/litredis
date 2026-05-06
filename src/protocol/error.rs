use std::fmt;
use std::fmt::Formatter;

#[derive(Debug)]
pub enum ProtocolError {
    EmptyCommand,
    UnterminatedString,
    UnknownCommand(String),  // includes the bad command name
    InvalidArgument(String), // includes the parse reason
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand => write!(f, "empty command"),
            Self::UnterminatedString => write!(f, "unterminated quoted string"),
            Self::UnknownCommand(cmd) => write!(f, "unknown or wrong-arity command '{cmd}'"),
            Self::InvalidArgument(reason) => write!(f, "invalid argument: {reason}"),
        }
    }
}
