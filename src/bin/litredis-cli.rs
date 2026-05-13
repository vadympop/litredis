use std::io;

use clap::Parser;
use redis_app::protocol::RespValue;
use redis_app::protocol::resp::encode_command;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Parser, Debug)]
#[command(name = "litredis-cli", about = "Interactive client for litredis")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 9736)]
    port: u16,
}

const MAX_ARRAY_LEN: usize = 1_000_000;

#[tokio::main]
async fn main() -> io::Result<()> {
    // parse args and open tcp connection
    let args = Args::parse();
    let addr = format!("{}:{}", args.host, args.port);
    let stream = TcpStream::connect(addr).await?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // run read loop in background so stdin can stay responsive
    let reader_task = tokio::spawn(async move { read_loop(&mut reader).await });

    let stdin = tokio::io::stdin();
    let mut stdin_reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = stdin_reader.read_line(&mut line).await?;
        if n == 0 {
            // eof on stdin
            break;
        }

        let args = split_args(line.trim_end()).map_err(io::Error::other)?;
        if args.is_empty() {
            continue;
        }

        let command = encode_command(&args);
        write_half.write_all(&command).await?;
        write_half.flush().await?;
    }

    write_half.shutdown().await?;

    // abort reader task so we do not hang on idle server
    reader_task.abort();

    // treat connection close as a clean exit
    match reader_task.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(e) if e.is_cancelled() => Ok(()),
        Err(e) => Err(io::Error::other(e)),
    }
}

fn split_args(input: &str) -> Result<Vec<String>, String> {
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
                    None => return Err("unterminated quoted string".into()),
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

async fn read_loop<R>(reader: &mut R) -> io::Result<()>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut stdout = tokio::io::stdout();

    loop {
        // read one full resp frame
        let frame = match read_resp_frame(reader).await {
            Ok(frame) => frame,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        };

        // parse resp bytes into typed reply then render
        let reply = parse_resp_frame(&frame)?;
        let output = format_reply(&reply);

        stdout.write_all(output.as_bytes()).await?;
        stdout.flush().await?;
    }
}

async fn read_resp_frame<R>(reader: &mut R) -> io::Result<Vec<u8>>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    // accumulate raw bytes until a full frame is read
    let mut raw = Vec::new();
    let mut remaining: Vec<i64> = Vec::new();

    loop {
        let mut line = Vec::new();
        let n = reader.read_until(b'\n', &mut line).await?;
        if n == 0 {
            // socket closed mid frame
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ));
        }

        if line.is_empty() {
            // guard against protocol desync
            return Err(io::Error::new(io::ErrorKind::InvalidData, "empty frame"));
        }

        raw.extend_from_slice(&line);

        match line[0] {
            b'+' | b'-' | b':' => consume_item(&mut remaining),
            b'$' => {
                // read bulk payload when length is known
                let len = parse_i64(&line[1..])?;
                if len >= 0 {
                    let mut body = vec![0u8; len as usize + 2];
                    reader.read_exact(&mut body).await?;
                    raw.extend_from_slice(&body);
                }
                consume_item(&mut remaining);
            }
            b'*' => {
                // track nested array sizes
                let count = parse_i64(&line[1..])?;
                let count = checked_array_len(count)?;
                if !remaining.is_empty() {
                    consume_item(&mut remaining);
                }
                if let Some(count) = count
                    && count > 0
                {
                    let count = i64::try_from(count).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "array length overflow")
                    })?;
                    remaining.push(count);
                }
            }
            _ => {
                // unknown resp prefix
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unknown RESP prefix",
                ));
            }
        }

        if remaining.is_empty() {
            break;
        }
    }

    Ok(raw)
}

fn consume_item(remaining: &mut Vec<i64>) {
    // decrement top array counter and pop finished arrays
    if let Some(top) = remaining.last_mut() {
        *top -= 1;
    }

    while matches!(remaining.last(), Some(0)) {
        remaining.pop();
    }
}

fn parse_i64(bytes: &[u8]) -> io::Result<i64> {
    // parse an integer line without crlf
    let s = std::str::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-utf8 length"))?;
    let trimmed = s.trim_end_matches(['\r', '\n']);
    trimmed
        .parse::<i64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid length"))
}

