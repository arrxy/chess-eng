# I Couldn't Overwhelm My Chess Server. I Could Only Overwhelm My Load Tester.

*A case study in taking a toy chess engine from a single in-memory process to a
sharded, fault-tolerant, distributed system in Rust — and load-testing it until the
only thing left standing was the load generator.*

---

## TL;DR

- Took a hobby Rust chess engine and turned it into a horizontally-scalable,
  fault-tolerant multiplayer server. The chess was never the hard part; **where the
  game lives and what happens when a server dies** was.
- **State is externalized to Valkey** (a Redis fork) so app servers are stateless
  and disposable; any server can serve any game. MongoDB holds durable history.
- Moves travel two independent planes: **real-time delivery** via Valkey pub/sub,
  and **durable persistence** via a Valkey Stream batched into MongoDB.
- Swapped a per-move **distributed lock for optimistic compare-and-set** (a versioned
  Lua write), halving per-move datastore ops — ~3× the single-node game ceiling.
- **Sharded game state across Valkey nodes** by a deterministic hash of the game id,
  so state, locks, *and* pub/sub channels all co-locate and cross-server delivery
  still works.
- Deployed on DigitalOcean (autoscale pool, LB-by-tag, golden snapshot + Watchtower,
  CI/CD).
- Built a load tester that **independently re-derives expected state from a second
  copy of the engine** and asserts on it every move. It found a real distributed
  bug on its first run, and **consistency never failed** across millions of moves.
- The punchline: every ceiling I hit was on the **client/edge** (laptop RTT, the load
  balancer's TLS handshake rate, the generator's single core), never the sharded
  backend. You can't stress a scaled-out server from one box — the box gives out
  first.

---

## 1. Introduction

This started as a hobby chess engine written in Rust: a board, some pieces, legal
move generation. The interesting part was never the chess. It was the question that
shows up the moment two people want to play over a network:

> Where does the game live, and what happens when the server holding it dies?

A naive implementation keeps each game in the server process's memory. That works
until you have more traffic than one machine can hold, or until that machine
restarts and every in-progress game vanishes. The journey from "a `HashMap` in one
process" to "game state that any of N servers can serve, that survives a node
death, and that scales by adding nodes" is the actual engineering, and it is the
subject of this article.

We cover the high-level design (HLD), the low-level design (LLD) of each
subsystem, the deployment on DigitalOcean, and a load-testing campaign whose main
finding was a lesson in its own right: **in a horizontally-scaled system, it is
surprisingly hard to build a load generator powerful enough to actually stress the
server.**

Stack: Rust (edition 2024), Axum for HTTP/WebSocket, MongoDB for durable history,
Valkey (an open-source Redis fork) for shared live state, a React/Vite frontend,
and DigitalOcean for infrastructure.

---

## 2. High-Level Design

### 2.1 The shape of the problem

Chess is turn-based and stateful. A game is a small but non-trivial blob of state
(board, whose turn, castling rights, en-passant square, captured pieces, move
history) that two clients mutate alternately, in real time, with strict
correctness requirements — an illegal or lost move is unacceptable.

Three forces pull on the design:

1. **Real-time delivery** — when White moves, Black must see it within
   milliseconds.
2. **Durability** — finished games must be persisted for history/replay.
3. **Scale and resilience** — many concurrent games across many servers, and no
   single server's death should lose a game.

### 2.2 Architecture overview

```
                          Browser (React + WebSocket)
                                     │  wss://
                                     ▼
                         Regional Load Balancer (TLS)
              ┌──────────────────────┼──────────────────────┐
              ▼                      ▼                      ▼
        App server 1           App server 2     …     App server N   (autoscaled)
        (Axum + WS)            (Axum + WS)            (Axum + WS)
              │   stateless except for local socket handles   │
              └───────────────┬───────────────┬───────────────┘
                              │               │
                  shared live state      durable history
                              ▼               ▼
                   ┌──────────────────┐   ┌──────────┐
                   │  Valkey shards   │   │ MongoDB  │
                   │  (game state,    │   │ (users,  │
                   │   pub/sub, locks,│   │  finished│
                   │   move stream)   │   │  games)  │
                   └──────────────────┘   └──────────┘
```

