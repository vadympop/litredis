[![Review Assignment Due Date](https://classroom.github.com/assets/deadline-readme-button-22041afd0340ce965d47ae6ef1cefeee28c7c493a6346c4f15d667ab976d596c.svg)](https://classroom.github.com/a/RHGu4AQi)

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