fn checked_array_len(count: i64) -> io::Result<Option<usize>> {
    // validate and cap array length
    if count == -1 {
        return Ok(None);
    }
    if count < -1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid array length",
        ));
    }
    let count = usize::try_from(count)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "array length overflow"))?;
    if count > MAX_ARRAY_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "array too large",
        ));
    }
    Ok(Some(count))
}

fn parse_resp_frame(bytes: &[u8]) -> io::Result<RespValue> {
    // parse exactly one resp value from the frame
    let mut idx = 0;
    let value = parse_resp_value(bytes, &mut idx)?;
    if idx != bytes.len() {
        // extra bytes mean framing error
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "trailing bytes in frame",
        ));
    }
    Ok(value)
}

fn parse_resp_value(bytes: &[u8], idx: &mut usize) -> io::Result<RespValue> {
    if *idx >= bytes.len() {
        // guard against empty or truncated frame
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "unexpected end of frame",
        ));
    }

    match bytes[*idx] {
        b'+' => {
            // simple string
            *idx += 1;
            let line = read_line(bytes, idx)?;
            let s = bytes_to_string(line)?;
            Ok(RespValue::Simple(s))
        }
        b'-' => {
            // error string
            *idx += 1;
            let line = read_line(bytes, idx)?;
            let s = bytes_to_string(line)?;
            Ok(RespValue::Error(s))
        }
        b':' => {
            // integer reply
            *idx += 1;
            let line = read_line(bytes, idx)?;
            let n = parse_i64(line)?;
            Ok(RespValue::Integer(n))
        }
        b'$' => {
            // bulk string or nil
            *idx += 1;
            let line = read_line(bytes, idx)?;
            let len = parse_i64(line)?;
            if len < -1 {
                // resp bulk length must be -1 or >= 0
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid bulk length",
                ));
            }
            if len == -1 {
                // nil bulk string
                return Ok(RespValue::Nil);
            }
            let len = len as usize;
            if *idx + len + 2 > bytes.len() {
                // payload not fully received
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "bulk string truncated",
                ));
            }
            let data = &bytes[*idx..*idx + len];
            *idx += len;
            if bytes.get(*idx) != Some(&b'\r') || bytes.get(*idx + 1) != Some(&b'\n') {
                // enforce bulk payload terminator
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid bulk terminator",
                ));
            }
            *idx += 2;
            let s = bytes_to_string(data)?;
            Ok(RespValue::Bulk(s))
        }
        b'*' => {
            // array or nil array
            *idx += 1;
            let line = read_line(bytes, idx)?;
            let count = parse_i64(line)?;
            let count = match checked_array_len(count)? {
                Some(count) => count,
                None => {
                    // nil array
                    return Ok(RespValue::Nil);
                }
            };

            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                // parse nested values recursively
                items.push(parse_resp_value(bytes, idx)?);
            }
            Ok(RespValue::Array(items))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unknown RESP prefix",
        )),
    }
}

fn read_line<'a>(bytes: &'a [u8], idx: &mut usize) -> io::Result<&'a [u8]> {
    if *idx >= bytes.len() {
        // guard against reading past buffer
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "unexpected end of frame",
        ));
    }

    let start = *idx;
    let mut end = None;
    // scan until newline
    for (i, byte) in bytes.iter().enumerate().skip(start) {
        if *byte == b'\n' {
            end = Some(i);
            break;
        }
    }

    let end =
        end.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing line ending"))?;
    if end == start || bytes[end - 1] != b'\r' {
        // require crlf terminator
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid line ending",
        ));
    }

    *idx = end + 1;
    Ok(&bytes[start..end - 1])
}

fn bytes_to_string(bytes: &[u8]) -> io::Result<String> {
    // enforce utf8 for display
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-utf8 string"))
}

fn format_reply(reply: &RespValue) -> String {
    // join lines and ensure trailing newline
    let mut out = format_reply_lines(reply, false).join("\n");
    out.push('\n');
    out
}

fn format_reply_lines(reply: &RespValue, in_array: bool) -> Vec<String> {
    // convert reply into printable lines
    match reply {
        RespValue::Simple(s) => vec![format_string(s, in_array)],
        RespValue::Error(s) => vec![format!("(error) {}", s)],
        RespValue::Integer(n) => vec![format!("(integer) {}", n)],
        RespValue::Bulk(s) => vec![format_string(s, in_array)],
        RespValue::Array(items) => format_array_lines(items),
        RespValue::Nil => vec!["(nil)".to_string()],
    }
}