The key architectural decision: **app servers hold almost no authoritative
state.** The live game lives in Valkey; the only thing an app server keeps in
memory is the set of local WebSocket sender handles for the clients currently
connected to *it*. Any server can serve any game by reading its state from Valkey.
This is what makes the servers horizontally scalable and individually disposable.

### 2.3 The two data planes

A move travels two completely independent paths, and separating them is central to
the design:

- **The propagation plane** (real-time, every game): apply the move to the live
  state in Valkey, then deliver the new state to the opponent — directly if they
  share a server, or via Valkey pub/sub if they are on a different server.
- **The persistence plane** (durable, signed-in games only): enqueue the move on a
  Valkey Stream; a background worker batches stream entries and bulk-writes them to
  MongoDB.

Anonymous games use only the propagation plane and are ephemeral. Signed-in games
use both. Conflating these two planes is a common mistake; keeping them separate is
why an empty move-stream is *healthy* (it drains to Mongo) rather than a bug.

### 2.4 Scaling model

Everything scales horizontally:

- **App servers**: stateless, behind a load balancer, added/removed by an
  autoscaler.
- **Valkey**: sharded by game id across N independent nodes — each game (and all
  its keys) lives on one node, chosen by a deterministic hash, so load spreads
  across nodes.
- **MongoDB**: write amplification is absorbed by batching; reads are paginated
  history queries.

---

## 3. Low-Level Design

### 3.1 The chess engine

The engine is deliberately simple and has no knowledge of networking or storage.

- **Board**: an `8×8` grid, `Vec<Vec<Option<Box<dyn Piece>>>>`, indexed by a
  `Position { x, y }`.

  > A note for the Rust crowd before you reach for the comments section: a nested
  > `Vec` of heap-allocated trait objects is *not* how you'd build a fast engine.
  > It's pointer-chasing and cache-hostile; a flat `[Option<Piece>; 64]` enum array
  > (or bitboards for a serious one) would be contiguous and far friendlier to the
  > cache. This was a deliberate trade of performance for developer velocity —
  > trait objects made the per-piece move logic pleasant to write, and **compute was
  > never the bottleneck.** The board mutates a handful of times per game; the
  > limits we actually hit were always I/O, network, and shared state, never the
  > engine. If move generation had been on the hot path, this is the first thing
  > that would have changed.
- **`Piece` trait**: each piece type implements
  `possible_moves(from, board) -> Vec<Position>` — pseudo-legal geometry plus
  blocking/capture rules. Trait objects (`Box<dyn Piece>`) keep the board generic
  over piece type.
- **`Game`**: owns the board, two players, and whose turn it is. `make_move`
  validates turn ownership, generates pseudo-legal moves, simulates the move on a
  cloned board, rejects it if it leaves the mover in check, then commits. Castling,
  en passant, and promotion are handled here.
- **`GameStatus`**: `Ongoing | Check | Checkmate | Stalemate`, computed by testing
  whether the side to move has any legal reply.

The engine was validated against 331 real games (World Championship matches and
classic miniatures) replayed move-by-move and cross-checked against `python-chess`.
This matters later: the load-test harness reuses this exact engine to independently
verify server correctness.

### 3.2 The WebSocket protocol

A small JSON protocol over a single WebSocket per client:

| Direction | Message |
|---|---|
| C→S | `{"type":"create"}` / `{"type":"join","game_id"}` / `{"type":"reconnect",…}` |
| C→S | `{"type":"move","from","to","promotion"?}` / `{"type":"moves","x","y"}` / `{"type":"forfeit"}` |
| S→C | `{"type":"joined",…}` / `{"type":"state",…}` / `{"type":"possible_moves",…}` |
| S→C | `{"type":"game_over",…}` / `{"type":"opponent_disconnected"}` / `{"type":"opponent_reconnected"}` |

The `state` frame is the full snapshot — board, turn, status, players, captured
pieces, last move — sent to both players after every move.

### 3.3 Shared live state in Valkey

The authoritative live state is a serializable struct, `RedisGameState`:

- Board as an `8×8` grid of `{piece_type, color}` (trait objects can't be
  serialized, so we store the enum pair and reconstruct `Box<dyn Piece>` via a
  factory on read).
- Turn, the four castling-rights booleans, en-passant target.
- Move history, captured-piece lists.
- Player identities, `started`/`persisted` flags, timestamps.
- `mongo_game_id` (set once both players join), and `final_status` (a crash-safety
  marker, see §3.7).
