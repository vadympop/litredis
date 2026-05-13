"""
litredis demo - shows the server working with the real redis client(python client in that case)

Requirements:  pip install redis
Start server:  cargo run
Then run:      python demo.py
"""

import sys
import time
import threading
import redis

HOST = "localhost"
PORT = 9736

RESET = "\033[0m"
BOLD = "\033[1m"
GREEN = "\033[32m"
YELLOW = "\033[33m"
CYAN = "\033[36m"
RED = "\033[31m"
DIM = "\033[2m"


def banner(text: str) -> None:
    width = 56
    print(f"\n{BOLD}{CYAN}{'─' * width}{RESET}")
    print(f"{BOLD}{CYAN}  {text}{RESET}")
    print(f"{BOLD}{CYAN}{'─' * width}{RESET}")


def step(label: str, value=None) -> None:
    marker = f"{GREEN}✓{RESET}"
    if value is None:
        print(f"  {marker}  {label}")
    else:
        print(f"  {marker}  {label:<38} {DIM}{value!r}{RESET}")


def connect(password: str | None = None) -> redis.Redis:
    return redis.Redis(
        host=HOST, port=PORT, password=password, decode_responses=True
    )


def demo_ping(r: redis.Redis) -> None:
    banner("PING / ECHO")
    result = r.ping()
    step("PING", result)

    result = r.execute_command("ECHO", "litredis!")
    step("ECHO litredis!", result)


def demo_set_get(r: redis.Redis) -> None:
    banner("SET / GET / DEL / EXISTS")
    r.delete("demo:greeting")

    r.set("demo:greeting", "Hello, world!")
    step("SET demo:greeting", "Hello, world!")

    val = r.get("demo:greeting")
    step("GET demo:greeting", val)

    exists = r.exists("demo:greeting")
    step("EXISTS demo:greeting", exists)

    r.delete("demo:greeting")
    missing = r.get("demo:greeting")
    step("GET after DEL", missing)

    exists_after = r.exists("demo:greeting")
    step("EXISTS after DEL", exists_after)


def demo_counters(r: redis.Redis) -> None:
    banner("INCR / DECR / INCRBY")
    r.delete("demo:hits")
    r.set("demo:hits", "0")

    for _ in range(3):
        r.incr("demo:hits")
    step("INCR x3", r.get("demo:hits"))

    # r.decr("demo:hits")
    # step("DECR", r.get("demo:hits"))

    r.incrby("demo:hits", 10)
    step("INCRBY 10", r.get("demo:hits"))

    r.incrby("demo:hits", -4)
    step("INCRBY -4", r.get("demo:hits"))


def demo_ttl(r: redis.Redis) -> None:
    banner("EXPIRE / TTL / PERSIST")
    r.set("demo:session", "abc123")
    r.expire("demo:session", 60)

    ttl = r.ttl("demo:session")
    step(f"TTL after EXPIRE 60s", ttl)

    r.persist("demo:session")
    step("TTL after PERSIST", r.ttl("demo:session"))

    r.set("demo:short", "bye", ex=2)
    step("SET with EX=2, TTL", r.ttl("demo:short"))

    print(f"  {DIM}  (waiting 2 s for key to expire…){RESET}")
    time.sleep(2.1)
    step("GET after expiry", r.get("demo:short"))


def demo_copy(r: redis.Redis) -> None:
    banner("COPY")
    r.delete("demo:src", "demo:dst", "demo:dst2")

    r.set("demo:src", "original-value")
    r.copy("demo:src", "demo:dst")
    step("COPY src -> dst", r.get("demo:dst"))
    step("src still intact", r.get("demo:src"))

    r.set("demo:dst2", "old-value")
    no_replace = r.copy("demo:src", "demo:dst2")
    step("COPY without REPLACE (blocked)", no_replace)

    replaced = r.copy("demo:src", "demo:dst2", replace=True)
    step("COPY with REPLACE", replaced)
    step("dst2 now holds", r.get("demo:dst2"))

    # COPY should propagate the TTL
    r.delete("demo:src_ttl", "demo:dst_ttl")
    r.set("demo:src_ttl", "v", ex=60)
    r.copy("demo:src_ttl", "demo:dst_ttl")
    step("COPY preserves TTL (dst TTL)", r.ttl("demo:dst_ttl"))


def demo_pubsub(r: redis.Redis) -> None:
    banner("PUB / SUB")

    received: list[str] = []

    def subscriber() -> None:
        sub_conn = connect()
        ps = sub_conn.pubsub()
        ps.subscribe("demo:channel")
        for msg in ps.listen():
            if msg["type"] == "message":
                received.append(msg["data"])
                break
        ps.unsubscribe("demo:channel")
        sub_conn.close()

    t = threading.Thread(target=subscriber, daemon=True)
    t.start()
    time.sleep(0.15)  # let subscriber register

    delivered = r.publish("demo:channel", "hello from publisher")
    step("PUBLISH (subscribers notified)", delivered)

    t.join(timeout=2)
    step("Subscriber received", received[0] if received else "<nothing>")


def demo_auth() -> None:
    banner("AUTH (password-protected server)")
    print(f"  {DIM}  Skipped - start server with --password <pw> to test.{RESET}")
    print(f"  {DIM}  Example: cargo run -- --password secret{RESET}")
    print(f"  {DIM}  Then connect: redis.Redis(password='secret'){RESET}")


def main() -> None:
    print(f"\n{BOLD}litredis demo{RESET}  {DIM}connecting to {HOST}:{PORT}…{RESET}")

    try:
        r = connect()
        r.ping()
    except redis.exceptions.ConnectionError:
        print(
            f"\n{RED}ERROR:{RESET} Cannot reach server on {HOST}:{PORT}.\n"
            f"  Run  {BOLD}cargo run{RESET}  first, then retry.\n"
        )
        sys.exit(1)

    demo_ping(r)
    demo_set_get(r)
    demo_counters(r)
    demo_ttl(r)
    demo_copy(r)
    demo_pubsub(r)
    demo_auth()

    print(f"\n{BOLD}{GREEN}All demo sections completed successfully.{RESET}\n")


if __name__ == "__main__":
    main()
