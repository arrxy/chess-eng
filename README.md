# Parallel Chess : a horizontally-scalable real-time multiplayer chess server

A multiplayer chess server written in **Rust** that started as a hobby engine and
grew into a distributed system: stateless app servers behind a load balancer, live
game state sharded across **Valkey** (a Redis fork), durable history in **MongoDB**,
cross-server move delivery over pub/sub, optimistic concurrency, node-failure
recovery, and a React frontend; all deployed on DigitalOcean with autoscaling and
CI/CD.

The chess was never the hard part. *Where the game lives and what happens when a
server dies* was. For the full design story and the load-testing campaign, read
**[`findings/finding.md`](findings/finding.md)**.

---

## Features

- ♟️ **Full chess rules** : legal move generation, check/checkmate/stalemate,
  castling, en passant, promotion. Validated against 331 real games cross-checked
  with `python-chess`.
- **Real-time multiplayer** over WebSockets; play anonymously or signed in with
  Google.
- **Stateless, scalable servers** : game state lives in Valkey, so any server can
  serve any game and servers are disposable.
- **Sharded state** across N Valkey nodes by a deterministic hash of the game id.
- **Optimistic concurrency** : a versioned compare-and-set per move (no per-move
  distributed lock).
- **Batched persistence** : moves flow through a Valkey Stream and are bulk-written
  to MongoDB; finished games are replayable.
- **Reconnect, forfeit, and a 60-minute rejoin window**; inactive games are swept.
- **Failure recovery** : heartbeats + peer monitoring claim and finalize an orphaned
  game when a server dies.
- **A load tester that's also a correctness oracle** : it runs a second copy of the
  engine and verifies every move's state on both clients.

---

## Architecture at a glance

```
        Browser (React + WebSocket)
                  │ wss://
                  ▼
        Load Balancer (TLS)
   ┌──────────────┼──────────────┐
   ▼              ▼              ▼
 App server 1  App server 2 … App server N     (autoscaled, stateless)
   └──────────────┼──────────────┘
        ┌─────────┴─────────┐
        ▼                   ▼
   Valkey shards         MongoDB
   (live state,          (users, finished
    pub/sub, locks,       games, history)
    move stream)
```

A move travels two independent planes: **real-time delivery** (pub/sub + live state)
and **durable persistence** (stream → batch → MongoDB). See
[`findings/finding.md`](findings/finding.md) for the deep dive.

---

## Tech stack

| Layer | Choice |
|---|---|
| Language | Rust (edition 2024) |
| HTTP / WebSocket | Axum |
| Live state / pub-sub / streams | Valkey (Redis-compatible), via `bb8-redis` |
| Durable storage | MongoDB |
| Auth | Google Identity Services + opaque session cookies |
| Frontend | React + Vite |
| Load tester | Rust + `tokio-tungstenite` (workspace crate `stress/`) |
| Infra | DigitalOcean (autoscale pool, LB, managed DBs, registry) |

---

## Project structure

```
src/
  board/        chess engine — Board, Game, move rules
  pieces/       Piece trait + one file per piece type
  db/           MongoDB layer (mongo.rs, *_schema.rs)
  redis_state/  shared state: RedisGameState, sharding, CAS, pub/sub, stream, lock
  server/       Axum state, WebSocket handler (ws.rs), auth
  background/   heartbeat, batch_flush, sweeper tasks
  routes/, service/   HTTP routing + handlers
  lib.rs        exposes the engine as a library (reused by the load tester)
  main.rs       server binary
frontend/       React + Vite app (built into static/dist, embedded via include_str!)
stress/         load-testing + consistency-verification harness (workspace crate)
documents/      deployment runbook
findings/       the write-up: design + load-testing case study
tests/          fixtures (famous_games.tsv)
```

---

## Getting started (local)

### Prerequisites
- Rust (1.88+), Node.js, and reachable **MongoDB** + **Valkey/Redis** instances.

### 1. Build the frontend (required before `cargo build` — it's embedded)
```bash
cd frontend && npm install && npm run build   # emits static/dist/
```

### 2. Configure
```bash
cp .env.example .env   # then edit
```

### 3. Run
```bash
cargo run            # http://localhost:3000
# or: cargo run --release
```

Open two browser tabs: create a game in one, copy the game id, join from the other.

### Frontend dev with hot reload
```bash
cd frontend && npm run dev   # proxies API/WS to :3000
```

---

## Configuration (`.env`)

| Variable | Required | Description |
|---|----------|---|
| `MONGODB_URI` | Y        | MongoDB connection string |
| `MONGODB_DB` | Y        | Database name |
| `REDIS_URL` | Y/N      | Single Valkey/Redis node |
| `REDIS_URLS` | Y/N      | Comma-separated shard nodes; **takes precedence** over `REDIS_URL`. First node is the coordination node (move stream, heartbeats). All servers must use the same list in the same order. |
| `SERVER_ID` | –        | Stable per-instance id (defaults to a random UUID); use the host/pod name in production |
| `GOOGLE_CLIENT_ID` | –        | Enables Google sign-in (hidden when unset) |
| `PORT` | –        | Defaults to `3000` |

\* Provide either `REDIS_URL` (single node) or `REDIS_URLS` (sharded).

---

## WebSocket protocol

JSON text frames on `GET /ws`:

| Direction | Messages |
|---|---|
| Client → | `create`, `join`, `reconnect`, `move`, `moves`, `forfeit` |
| Server → | `joined`, `state`, `possible_moves`, `game_over`, `opponent_disconnected`, `opponent_reconnected`, `error` |

The `state` frame is the full snapshot (board, turn, status, players, captured, last
move) sent to both players after every move.

---

## Testing

### Engine tests
```bash
cargo test            # includes the 331-game replay suite
```

### Load + consistency harness
A staged load tester that drives full games and, using an in-process mirror of the
engine, **verifies every move's state on both clients** (cross-client equality +
correctness). See [`stress/README.md`](stress/README.md).

```bash
# start the server, then in another shell:
ulimit -n 100000
cargo run --release -p stress -- \
  --url ws://localhost:3000/ws --stages 100,500,1000 --max-plies 60
```

Report columns: `ok/attempted | fail c(onnect) t(imeout) s(erver) X(consistency)
d(esync) | moves/sec | latency p50/p95/p99 | wall`. Any `X` (consistency) failure is
fatal.

---

## Deployment

Runs from a single 3-stage Docker image on a DigitalOcean autoscale pool behind a
load balancer, with managed MongoDB + Valkey, a golden snapshot + Watchtower for
auto-updates, and GitHub Actions for CI/CD. Full runbook:
**[`documents/deployment.md`](documents/deployment.md)**.

```bash
docker build -t chess .
docker run -p 3000:3000 --env-file .env chess
```

---

## Further reading

- **[`findings/finding.md`](findings/finding.md)** : the design + load-testing case
  study (HLD, LLD, and what the load tests actually taught).
- **[`documents/deployment.md`](documents/deployment.md)** : the production
  deployment runbook.
- **[`stress/README.md`](stress/README.md)** : the load-testing harness.

---

## License

This project is licensed under the [MIT License](LICENSE) - see the [LICENSE](LICENSE) file for details.

