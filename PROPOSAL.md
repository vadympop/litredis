# Asynchronous in-memory key–value database server inspired by Redis

## Authors
 
- Yurii Oliinyk
- Vadym Popovych

## Introduction
We are building a simplified in-memory key-value server inspired by Redis. It accepts client connections over TCP, stores and retrieves data through simple commands, and supports real-time messaging between clients via publish/subscribe. Data is persisted to disk so it survives server restarts.

## What problem does it solve?
Applications often need a fast, shared place to store temporary data - user sessions, counters, messages between services. A dedicated in-memory server handles this much faster than a traditional database, because there is no disk access on every read or write.

## What do we hope to learn?
We want to gain hands-on experience with async Rust and networking. Building a Redis-inspired server also gives us a chance to understand how such systems work under the hood, not just how to use them.

## Requirements
 
- Accept multiple simultaneous client connections over TCP
- Communicate using a well-defined text-based protocol
- Support the following core commands:
  - `SET key value` - store a value under a key
  - `GET key` - retrieve the value for a key
  - `DEL key` - remove a key
  - `EXISTS key` - check whether a key is present
  - `EXPIRE key seconds` - set a time-to-live on a key
  - `TTL key` - query the remaining time-to-live
  - `INCR / DECR key` - increment or decrement a numeric value
- Automatically remove keys once their TTL has elapsed
- Support publish/subscribe messaging:
  - `SUBSCRIBE channel` - start receiving messages on a channel
  - `PUBLISH channel message` - send a message to all subscribers of a channel
  - `UNSUBSCRIBE channel` - stop receiving messages
- Persist all data to disk on shutdown and restore it on startup

## Dependencies

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime, TCP server, concurrent tasks |
| `bytes` | Byte buffer utilities for reading and parsing data from TCP streams |
| `dashmap` | Concurrent HashMap for the key-value store, allows multiple clients to read and write without blocking each other |
| `serde` + `serde_json` | Serializing the in-memory store to JSON for disk persistence |
| `anyhow` | Lightweight error handling throughout the application |
