use std::io;

use clap::Parser;
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

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let addr = format!("{}:{}", args.host, args.port);
    let stream = TcpStream::connect(addr).await?;
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    let reader_task = tokio::spawn(async move { read_loop(&mut reader).await });

    let stdin = tokio::io::stdin();
    let mut stdin_reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = stdin_reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        write_half.write_all(line.as_bytes()).await?;
        write_half.flush().await?;
    }

    write_half.shutdown().await?;

    match reader_task.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(io::Error::other(e)),
    }
}

async fn read_loop<R>(reader: &mut R) -> io::Result<()>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut stdout = tokio::io::stdout();

    loop {
        let frame = match read_resp_frame(reader).await {
            Ok(frame) => frame,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e),
        };

        stdout.write_all(&frame).await?;
        stdout.flush().await?;
    }
}

async fn read_resp_frame<R>(reader: &mut R) -> io::Result<Vec<u8>>
where
    R: AsyncBufRead + AsyncRead + Unpin,
{
    let mut raw = Vec::new();
    let mut remaining: Vec<i64> = Vec::new();

    loop {
        let mut line = Vec::new();
        let n = reader.read_until(b'\n', &mut line).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ));
        }

        if line.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "empty frame"));
        }

        raw.extend_from_slice(&line);

        match line[0] {
            b'+' | b'-' | b':' => consume_item(&mut remaining),
            b'$' => {
                let len = parse_i64(&line[1..])?;
                if len >= 0 {
                    let mut body = vec![0u8; len as usize + 2];
                    reader.read_exact(&mut body).await?;
                    raw.extend_from_slice(&body);
                }
                consume_item(&mut remaining);
            }
            b'*' => {
                let count = parse_i64(&line[1..])?;
                if !remaining.is_empty() {
                    consume_item(&mut remaining);
                }
                if count > 0 {
                    remaining.push(count);
                }
            }
            _ => {
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
    if let Some(top) = remaining.last_mut() {
        *top -= 1;
    }

    while matches!(remaining.last(), Some(0)) {
        remaining.pop();
    }
}

fn parse_i64(bytes: &[u8]) -> io::Result<i64> {
    let s = std::str::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non-utf8 length"))?;
    let trimmed = s.trim_end_matches(['\r', '\n']);
    trimmed
        .parse::<i64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid length"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};

    async fn read_frame_from_bytes(bytes: &[u8]) -> Vec<u8> {
        let (mut tx, rx) = tokio::io::duplex(1024);
        tx.write_all(bytes).await.unwrap();
        tx.shutdown().await.unwrap();
        let mut reader = BufReader::new(rx);
        read_resp_frame(&mut reader).await.unwrap()
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
}
