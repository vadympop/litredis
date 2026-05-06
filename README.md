## Usage

Start the server:

```bash
cargo run --bin redis-app
```

Start the interactive client (one line = one command):

```bash
cargo run --bin litredis-cli -- --host 127.0.0.1 --port 9736
```

Examples:

```text
PING
SET key "hello world"
GET key
SUBSCRIBE news
```