- A `version` integer for optimistic concurrency (see §3.5).

It is stored as a Valkey hash at key `game:{id}`, field `state` (JSON), with a TTL
that doubles as the inactivity timer. Reconstructing a live `Game` from this struct
("hydration") lets any server resume any game.

### 3.4 Move propagation and cross-server delivery

When a move is processed on server A:

1. Load `game:{id}` state, hydrate a `Game`, validate the turn, apply the move.
2. Write the new state back.
3. **Directly** send the new `state` frame to any local sockets for that game.
4. **Publish** the frame to the Valkey channel `game:{id}:events`.

Every connected player runs a dedicated pub/sub listener task subscribed to their
game's channel. A player on a *different* server receives the publish and forwards
it to their socket. To avoid double-delivery when both players share a server, each
published frame carries a `delivered_by: <server_id>` field; a listener skips
frames its own server already delivered directly.

This pub/sub channel is the cross-server bus. It is also the subtle part: **a
Valkey `PUBLISH` only reaches subscribers on the same Valkey node.** This constraint
drives the sharding design in §3.8.

> **A bug this surfaced.** Early on, the "game start" state was only *published*,
> not sent directly to the creator. When both players landed on the same server,
> the creator's own listener deduplicated the publish (`delivered_by` matched) and
> the creator was never notified the game had started:
>
> ```
> Both players on Server A. Black joins → A publishes the start state.
>
>   A: PUBLISH game:G:events  {state, delivered_by: "A"}
>        │
>        ├─▶ A's listener for BLACK  → delivered_by "A" == self → SKIP   (intended dedup)
>        └─▶ A's listener for WHITE  → delivered_by "A" == self → SKIP   ✗ creator never told
>
> Nobody sent WHITE the state directly, so WHITE waits forever.
> ```
>
> Behind a load balancer without sticky sessions, this struck whenever both players
> hashed to the same droplet — roughly half the time. The fix: deliver the start
> state **directly to both local players** *and* publish for a remote one — the same
> pattern moves already used (the direct send covers same-server players; the
> `delivered_by` dedup then correctly suppresses only the redundant published copy).
> The load-test harness caught this on its very first run.

### 3.5 Concurrency: from distributed lock to optimistic CAS

The first implementation serialized writes to a game with a **distributed lock**: a
`SET game:{id}:lock <token> NX PX 3000` before the read-modify-write, and a Lua
script to release it atomically only if the token matched. Correct, but it cost
several Valkey round-trips per move:

```
SET NX (acquire)  →  HGET (load)  →  HSET (save)  →  EXPIRE  →  EVAL (release)  →  PUBLISH
```

Six operations, all serialized on Valkey's single command thread (Redis/Valkey
execute commands on one core). At scale, that thread became the bottleneck.

We replaced the lock with **optimistic concurrency**:

- Each state carries a `version`. Reads fetch state + version in one `HMGET`.
- The write is a single Lua script: *if the stored version still equals the
  expected version, write the new state and bump the version; otherwise report a
  conflict.*
- On conflict, the handler re-reads and retries (bounded retries).

```lua
local v = redis.call('HGET', KEYS[1], 'version')
if v == false      then return -1 end   -- gone
if v ~= ARGV[1]    then return 0  end   -- conflict
redis.call('HSET',    KEYS[1], 'state', ARGV[2])
redis.call('HINCRBY', KEYS[1], 'version', 1)
redis.call('PEXPIRE', KEYS[1], ARGV[3])
return 1
```

A move now costs **`HMGET` + `EVAL` + `PUBLISH`** — three operations instead of
six, and no Lua interpreter call for a separate lock release. Because chess is
turn-based, genuine conflicts on a single game are near-zero; the retry exists only
for safety. A `persisted` guard also rejects moves on an already-finished game,
which keeps a racing forfeit from being overwritten.

**Result:** the per-move datastore cost dropped from six operations to three. In
load tests this surfaced as a roughly 3× higher game ceiling before the next wall
(about 500 → 1500 concurrent games). A caveat I'll keep repeating: these figures
measure the *reduction in per-move work*, not the server's absolute capacity. As
§5.3 makes clear, the load generator saturated before the backend did, so read every
"Result" number in this article as a **relative delta** — "the same client got
further before something gave out" — not a measured server ceiling. Fewer ops per
move means more headroom; how much headroom in absolute terms, this campaign never
got to measure, because it could never push hard enough.

