use anyhow::{bail, Result};

#[derive(Debug)]
pub enum Command {
    Ping(Option<String>),
    Echo(String),
}

#[derive(Debug)]
pub enum Reply {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(String),
    Nil,
}

pub fn parse_command(line: &str) -> Result<Command> {
    let args = split_args(line.trim())?;
    if args.is_empty() {
        bail!("empty command");
    }
    match args[0].to_uppercase().as_str() {
        "PING" => Ok(Command::Ping(args.into_iter().nth(1))), // this variant consumes Vec, instead of simple .get(
        "ECHO" => {
            if args.len() < 2 {
                bail!("wrong number of arguments for 'echo'");
            }
            Ok(Command::Echo(args.into_iter().nth(1).unwrap()))
        }
        cmd => bail!("unknown command '{}'", cmd),
    }
}

pub fn encode_reply(reply: &Reply) -> String {
    match reply {
        Reply::Simple(s) => format!("+{}\r\n", s),
        Reply::Error(s) => format!("-ERR {}\r\n", s),
        Reply::Integer(n) => format!(":{}\r\n", n),
        Reply::Bulk(s) => format!("${}\r\n{}\r\n", s.len(), s),
        Reply::Nil => "$-1\r\n".to_string(),
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
