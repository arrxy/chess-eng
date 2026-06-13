# stress — staged load + state-consistency harness

Drives many full chess games in parallel over the real WebSocket protocol,
escalating the game count in stages, and verifies on **every move** that the
`state` frames are consistent:

- **Cross-client equality** — white's and black's `state` for the same ply are identical.
- **Full correctness** — each received board matches a position computed independently
  by an in-process mirror of the chess engine (`chess::board::game::Game`).

Each game opens two WebSocket connections (white + black), plays random legal moves
(seeded, reproducible), and compares both servers' boards to the mirror after each ply.

## Stages

Default ladder: `100, 200, 300, 1000, 5000, 10000`, then **+100 each stage until
failure**. A stage fails when:

- any **consistency** violation occurs (always fatal — `X` in the report), or
- the non-consistency failure rate exceeds `--fail-threshold` (default 2%).

On failure the driver prints the ceiling (last fully-healthy stage) and stops.

## Running

Start the server first (use `--release` for meaningful ceiling numbers), with Redis
and Mongo reachable:

```bash
cargo run --release -p chess          # in one terminal
```

Then, from a shell with a raised fd limit:

```bash
ulimit -n 100000
cargo run --release -p stress         # full ladder against localhost
```

Useful flags (`--help` for all):

```
--url ws://localhost:3000/ws    target; use wss://chess.socketlab.tech/ws for the cluster
--stages 1,50,200               override the ladder (smoke / quick runs)
--max-plies 30                  plies per game
--connect-concurrency 500       throttle on simultaneous connection setups
--fail-threshold 0.02           stage failure rate
--max-games 200000              safety stop for the +100 loop
--seed 1                        RNG base (per-game seed = base + index)
--session-cookie <val>          treat games as signed-in (see below)
```

Report columns: `ok/attempted | fail c(onnect) t(imeout) s(erver) X(consistency)
d(esync) | moves total + moves/sec | latency µs p50/p95/p99/max | wall`.

## Limitations / notes

- **File descriptors:** each game = 2 sockets. 10k games ≈ 20k+ fds — run
  `ulimit -n 100000` first.
- **Ephemeral ports:** one client IP → one destination `ip:port` tops out near
  ~28k connections (~14k games). Past that the *client* is the bottleneck, not the
  server. To push higher: generate load from several machines, or run the local
  server on multiple ports and target them round-robin. For true cluster numbers,
  run this from a separate box (or boxes) against the load balancer.
- **Anonymous vs signed-in:** anonymous games never get a Mongo document, so they do
  **not** exercise the move-stream / batch-flush / Mongo path — only the WS + Redis
  hot path. Pass `--session-cookie <value>` (a real `session` cookie minted once via
  Google login) to make games persist and exercise the full pipeline.
- **Verifying the checker bites:** to confirm the consistency assertions aren't a
  no-op, point `--url` at a deliberately stale/buggy server build, or temporarily skew
  the mirror — the run must report `X` (consistency) failures.