fn format_array_lines(items: &[RespValue]) -> Vec<String> {
    if items.is_empty() {
        return vec!["(empty array)".to_string()];
    }

    let mut lines = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let prefix = format!("{}) ", idx + 1);
        let item_lines = format_reply_lines(item, true);
        if item_lines.is_empty() {
            // keep numbering even for empty sub-values
            lines.push(prefix.trim_end().to_string());
            continue;
        }

        lines.push(format!("{}{}", prefix, item_lines[0]));
        let pad = " ".repeat(prefix.len());
        for line in item_lines.iter().skip(1) {
            // align wrapped lines under the first item text
            lines.push(format!("{}{}", pad, line));
        }
    }

    lines
}

fn format_string(s: &str, in_array: bool) -> String {
    // quote strings only when nested in arrays
    if !in_array {
        return s.to_string();
    }

    // escape backslashes and quotes for array display
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};

    async fn read_frame_from_bytes(bytes: &[u8]) -> Vec<u8> {
        // helper to feed bytes into the reader
        let (mut tx, rx) = tokio::io::duplex(1024);
        tx.write_all(bytes).await.unwrap();
        tx.shutdown().await.unwrap();
        let mut reader = BufReader::new(rx);
        read_resp_frame(&mut reader).await.unwrap()
    }

    #[test]
    fn split_plain_command() {
        assert_eq!(split_args("SET foo bar").unwrap(), ["SET", "foo", "bar"]);
    }

    #[test]
    fn split_quoted_argument() {
        assert_eq!(
            split_args(r#"SET msg "hello world""#).unwrap(),
            ["SET", "msg", "hello world"]
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
    fn split_empty_line() {
        assert!(split_args("   ").unwrap().is_empty());
    }

    #[test]
    fn split_unterminated_quote() {
        assert!(split_args(r#"ECHO "oops"#).is_err());
    }

    #[tokio::test]
    async fn read_simple_string_frame() {
        let input = b"+PONG\r\n";
        let out = read_frame_from_bytes(input).await;
        assert_eq!(out, input);
    }

    #[tokio::test]
    async fn read_bulk_string_frame() {
        let input = b"$5\r\nhello\r\n";
        let out = read_frame_from_bytes(input).await;
        assert_eq!(out, input);
    }

    #[tokio::test]
    async fn read_array_then_next_frame() {
        let input = b"*2\r\n$4\r\npong\r\n$2\r\nhi\r\n:1\r\n";
        let (mut tx, rx) = tokio::io::duplex(1024);
        tx.write_all(input).await.unwrap();
        tx.shutdown().await.unwrap();
        let mut reader = BufReader::new(rx);

        let first_expected = b"*2\r\n$4\r\npong\r\n$2\r\nhi\r\n";
        let second_expected = b":1\r\n";

        let first = read_resp_frame(&mut reader).await.unwrap();
        let second = read_resp_frame(&mut reader).await.unwrap();

        assert_eq!(first, first_expected);
        assert_eq!(second, second_expected);
    }

    #[test]
    fn parse_integer_reply() {
        let input = b":42\r\n";
        let reply = parse_resp_frame(input).unwrap();
        assert!(matches!(reply, RespValue::Integer(42)));
    }

    #[test]
    fn parse_nil_bulk_reply() {
        let input = b"$-1\r\n";
        let reply = parse_resp_frame(input).unwrap();
        assert!(matches!(reply, RespValue::Nil));
    }

    #[test]
    fn parse_array_with_mixed_types() {
        let input = b"*3\r\n$3\r\nfoo\r\n$-1\r\n:7\r\n";
        let reply = parse_resp_frame(input).unwrap();

        match reply {
            RespValue::Array(items) => {
                assert_eq!(items.len(), 3);
                assert!(matches!(items[0], RespValue::Bulk(ref s) if s == "foo"));
                assert!(matches!(items[1], RespValue::Nil));
                assert!(matches!(items[2], RespValue::Integer(7)));
            }
            _ => panic!("expected array reply"),
        }
    }

    #[test]
    fn format_array_output() {
        let reply = RespValue::Array(vec![
            RespValue::Bulk("a".to_string()),
            RespValue::Integer(2),
        ]);
        let out = format_reply(&reply);
        assert_eq!(out, "1) \"a\"\n2) (integer) 2\n");
    }
}