### 3.6 Durable persistence via a move stream

Writing every move straight to MongoDB would amplify writes badly (one DB write per
move per game). Instead, moves are batched through a Valkey Stream:

- On a non-final move of a signed-in game: `XADD moves_stream` with the game id,
  Mongo id, and move JSON.
- A background worker on each server runs `XREADGROUP` (consumer group `workers`)
  pulling up to 500 entries every 500ms, groups them by game, and issues a single
  MongoDB `BulkWrite` that `$push`es the moves.
- After a successful flush: `XACK` **and** `XDEL` the entries.

That `XDEL` matters. `XACK` alone removes entries from the pending list but **not
from the stream** — without `XDEL` the stream grows forever. A `MAXLEN ~` cap on
`XADD` guards against orphaned entries. The final move of a game (checkmate/
stalemate) is written synchronously rather than streamed, so the game can be
finalized immediately.

The stream is a *queue*, not storage. In steady state its length hovers near zero
because the worker drains it continuously. An empty stream is the healthy state;
the durable copy is in MongoDB.

### 3.7 Fault tolerance: heartbeats and orphan recovery

Each server writes a heartbeat (`SETEX server:{id}:heartbeat 15`) every five
seconds and registers itself in a `known_servers` set. Every ten seconds, each
server scans its peers. If a peer's heartbeat key has expired, it is presumed dead,
and the surviving server:

1. `XAUTOCLAIM`s the dead consumer's pending stream entries and flushes them to
   MongoDB (so moves in flight at the time of death are not lost).
2. Reads the dead server's tracked games (`server:{id}:games`) and, for each,
   finalizes it (if it had ended) or marks it disconnected and notifies the
   surviving player.

The `final_status` field is the crash-safety hinge: it is written to Valkey *before*
the synchronous MongoDB finalize, so if the server dies in between, a peer sees
`final_status` set and completes the finalize on its behalf.

### 3.8 Sharding across multiple Valkey nodes

One Valkey node is single-threaded for command execution, so at some point its core
saturates regardless of RAM. The fix is to shard game state across N independent
nodes.

