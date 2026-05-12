use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt};

use crate::protocol::error::ProtocolError;
use crate::protocol::{Command, NormalCommand, RespValue, SessionCommand};

const LINE_ENDING: &str = "\r\n";

pub fn parse_command(line: &str) -> Result<Command, ProtocolError> {
    let args = split_args(line.trim())?;
    parse_command_args(args)
}

pub fn parse_command_args(mut args: Vec<String>) -> Result<Command, ProtocolError> {
    if args.is_empty() {
        return Err(ProtocolError::EmptyCommand);
    }
    args[0].make_ascii_uppercase();

    // Dont use values directly from slice-pattern because there are UPPER values,
    // but commands required original
    match args.as_slice() {
        [cmd] if cmd == "PING" => Ok(Command::Normal(NormalCommand::Ping(None))),
        [cmd, key] if cmd == "PING" => Ok(Command::Normal(NormalCommand::Ping(Some(key.clone())))),
        [cmd, key] if cmd == "ECHO" => Ok(Command::Normal(NormalCommand::Echo(key.clone()))),

        [cmd, key] if cmd == "GET" => Ok(Command::Normal(NormalCommand::Get { key: key.clone() })),
        [cmd, key, value] if cmd == "SET" => Ok(Command::Normal(NormalCommand::Set {
            key: key.clone(),
            value: value.clone(),
        })),
        [cmd, key] if cmd == "DEL" => Ok(Command::Normal(NormalCommand::Del { key: key.clone() })),
        [cmd, key] if cmd == "EXISTS" => {
            Ok(Command::Normal(NormalCommand::Exists { key: key.clone() }))
        }
        [cmd, key] if cmd == "INCR" => {
            Ok(Command::Normal(NormalCommand::Incr { key: key.clone() }))
        }
        [cmd, key] if cmd == "DECR" => {
            Ok(Command::Normal(NormalCommand::Decr { key: key.clone() }))
        }
        [cmd, key, value] if cmd == "EXPIRE" => Ok(Command::Normal(NormalCommand::Expire {
            key: key.clone(),
            seconds: value
                .parse::<u64>()
                .map_err(|e| ProtocolError::InvalidArgument(e.to_string()))?,
        })),
        [cmd, key] if cmd == "TTL" => Ok(Command::Normal(NormalCommand::Ttl { key: key.clone() })),
        [cmd, channels @ ..] if cmd == "SUBSCRIBE" && !channels.is_empty() => {
            Ok(Command::Session(SessionCommand::Subscribe {
                channels: channels.to_vec(),
            }))
        }
        [cmd] if cmd == "UNSUBSCRIBE" => Ok(Command::Session(SessionCommand::Unsubscribe {
            channels: Vec::new(),
        })),
        [cmd, channels @ ..] if cmd == "UNSUBSCRIBE" => {
            Ok(Command::Session(SessionCommand::Unsubscribe {
                channels: channels.to_vec(),
            }))
        }
        [cmd, channel, message] if cmd == "PUBLISH" => {
            Ok(Command::Normal(NormalCommand::Publish {
                channel: channel.clone(),
                message: message.clone(),
            }))
        }
        [cmd, password] if cmd == "AUTH" => Ok(Command::Auth {
            password: password.clone(),
        }),
        [cmd] if cmd == "QUIT" => Ok(Command::Session(SessionCommand::Quit)),
        [cmd] if cmd == "RESET" => Ok(Command::Session(SessionCommand::Reset)),

        [cmd, ..] => Err(ProtocolError::UnknownCommand(cmd.clone())),
        [] => Err(ProtocolError::EmptyCommand),
    }
}

pub fn encode_resp_value(reply: &RespValue) -> String {
    match reply {
        RespValue::Simple(s) => format!("+{}{}", s, LINE_ENDING),
        RespValue::Error(s) => format!("-ERR {}{}", s, LINE_ENDING),
        RespValue::Integer(n) => format!(":{}{}", n, LINE_ENDING),
        RespValue::Bulk(s) => format!("${}{}{}{1}", s.len(), LINE_ENDING, s),
        RespValue::Array(s) => format!(
            "*{}{}{}",
            s.len(),
            LINE_ENDING,
            s.iter().map(encode_resp_value).collect::<String>()
        ),
        RespValue::Nil => format!("$-1{}", LINE_ENDING),
    }
}

pub fn encode_command(args: &[String]) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(format!("*{}{}", args.len(), LINE_ENDING).as_bytes());

    for arg in args {
        encoded.extend_from_slice(format!("${}{}", arg.len(), LINE_ENDING).as_bytes());
        encoded.extend_from_slice(arg.as_bytes());
        encoded.extend_from_slice(LINE_ENDING.as_bytes());
    }

    encoded
}

