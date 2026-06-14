# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project overview

Distributed real-time multiplayer chess server in **Rust (edition 2024)** with:

- Pure chess engine (`src/board/`, `src/pieces/`) — no I/O
- **Axum** HTTP + WebSocket server
- **Valkey/Redis** for live game state (sharded), pub/sub, move stream, optimistic CAS
- **MongoDB** for users, sessions, and finished game history
- **React + Vite** frontend embedded into the binary via `include_str!`

App servers are stateless; live state lives in Redis. Background tasks handle heartbeats, batched Mongo writes, and inactivity sweeps. Design details: [`findings/finding.md`](findings/finding.md).

## Setup

```bash
cp .env.example .env          # edit MONGODB_URI, MONGODB_DB, REDIS_URL or REDIS_URLS
cd frontend && npm install && npm run build   # required before cargo build
cargo run                     # http://localhost:3000
```

Frontend dev with HMR (proxies API/WS to :3000):

```bash
cd frontend && npm run dev
```

### Environment variables

| Variable | Required | Notes |
|----------|----------|-------|
| `MONGODB_URI` | yes | MongoDB connection string |
| `MONGODB_DB` | yes | Database name |
| `REDIS_URL` | one of | Single Redis/Valkey node |
| `REDIS_URLS` | one of | Comma-separated shards; **overrides** `REDIS_URL`. First node is coordination (stream, heartbeats). |
| `SERVER_ID` | no | Stable per-instance id (defaults to random UUID) |
| `GOOGLE_CLIENT_ID` | no | Google sign-in hidden when unset |
| `PORT` | no | Defaults to `3000` |

## Key commands

| Action | Command |
|--------|---------|
| Fast compile check | `cargo check` |
| Build | `cargo build` |
| Run server | `cargo run` |
| Engine tests | `cargo test` |
| Lint | `cargo clippy` |
| Format | `cargo fmt` |
| Load test | `cargo run --release -p stress -- --url ws://localhost:3000/ws` |

Engine tests include a 331-game replay suite in `src/board/famous_games.rs` using `tests/fixtures/famous_games.tsv`.

## Codebase layout

```
src/
  board/          Board, Game, player — chess rules and status
  pieces/         Piece trait + one file per piece type
  db/             Mongo schemas + connect (mongo.rs)
  repository/     User, session, game data access
  redis_state/    RedisGameState, sharding, CAS, pub/sub, stream, hydrate
  server/         AppState, WebSocket handler (ws.rs), auth
  service/        HTTP route handlers (game_service, frontend_service)
  routes/         Axum router
  background/     heartbeat, batch_flush, sweeper
  lib.rs          Public engine API (board + pieces only)
  main.rs         Server binary startup
frontend/         React UI → static/dist/ (gitignored build output)
stress/           Workspace crate: load + consistency harness
```

## Architecture notes

**Coordinate system:** `Position { x, y }` — `x` is row (0 = black back rank, 7 = white back rank), `y` is column. White moves `x--`; black moves `x++`.

**Move flow (engine):** `Game::make_move` → `piece.possible_moves(from, board)` → `Board::move_piece`. `can_move` is geometry-only; do not use it for legality.

**Live state:** `RedisGameState` in `src/redis_state/` is authoritative during a game. `AppState.games` holds only local WebSocket senders (`LocalGameSession`), not game logic.

**Persistence:** Moves batch through a Redis stream → MongoDB. Finished games (checkmate, stalemate, forfeit, abandon) are stored when at least one player is signed in.

**Special moves:** castling, en passant, promotion are implemented. Castling generation lives in `Board::castling_moves` (not `King::possible_moves`).

## HTTP routes

Static: `/`, `/app.js`, `/app.css`  
WebSocket: `/ws`  
Auth: `/auth/config`, `/auth/google`, `/auth/me`, `/auth/logout`  
API: `/api/games`, `/api/active-games`, `/stats`, `/version`

## WebSocket protocol

JSON text frames on `GET /ws`:

| Direction | Messages |
|-----------|----------|
| Client → | `create`, `join`, `reconnect`, `move`, `moves`, `forfeit` |
| Server → | `joined`, `state`, `possible_moves`, `game_over`, `opponent_disconnected`, `opponent_reconnected`, `error` |

Board in `state` is 8×8 JSON (row 0 = black back rank). Cells are `null` or `{"type":"pawn"|…,"color":"white"|"black"}`.

## Conventions

- **Always build frontend before `cargo build`** — binary embeds `static/dist/` via `include_str!`.
- Keep engine code (`board/`, `pieces/`) free of server/DB/Redis dependencies.
- Match existing naming, module layout, and error-handling style in surrounding code.
- Minimize diff scope; avoid unrelated refactors.
- Do not add tests unless requested or they cover meaningful behavior.
- Do not create git commits unless explicitly asked.

## Do not

- Commit `.env` or secrets.
- Edit `static/dist/` by hand (build output).
- Assume in-memory game state — live games live in Redis.
- Break the engine/library split: `lib.rs` exposes only `board` + `pieces` for the `stress` crate.

## Further reading

- [`README.md`](README.md) — features, local setup, deployment overview
- [`findings/finding.md`](findings/finding.md) — distributed design + load testing
- [`documents/deployment.md`](documents/deployment.md) — production runbook
- [`stress/README.md`](stress/README.md) — load tester usage