The routing rule is a **deterministic hash of the game id** (FNV-1a, *not* the
standard library's randomized hasher — every server must agree on placement):

```
shard(game_id) = fnv1a(game_id) % N
```

All of a game's keys — state, lock, **and pub/sub channel** — go to the same node.
Because the hash is identical on every server, White's server and Black's server
independently route the same game to the same node, so the pub/sub channel they
share actually lives on one node and cross-server delivery still works. Global
coordination data (the move stream, `known_servers`, heartbeats) lives on a
designated coordination node (shard 0).

Adding a sixth node is a one-line config change (`REDIS_URLS` is a comma-separated
list; the code uses `% len`). The caveat is modulo hashing's classic weakness:
changing N reshuffles almost every game's placement. This is worse than it first
sounds, in two distinct ways:

- **It invalidates existing active games.** A game written under N=5 physically
  lives on `fnv1a(id) % 5`, but every server now computes `fnv1a(id) % 6` and looks
  on a *different* node — where the state isn't. The old data isn't moved; the
  lookups simply point at the wrong shard, so in-flight games break until they
  expire and clients start fresh.
- **A mixed fleet is split-brain.** During a rolling deploy, if some servers have
  N=5 and others N=6, the two players of one game can route to different nodes for
  the same game's state *and* pub/sub channel — inconsistent reads and broken
  delivery.

So changing the node count is a coordinated, all-servers-at-once operation, ideally
in a low-traffic window. Consistent hashing would shrink the reshuffle to ~`1/N` of
games and remove the all-at-once requirement; it is the natural next step.

**Result:** sharding across five nodes again roughly doubled observed throughput
(same relative-delta caveat as §3.5) and, more importantly, moved the bottleneck
*off* Valkey entirely — after this change, no single Valkey node was the thing that
gave out first.

### 3.9 Reconnect, forfeit, and inactivity

- **Disconnect no longer abandons.** A dropped player's game is kept alive in
  Valkey for a 60-minute rejoin window. They reconnect via an active-games list or
  by re-entering the game id (the join handler detects a returning, signed-in
  player and routes to the reconnect path).
- **Forfeit** is an explicit message; the opponent wins, recorded as a normal win.
- **Inactivity sweeper**: a background task finalizes any `InProgress` game idle for
  60 minutes as `Abandoned` and clears its Valkey state. `updated_at` is bumped on
  every move batch and on disconnect, so the window tracks real activity.

---

## 4. Deployment (DigitalOcean)

The whole system runs on DigitalOcean, deployed from a single container image.

- **Image**: a 3-stage Docker build — Node builds the React frontend (embedded into
  the Rust binary via `include_str!`), a Rust stage compiles the release binary, and
  a slim Debian runtime stage ships just the binary plus CA certificates. The build
  compiles only the server binary, skipping the load-test crate that shares the
  workspace.
- **Registry**: DigitalOcean Container Registry.
- **App servers**: a Droplet **Autoscale Pool** (min/max instances, CPU-target
  scaling) behind a **Regional Load Balancer** terminating TLS via a Let's Encrypt
  certificate. The LB targets the pool **by tag**, so autoscaled droplets join and
  leave automatically.
- **Golden image**: droplets boot from a snapshot containing Docker, the chess
  container (managed by a small systemd unit that injects a per-droplet `SERVER_ID`
  from the metadata service), and Watchtower for auto-updates. The monitoring agent
  is baked in so the autoscaler can read CPU.
- **CI/CD**: a push to `master` triggers GitHub Actions, which builds and pushes
  `:latest`; Watchtower on each droplet pulls it within minutes. Config changes
  (like the Valkey node list) require a re-snapshot and droplet recycle, since they
  live in the boot script, not the image.
- **Managed data**: MongoDB and five Valkey nodes, all in the same datacenter as the
  droplets so they communicate over the private VPC network.

A recurring operational lesson: **everything must live in the same region.** Private
networking only works within one datacenter, and an early mistake of placing the
load balancer and droplets in different regions from the databases produced
mysterious connection failures.

---

## 5. Load Testing

### 5.1 The harness

A purpose-built Rust load generator (`tokio-tungstenite`) drives full games over
the real WebSocket protocol. Its defining feature is **independent correctness
verification**: each simulated game runs an in-process mirror of the real chess
engine. After every move it asserts that *both* clients' received boards equal the
board the mirror computed — this checks cross-client equality (the distributed
consistency guarantee) and full correctness (the move was applied right) in a single
comparison.

It escalates load in stages (100, 200, …, then larger), reports per-stage
throughput and latency percentiles (via HDR histograms), classifies failures
(connect / timeout / server-error / **consistency** / desync), and stops when a
stage's failure rate crosses a threshold — with any consistency failure treated as
fatal.

### 5.2 The bottleneck tour

The headline finding is that across the entire campaign, **the consistency check
never failed** — zero divergence across millions of moves, including games whose two
players were served by different droplets. The architecture was correct under
everything thrown at it.

What *did* fail was instructive, because every ceiling turned out to be on the
client or edge, not the sharded backend:

| Wall hit | Symptom | Root cause | Fix |
|---|---|---|---|
| Laptop generator | p50 ~300ms latency | Transatlantic RTT; games play moves sequentially, so per-game rate ≈ 1/RTT | Generate load from inside the datacenter |
| Load balancer | bursts of connection resets | LB caps new TLS handshakes at ~500/sec; the test stormed thousands at once | A token-bucket `--connect-rate` flag pacing handshakes under the cap |
| Single Valkey node | flat throughput regardless of app servers | one Valkey command thread saturated | Halve per-move ops (CAS) + shard across nodes |
| Generator droplet | "too many open files", flat throughput | one client process holding all 2N sockets on a 1-vCPU box | Raise fd limits; bigger / multiple generators |

The one number I'll state without hedging, because it's a clean pass/fail rather than
a throughput figure: a single droplet served **4000 concurrent games** with zero move
failures, zero timeouts, and zero consistency violations. Everything else — the "3×
game ceiling," the "doubled throughput" — is the *same client getting further* after
a server-side change reduced per-move work, measured while the generator itself was
the limiting factor. They're directionally real (fewer ops per move genuinely buys
headroom) but they are deltas on a client-bound benchmark, not the server's ceiling.
The honest version of the whole results section is one sentence: **I made the server
do less work per move and it let the same overloaded client push a bit further, and
correctness never broke** — and then I ran out of client.