pub async fn read_resp_value<R>(reader: &mut R) -> Result<RespValue, ProtocolError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let prefix = read_prefix(reader).await?;
    match prefix {
        b'*' => read_resp_array(reader).await,
        other => read_resp_non_array_value(reader, other).await,
    }
}

pub fn command_from_resp_value(value: RespValue) -> Result<Command, ProtocolError> {
    let RespValue::Array(items) = value else {
        return Err(ProtocolError::InvalidFrame(
            "command must be a RESP array".into(),
        ));
    };

    if items.is_empty() {
        return Err(ProtocolError::EmptyCommand);
    }

    let mut args = Vec::with_capacity(items.len());
    for item in items {
        match item {
            RespValue::Bulk(arg) => args.push(arg),
            _ => {
                return Err(ProtocolError::InvalidFrame(
                    "command arguments must be bulk strings".into(),
                ));
            }
        }
    }

    parse_command_args(args)
}

async fn read_resp_non_array_value<R>(
    reader: &mut R,
    prefix: u8,
) -> Result<RespValue, ProtocolError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    match prefix {
        b'+' => Ok(RespValue::Simple(read_utf8_line(reader).await?)),
        b'-' => Ok(RespValue::Error(read_utf8_line(reader).await?)),
        b':' => read_integer(reader).await,
        b'$' => read_bulk_string(reader).await,
        _ => Err(ProtocolError::InvalidFrame(
            "unsupported RESP prefix".into(),
        )),
    }
}

async fn read_resp_array<R>(reader: &mut R) -> Result<RespValue, ProtocolError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let line = read_crlf_line(reader).await?;
    let count = parse_i64(&line)?;
    if count == -1 {
        return Err(ProtocolError::InvalidFrame(
            "null arrays are not supported".into(),
        ));
    }
    if count < -1 {
        return Err(ProtocolError::InvalidFrame("invalid array length".into()));
    }

    let count = usize::try_from(count)
        .map_err(|_| ProtocolError::InvalidFrame("array length overflow".into()))?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        let prefix = read_prefix(reader).await?;
        if prefix == b'*' {
            return Err(ProtocolError::InvalidFrame(
                "nested arrays are not supported".into(),
            ));
        }
        values.push(read_resp_non_array_value(reader, prefix).await?);
    }

    Ok(RespValue::Array(values))
}

async fn read_prefix<R>(reader: &mut R) -> Result<u8, ProtocolError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0u8; 1];
    reader
        .read_exact(&mut prefix)
        .await
        .map_err(|e| ProtocolError::InvalidFrame(e.to_string()))?;
    Ok(prefix[0])
}

async fn read_integer<R>(reader: &mut R) -> Result<RespValue, ProtocolError>
where
    R: AsyncBufRead + Unpin,
{
    let line = read_crlf_line(reader).await?;
    let n = parse_i64(&line)?;
    Ok(RespValue::Integer(n))
}

async fn read_bulk_string<R>(reader: &mut R) -> Result<RespValue, ProtocolError>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let line = read_crlf_line(reader).await?;
    let len = parse_i64(&line)?;
    if len == -1 {
        return Ok(RespValue::Nil);
    }
    if len < -1 {
        return Err(ProtocolError::InvalidFrame("invalid bulk length".into()));
    }

    let len = usize::try_from(len)
        .map_err(|_| ProtocolError::InvalidFrame("bulk length overflow".into()))?;
    let mut body = vec![0u8; len + LINE_ENDING.len()];
    reader
        .read_exact(&mut body)
        .await
        .map_err(|e| ProtocolError::InvalidFrame(e.to_string()))?;

    if &body[len..] != LINE_ENDING.as_bytes() {
        return Err(ProtocolError::InvalidFrame(
            "invalid bulk string terminator".into(),
        ));
    }

    let value = bytes_to_string(&body[..len])?;
    Ok(RespValue::Bulk(value))
}

async fn read_utf8_line<R>(reader: &mut R) -> Result<String, ProtocolError>
where
    R: AsyncBufRead + Unpin,
{
    let line = read_crlf_line(reader).await?;
    bytes_to_string(&line)
}

async fn read_crlf_line<R>(reader: &mut R) -> Result<Vec<u8>, ProtocolError>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    let n = reader
        .read_until(b'\n', &mut line)
        .await
        .map_err(|e| ProtocolError::InvalidFrame(e.to_string()))?;
    if n == 0 {
        return Err(ProtocolError::InvalidFrame("unexpected EOF".into()));
    }

    if line.len() < LINE_ENDING.len() || !line.ends_with(LINE_ENDING.as_bytes()) {
        return Err(ProtocolError::InvalidFrame("invalid line ending".into()));
    }

    line.truncate(line.len() - LINE_ENDING.len());
    Ok(line)
}

