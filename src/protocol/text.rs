use crate::protocol::{Command, Reply};
use anyhow::{Result, bail};

const LINE_ENDING: &str = "\r\n";

pub fn parse_command(line: &str) -> Result<Command> {
    let mut args = split_args(line.trim())?;
    if args.is_empty() {
        bail!("empty command");
    }
    args[0].make_ascii_uppercase();

    // Dont use values directly from slice-pattern because there are UPPER values,
    // but commands required original
    match args.as_slice() {
        [cmd] if cmd == "PING" => Ok(Command::Ping(None)),
        [cmd, key] if cmd == "PING" => Ok(Command::Ping(Some(key.clone()))),
        [cmd, key] if cmd == "ECHO" => Ok(Command::Echo(key.clone())),

        [cmd, key] if cmd == "GET" => Ok(Command::Get { key: key.clone() }),
        [cmd, key, value] if cmd == "SET" => Ok(Command::Set {
            key: key.clone(),
            value: value.clone(),
        }),
        [cmd, key] if cmd == "DEL" => Ok(Command::Del { key: key.clone() }),
        [cmd, key] if cmd == "EXISTS" => Ok(Command::Exists { key: key.clone() }),
        [cmd, key] if cmd == "INCR" => Ok(Command::Incr { key: key.clone() }),
        [cmd, key] if cmd == "DECR" => Ok(Command::Decr { key: key.clone() }),
        [cmd, key, value] if cmd == "EXPIRE" => Ok(Command::Expire {
            key: key.clone(),
            seconds: value.parse::<u64>()?,
        }),
        [cmd, key] if cmd == "TTL" => Ok(Command::Ttl { key: key.clone() }),
        [cmd, channels @ ..] if cmd == "SUBSCRIBE" && !channels.is_empty() => {
            Ok(Command::Subscribe {
                channels: channels.to_vec(),
            })
        }
        [cmd] if cmd == "UNSUBSCRIBE" => Ok(Command::Unsubscribe {
            channels: Vec::new(),
        }),
        [cmd, channels @ ..] if cmd == "UNSUBSCRIBE" => Ok(Command::Unsubscribe {
            channels: channels.to_vec(),
        }),
        [cmd, channel, message] if cmd == "PUBLISH" => Ok(Command::Publish {
            channel: channel.clone(),
            message: message.clone(),
        }),
        [cmd] if cmd == "QUIT" => Ok(Command::Quit),
        [cmd] if cmd == "RESET" => Ok(Command::Reset),

        [cmd, ..] => bail!("unknown or wrong-arity command '{}'", cmd),
        [] => bail!("empty command"),
    }
}

pub fn encode_reply(reply: &Reply) -> String {
    match reply {
        Reply::Simple(s) => format!("+{}{}", s, LINE_ENDING),
        Reply::Error(s) => format!("-ERR {}{}", s, LINE_ENDING),
        Reply::Integer(n) => format!(":{}{}", n, LINE_ENDING),
        Reply::Bulk(s) => format!("${}{}{}{1}", s.len(), LINE_ENDING, s),
        Reply::Array(s) => format!(
            "*{}{}{}",
            s.len(),
            LINE_ENDING,
            s.iter().map(encode_reply).collect::<String>()
        ),
        Reply::Nil => format!("$-1{}", LINE_ENDING),
    }
}

/// Splits a command line into tokens, respecting double-quoted strings.
/// `ECHO "hello world"` → ["ECHO", "hello world"]
fn split_args(input: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        if c == '"' {
            chars.next();
            let mut buf = String::new();
            loop {
                match chars.next() {
                    Some('"') => break,
                    Some('\\') => {
                        if let Some(escaped) = chars.next() {
                            buf.push(escaped);
                        }
                    }
                    Some(ch) => buf.push(ch),
                    None => bail!("unterminated quoted string"),
                }
            }
            args.push(buf);
        } else {
            let mut buf = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                buf.push(c);
                chars.next();
            }
            args.push(buf);
        }
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_plain() {
        assert_eq!(split_args("SET foo bar").unwrap(), ["SET", "foo", "bar"]);
    }

    #[test]
    fn split_quoted() {
        assert_eq!(
            split_args(r#"ECHO "hello world""#).unwrap(),
            ["ECHO", "hello world"]
        );
    }

    #[test]
    fn split_escaped_quote() {
        assert_eq!(
            split_args(r#"ECHO "say \"hi\"""#).unwrap(),
            ["ECHO", r#"say "hi""#]
        );
    }

    #[test]
    fn unterminated_string() {
        assert!(split_args(r#"ECHO "oops"#).is_err());
    }

    #[test]
    fn parse_ping_bare() {
        assert!(matches!(parse_command("PING"), Ok(Command::Ping(None))));
    }

    #[test]
    fn parse_ping_msg() {
        assert!(matches!(
            parse_command("PING hello"),
            Ok(Command::Ping(Some(_)))
        ));
    }

    #[test]
    fn parse_echo() {
        assert!(matches!(parse_command("ECHO hi"), Ok(Command::Echo(_))));
    }

    #[test]
    fn encode_simple() {
        assert_eq!(
            encode_reply(&Reply::Simple("OK".into())),
            format!("+OK{}", LINE_ENDING)
        );
    }

    #[test]
    fn encode_error() {
        assert_eq!(
            encode_reply(&Reply::Error("bad".into())),
            format!("-ERR bad{}", LINE_ENDING)
        );
    }

    #[test]
    fn encode_bulk() {
        assert_eq!(
            encode_reply(&Reply::Bulk("hello".into())),
            format!("$5{0}hello{0}", LINE_ENDING)
        );
    }

    #[test]
    fn encode_array_of_bulk_strings() {
        assert_eq!(
            encode_reply(&Reply::Array(vec![
                Reply::Bulk("message".into()),
                Reply::Bulk("news".into()),
                Reply::Bulk("hello".into()),
            ])),
            format!("*3{0}$7{0}message{0}$4{0}news{0}$5{0}hello{0}", LINE_ENDING)
        );
    }
}