### 5.3 The real lesson: you can't stress a scaled-out server from one box

The deepest finding is structural. A single load-generator process holds *all*
`2N` WebSocket connections itself — for 8000 games, 16,000 persistent sockets on one
machine and (effectively) one core, each needing TLS, JSON parsing, an engine
mirror, and constant polling. In production, those 16,000 sockets come from 16,000
different devices, each holding one connection; the load is naturally distributed
across thousands of machines.

Concentrating all of it into one process means the **generator** saturates — CPU,
file descriptors, ephemeral ports (a single source IP tops out near ~28k connections
to one destination) — long before a horizontally-scaled, sharded server fleet
notices. It is *easy* to overwhelm one generator and *hard* to overwhelm N sharded
nodes. Genuinely finding the server's ceiling requires **distributed load
generation**: many generator machines, each holding a slice of the connections,
with the server-side CPU graphs (per Valkey node, per droplet) as the real measure
of saturation — not when a client fails, but when those graphs peg.

For the purpose of validating the architecture and sizing the fleet, this is enough:
the system is correct, consistent, and scales per-droplet, so fleet capacity is
roughly per-droplet capacity times droplet count.

---

## 6. Lessons Learned

1. **Separate the real-time plane from the durability plane.** Move delivery
   (pub/sub + live state) and move persistence (stream → batch → Mongo) are
   independent. Conflating them makes both harder to reason about.
2. **Prefer optimistic concurrency to locks for low-contention writes.** Turn-based
   games almost never conflict on a single entity; a versioned compare-and-set
   removed half the per-move datastore operations and a whole class of lock-lifetime
   bugs.
3. **Queues are not storage.** A stream needs explicit trimming; an `XACK` is not an
   `XDEL`. An empty queue is the healthy steady state.
4. **Deterministic placement is the whole game in sharding.** Every node must agree
   on where a key lives, which rules out randomized hashers and makes changing the
   node count a coordinated operation.
5. **Pub/sub does not cross datastore nodes.** Sharding a channel means every party
   to that channel must route to the same node — which falls out naturally if you
   shard by the same key everywhere.
6. **A good load test is a correctness oracle, not just a throughput meter.** This is
   the idea I'd take to any system, not just this one. The harness ran a second,
   independent copy of the engine as a *mirror*: after every move it re-derived the
   expected state from scratch and asserted that **both** clients received exactly
   that. A throughput meter tells you the system is fast; an oracle tells you it's
   *correct while* being fast — and under concurrency, that's the property that
   actually breaks. It caught a real distributed-delivery bug on its first run, and
   it's why the one claim in this article I'll make without a caveat is that
   correctness never failed. If you build load tests that only measure latency and
   throughput, you are not testing the hard part.

---

## 7. Further Work

Four threads I'd pull next, each already pointed to above:

- **Consistent hashing** for the Valkey shards, to make adding/removing a node move
  only ~`1/N` of games instead of reshuffling nearly all of them (§3.8).
- **Distributed load generation** — a fleet of generator machines coordinated to
  hold a slice of connections each, so the bottleneck is finally the server and not
  the client (§5.3).
- **Sharding the move stream** per node, so the coordination node isn't a write
  funnel for the persistence plane at very high signed-in volume (§3.6).
- **A flat board representation** (`[Option<Piece>; 64]` or bitboards) if move
  generation ever lands on the hot path (§3.1).

---

## 8. Conclusion

What began as a chess engine became an exercise in the fundamentals of distributed
systems: externalizing state so compute can be stateless and disposable, choosing a
concurrency model, separating real-time from durable paths, sharding for horizontal
scale, recovering from node death, and — not least — the humbling discovery that
measuring such a system is as hard as building it.

The chess was never the point. The point was that the same handful of ideas —
deterministic sharding, optimistic concurrency, a pub/sub bus, batched persistence,
heartbeat-based recovery — compose into a system that scales by adding nodes and
survives losing them. The board just made the correctness easy to verify: a chess
position is either right or it is not, and across the whole campaign, it was always
right.