fn parse_i64(bytes: &[u8]) -> Result<i64, ProtocolError> {
    let s = std::str::from_utf8(bytes)
        .map_err(|_| ProtocolError::InvalidFrame("non-utf8 length".into()))?;
    s.parse::<i64>()
        .map_err(|_| ProtocolError::InvalidFrame("invalid length".into()))
}

fn bytes_to_string(bytes: &[u8]) -> Result<String, ProtocolError> {
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|_| ProtocolError::InvalidFrame("non-utf8 string".into()))
}

/// Splits a command line into tokens, respecting double-quoted strings.
/// `ECHO "hello world"` → ["ECHO", "hello world"]
fn split_args(input: &str) -> Result<Vec<String>, ProtocolError> {
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
                    None => return Err(ProtocolError::UnterminatedString),
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
    use tokio::io::{AsyncWriteExt, BufReader};

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
        assert!(matches!(
            parse_command("PING"),
            Ok(Command::Normal(NormalCommand::Ping(None)))
        ));
    }

    #[test]
    fn parse_ping_msg() {
        assert!(matches!(
            parse_command("PING hello"),
            Ok(Command::Normal(NormalCommand::Ping(Some(_))))
        ));
    }

    #[test]
    fn parse_echo() {
        assert!(matches!(
            parse_command("ECHO hi"),
            Ok(Command::Normal(NormalCommand::Echo(_)))
        ));
    }

    #[test]
    fn parse_args_ping_bare() {
        assert!(matches!(
            parse_command_args(strings(&["PING"])),
            Ok(Command::Normal(NormalCommand::Ping(None)))
        ));
    }

    #[test]
    fn parse_args_lowercase_command() {
        assert!(matches!(
            parse_command_args(strings(&["get", "foo"])),
            Ok(Command::Normal(NormalCommand::Get { key })) if key == "foo"
        ));
    }

    #[test]
    fn parse_args_set_preserves_value_case() {
        assert!(matches!(
            parse_command_args(strings(&["set", "foo", "Bar"])),
            Ok(Command::Normal(NormalCommand::Set { key, value })) if key == "foo" && value == "Bar"
        ));
    }

    #[test]
    fn parse_args_subscribe_multiple_channels() {
        match parse_command_args(strings(&["SUBSCRIBE", "news", "alerts"])).unwrap() {
            Command::Session(SessionCommand::Subscribe { channels }) => {
                assert_eq!(channels, ["news", "alerts"]);
            }
            other => panic!("expected subscribe command, got {other:?}"),
        }
    }

    #[test]
    fn parse_args_auth() {
        assert!(matches!(
            parse_command_args(strings(&["AUTH", "secret"])),
            Ok(Command::Auth { password }) if password == "secret"
        ));
    }

    #[test]
    fn parse_args_wrong_arity_is_unknown_command() {
        assert!(matches!(
            parse_command_args(strings(&["SET", "foo"])),
            Err(ProtocolError::UnknownCommand(cmd)) if cmd == "SET"
        ));
    }

    #[test]
    fn parse_args_invalid_expire_seconds() {
        assert!(matches!(
            parse_command_args(strings(&["EXPIRE", "foo", "soon"])),
            Err(ProtocolError::InvalidArgument(_))
        ));
    }

    #[test]
    fn encode_simple() {
        assert_eq!(
            encode_resp_value(&RespValue::Simple("OK".into())),
            format!("+OK{}", LINE_ENDING)
        );
    }

    #[test]
    fn encode_error() {
        assert_eq!(
            encode_resp_value(&RespValue::Error("bad".into())),
            format!("-ERR bad{}", LINE_ENDING)
        );
    }

    #[test]
    fn encode_bulk() {
        assert_eq!(
            encode_resp_value(&RespValue::Bulk("hello".into())),
            format!("$5{0}hello{0}", LINE_ENDING)
        );
    }

    #[test]
    fn encode_array_of_bulk_strings() {
        assert_eq!(
            encode_resp_value(&RespValue::Array(vec![
                RespValue::Bulk("message".into()),
                RespValue::Bulk("news".into()),
                RespValue::Bulk("hello".into()),
            ])),
            format!("*3{0}$7{0}message{0}$4{0}news{0}$5{0}hello{0}", LINE_ENDING)
        );
    }

    #[test]
    fn encode_command_single_argument() {
        assert_eq!(encode_command(&strings(&["PING"])), b"*1\r\n$4\r\nPING\r\n");
    }

    #[test]
    fn encode_command_multiple_arguments() {
        assert_eq!(
            encode_command(&strings(&["SET", "foo", "bar"])),
            b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n"
        );
    }

    #[test]
    fn encode_command_preserves_spaces_inside_argument() {
        assert_eq!(
            encode_command(&strings(&["SET", "message", "hello world"])),
            b"*3\r\n$3\r\nSET\r\n$7\r\nmessage\r\n$11\r\nhello world\r\n"
        );
    }

    #[tokio::test]
    async fn read_simple_string_frame() {
        let value = read_frame(b"+OK\r\n").await.unwrap();
        assert!(matches!(value, RespValue::Simple(s) if s == "OK"));
    }

    #[tokio::test]
    async fn read_integer_frame() {
        let value = read_frame(b":42\r\n").await.unwrap();
        assert!(matches!(value, RespValue::Integer(42)));
    }

    #[tokio::test]
    async fn read_bulk_string_frame() {
        let value = read_frame(b"$5\r\nhello\r\n").await.unwrap();
        assert!(matches!(value, RespValue::Bulk(s) if s == "hello"));
    }

    #[tokio::test]
    async fn read_nil_bulk_string_frame() {
        let value = read_frame(b"$-1\r\n").await.unwrap();
        assert!(matches!(value, RespValue::Nil));
    }

    #[tokio::test]
    async fn read_array_of_bulk_strings_frame() {
        let value = read_frame(b"*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        match value {
            RespValue::Array(items) => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], RespValue::Bulk(ref s) if s == "GET"));
                assert!(matches!(items[1], RespValue::Bulk(ref s) if s == "foo"));
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_rejects_nested_array_frame() {
        assert!(matches!(
            read_frame(b"*2\r\n$4\r\nECHO\r\n*1\r\n$2\r\nhi\r\n").await,
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[tokio::test]
    async fn read_rejects_invalid_length() {
        assert!(matches!(
            read_frame(b"$x\r\n").await,
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[tokio::test]
    async fn read_rejects_invalid_line_ending() {
        assert!(matches!(
            read_frame(b"+OK\n").await,
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[tokio::test]
    async fn read_rejects_invalid_bulk_terminator() {
        assert!(matches!(
            read_frame(b"$3\r\nfooXX").await,
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[tokio::test]
    async fn read_rejects_unsupported_prefix() {
        assert!(matches!(
            read_frame(b"_\r\n").await,
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[tokio::test]
    async fn read_rejects_null_array() {
        assert!(matches!(
            read_frame(b"*-1\r\n").await,
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[test]
    fn command_from_resp_ping() {
        assert!(matches!(
            command_from_resp_value(RespValue::Array(vec![RespValue::Bulk("PING".into())])),
            Ok(Command::Normal(NormalCommand::Ping(None)))
        ));
    }

    #[test]
    fn command_from_resp_set() {
        assert!(matches!(
            command_from_resp_value(RespValue::Array(vec![
                RespValue::Bulk("SET".into()),
                RespValue::Bulk("foo".into()),
                RespValue::Bulk("bar".into()),
            ])),
            Ok(Command::Normal(NormalCommand::Set { key, value })) if key == "foo" && value == "bar"
        ));
    }

    #[test]
    fn command_from_resp_rejects_empty_array() {
        assert!(matches!(
            command_from_resp_value(RespValue::Array(vec![])),
            Err(ProtocolError::EmptyCommand)
        ));
    }

    #[test]
    fn command_from_resp_rejects_top_level_bulk() {
        assert!(matches!(
            command_from_resp_value(RespValue::Bulk("PING".into())),
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[test]
    fn command_from_resp_rejects_nested_array_argument() {
        assert!(matches!(
            command_from_resp_value(RespValue::Array(vec![
                RespValue::Bulk("ECHO".into()),
                RespValue::Array(vec![RespValue::Bulk("hi".into())]),
            ])),
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    #[test]
    fn command_from_resp_rejects_nil_argument() {
        assert!(matches!(
            command_from_resp_value(RespValue::Array(vec![
                RespValue::Bulk("ECHO".into()),
                RespValue::Nil,
            ])),
            Err(ProtocolError::InvalidFrame(_))
        ));
    }

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_string()).collect()
    }

    async fn read_frame(bytes: &[u8]) -> Result<RespValue, ProtocolError> {
        let (mut tx, rx) = tokio::io::duplex(1024);
        tx.write_all(bytes).await.unwrap();
        tx.shutdown().await.unwrap();
        let mut reader = BufReader::new(rx);
        read_resp_value(&mut reader).await
    }
}
