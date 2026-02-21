# 🦀 RedRust

A lightweight, educational Redis clone built in Rust. RedRust implements core Redis data structures and commands with a focus on learning systems programming concepts.

## Features

### Data Types
- **Strings**: Store and retrieve text values
- **Lists**: Ordered collections with stack/queue operations

### Commands

#### String Commands
| Command | Description | Example |
|---------|-------------|---------|
| `SET key value [EX seconds]` | Set a string value with optional expiration | `SET name Master EX 60` |
| `GET key` | Get a string value | `GET name` |
| `DEL key` | Delete a key | `DEL name` |
| `KEYS` | List all non-expired keys | `KEYS` |
| `EXPIRE key seconds` | Set expiration on existing key | `EXPIRE name 30` |
| `TTL key` | Get remaining time to live | `TTL name` |
| `TYPE key` | Get the type of a key | `TYPE name` |

#### List Commands
| Command | Description | Example |
|---------|-------------|---------|
| `LPUSH key value [value ...]` | Push to the left (front) of list | `LPUSH mylist hello` |
| `RPUSH key value [value ...]` | Push to the right (back) of list | `RPUSH mylist world` |
| `LPOP key` | Pop from the left of list | `LPOP mylist` |
| `RPOP key` | Pop from the right of list | `RPOP mylist` |
| `LLEN key` | Get list length | `LLEN mylist` |
| `LRANGE key start stop` | Get range of elements | `LRANGE mylist 0 -1` |

#### Persistence Commands
| Command | Description |
|---------|-------------|
| `SAVE` | Synchronously save the database to disk |
| `BGSAVE` | Asynchronously save the database (non-blocking) |
| `LASTSAVE` | Get timestamp of last successful save |

#### Utility Commands
| Command | Description |
|---------|-------------|
| `PING` | Test server connectivity (returns PONG) |

## Installation

### Prerequisites
- Rust toolchain (install via [rustup](https://rustup.rs/))
- telnet or netcat (`nc`) for testing

### Building
```bash
git clone <your-repo-url>
cd redrust
cargo build --release
```

### Running
```bash
cargo run
```

The server will start on `127.0.0.1:6379`.

## Usage

### Connect to the Server

Using netcat:
```bash
nc 127.0.0.1 6379
```

Or send individual commands:
```bash
# Set a value
echo -e "SET greeting hello\r\n" | nc 127.0.0.1 6379

# Get a value
echo -e "GET greeting\r\n" | nc 127.0.0.1 6379

# Create a list
echo -e "LPUSH tasks buy-milk\r\n" | nc 127.0.0.1 6379
echo -e "RPUSH tasks walk-dog\r\n" | nc 127.0.0.1 6379
echo -e "LRANGE tasks 0 -1\r\n" | nc 127.0.0.1 6379

# Set expiration
echo -e "SET temp_value data EX 10\r\n" | nc 127.0.0.1 6379
echo -e "TTL temp_value\r\n" | nc 127.0.0.1 6379

# Save to disk
echo -e "SAVE\r\n" | nc 127.0.0.1 6379
```

### Response Format

RedRust uses the Redis RESP (REdis Serialization Protocol) format:

- Simple Strings: `+OK\r\n`
- Errors: `-ERR message\r\n`
- Integers: `:42\r\n`
- Bulk Strings: `$5\r\nhello\r\n`
- Arrays: `*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n`
- Null: `$-1\r\n`

### Session Example

```
$ nc 127.0.0.1 6379
SET user:1 "Master"
+OK
GET user:1
$6
Master
EXPIRE user:1 60
:1
TTL user:1
:59
LPUSH queue:tasks "task1"
:1
LPUSH queue:tasks "task2"
:2
LRANGE queue:tasks 0 -1
*2
$5
task2
$5
task1
SAVE
+OK
PING
+PONG
```

## Persistence

RedRust automatically loads data from `redrust.rdb` on startup and can save to disk with the `SAVE` or `BGSAVE` commands. The database is stored as JSON with expiration times preserved.

## Architecture

```
┌─────────────────────────────────────────┐
│           TCP Listener (6379)            │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│        Connection Handler (thread)       │
│  ┌───────────────────────────────────┐   │
│  │  Command Parser (RESP protocol)  │   │
│  └───────────────────────────────────┘   │
│  ┌───────────────────────────────────┐   │
│  │  Command Processor               │   │
│  │  - String ops, List ops, etc.     │   │
│  └───────────────────────────────────┘   │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│    Shared State (Arc<Mutex<Store>>)        │
│  ┌───────────────────────────────────┐   │
│  │  HashMap<String, Entry>          │   │
│  │  - Value (String | List)         │   │
│  │  - Expiration (optional)           │   │
│  └───────────────────────────────────┘   │
└─────────────────────────────────────────┘
        │                    │
        ▼                    ▼
┌──────────────┐    ┌──────────────┐
│   Cleanup    │    │ Persistence │
│  (1s timer)  │    │ (SAVE/LOAD) │
└──────────────┘    └──────────────┘
```

## What You'll Learn

Building RedRust covers these Rust concepts:

- **Ownership & Borrowing**: Managing shared state with `Arc` and `Mutex`
- **Concurrency**: Multi-threaded request handling
- **Enums**: Representing different data types (Value::String vs Value::List)
- **Pattern Matching**: Handling different commands cleanly
- **Error Handling**: Using `Result` and `Option` types
- **Serialization**: Using serde for persistence
- **Network Programming**: TCP sockets and the RESP protocol
- **Memory Safety**: How Rust prevents data races at compile time

## Testing

### Basic Commands
```bash
# Start server
cargo run

# In another terminal, run these tests:
echo -e "PING\r\n" | nc 127.0.0.1 6379  # Should return +PONG
echo -e "SET x 1\r\n" | nc 127.0.0.1 6379
echo -e "GET x\r\n" | nc 127.0.0.1 6379
echo -e "DEL x\r\n" | nc 127.0.0.1 6379
echo -e "GET x\r\n" | nc 127.0.0.1 6379  # Should return $-1
```

### List Operations
```bash
echo -e "LPUSH mylist a\r\n" | nc 127.0.0.1 6379
echo -e "LPUSH mylist b\r\n" | nc 127.0.0.1 6379
echo -e "RPUSH mylist c\r\n" | nc 127.0.0.1 6379
echo -e "LRANGE mylist 0 -1\r\n" | nc 127.0.0.1 6379  # b, a, c
echo -e "LPOP mylist\r\n" | nc 127.0.0.1 6379  # b
echo -e "RPOP mylist\r\n" | nc 127.0.0.1 6379  # c
echo -e "LLEN mylist\r\n" | nc 127.0.0.1 6379  # 1
```

### Expiration
```bash
echo -e "SET temp test EX 3\r\n" | nc 127.0.0.1 6379
echo -e "TTL temp\r\n" | nc 127.0.0.1 6379  # ~3
sleep 4
echo -e "GET temp\r\n" | nc 127.0.0.1 6379  # $-1 (expired)
```

## Benchmarking

RedRust includes benchmarking tools to measure performance:

### Quick Shell Benchmark
```bash
# Simple benchmark using netcat (slower due to connection per command)
source benchmark.sh
```

### Python Benchmark (Recommended)
```bash
# Uses persistent connections for accurate results
python3 benchmark.py
```

### Using redis-benchmark
If you have Redis installed, you can use the official benchmark tool:
```bash
# Simple test
redis-benchmark -p 6379 -t set,get -n 10000

# Test lists
redis-benchmark -p 6379 -t lpush,lrange -n 10000
```

### Benchmark Results

Tests run on Apple Silicon MacBook Air with persistent connections:

| Operation | Throughput | Latency (avg) |
|-----------|-----------|---------------|
| **SET** | ~25,000 req/sec | - |
| **GET** | ~36,000 req/sec | - |
| **LPUSH** | ~44,000 req/sec | - |
| **Mixed** | ~47,000 req/sec | - |
| **PING** | - | 0.11 ms |

**Key Findings:**
- Persistent connections are **~100x faster** than connection-per-command
- GET is faster than SET (read operations are simpler)
- List operations are highly efficient (~44K ops/sec)
- Sub-millisecond latency (P95: 0.25ms)

**Comparison:** Real Redis achieves 100K-200K+ ops/sec on similar hardware. RedRust reaches **20-40% of Redis performance**—remarkable for an educational project!

**Note:** The shell benchmark with `nc` showed ~250-350 req/sec because it opens a new TCP connection for every command. Always use persistent connections for real workloads.

## Future Enhancements

Possible additions to expand RedRust:

- [ ] **Sets**: `SADD`, `SMEMBERS`, `SISMEMBER`, `SREM`
- [ ] **Hashes**: `HSET`, `HGET`, `HGETALL`, `HDEL`
- [ ] **Sorted Sets**: `ZADD`, `ZRANGE`, `ZREVRANGE`
- [ ] **Pub/Sub**: `SUBSCRIBE`, `PUBLISH`, `UNSUBSCRIBE`
- [ ] **Transactions**: `MULTI`, `EXEC`, `DISCARD`
- [ ] **Connection Pooling**: Efficient client management
- [ ] **Replication**: Master-slave setup
- [ ] **AOF Persistence**: Append-only file logging

## License

MIT - Built for educational purposes. Not production-ready (Redis it is not 😄).

## Acknowledgments

Built as a learning project to understand:
- How Redis works under the hood
- Systems programming in Rust
- Network protocols and serialization

---

*"Simplicity is the ultimate sophistication." — Leonardo da Vinci*
