# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cd frontend && npm install && npm run build   # build frontend into static/dist (required before cargo build)
cargo build          # compile (embeds static/dist via include_str!)
cargo run            # start server at http://localhost:3000
cargo check          # fast compile check
cargo clippy         # lint
cargo fmt            # format
cd frontend && npm run dev                    # frontend dev server with HMR, proxies API/WS to :3000
```

No tests are currently defined. Add them with `#[cfg(test)]` modules and run with `cargo test`.

**Environment:** copy `.env.example` to `.env`. `MONGODB_URI`/`MONGODB_DB` are required at startup; `GOOGLE_CLIENT_ID` is optional (Google login is hidden when unset).

## Architecture

This is a Rust chess engine (Rust edition 2024) with an Axum server, MongoDB persistence, and Google sign-in.

**Module layout:**

- `src/board/` — game state
  - `board.rs` — `Board` struct: 8×8 `Vec<Vec<Option<Box<dyn Piece>>>>`, piece placement/movement helpers
  - `game.rs` — `Game` struct: owns `Board` + two `Player`s + `turn: Color`; `make_move` is the main entry point
  - `player.rs` — `Player` struct: name + color, no game logic
- `src/pieces/` — piece logic
  - `pieces.rs` — shared types: `Position { x, y: u8 }`, `Color`, `PieceType`, and the `Piece` trait
  - one file per piece type (`pawn.rs`, `rook.rs`, `bishop.rs`, `knight.rs`, `queen.rs`, `king.rs`)
- `src/db/` — MongoDB layer
  - `mongo.rs` — `connect_db()`, `Db` struct (typed `users`/`games`/`sessions` collections), index builders
  - `user_schema.rs` / `game_schema.rs` / `session_schema.rs` — document structs + their indexes
- `src/server/` — Axum state, WebSocket handler (`ws.rs`), auth + history HTTP handlers (`auth.rs`)
- `frontend/` — Vite + React app: `src/App.jsx` (state + WebSocket logic), `src/chess.js` (glyphs, board helpers), `src/styles.css`, `src/components/` (Board, GameView, Lobby, MyGames, Replay, CapturedRow, GoogleButton, TweaksPanel). `npm run build` emits `static/dist/` with fixed filenames (`index.html`, `app.js`, `app.css`) which the Rust binary embeds via `include_str!` — so the frontend must be built before `cargo build`. `static/dist/` is gitignored.

**`Piece` trait** (in `pieces/pieces.rs`):
```rust
fn color(&self) -> Color;
fn piece_type(&self) -> PieceType;
fn can_move(&self, from: Position, to: Position) -> bool;   // geometry only, no board state
fn possible_moves(&self, from: Position, board: &Board) -> Vec<Position>;
```

**Coordinate system:** `Position { x, y }` where `x` is the row (0 = black back rank, 7 = white back rank) and `y` is the column. White moves in the `x--` direction; black moves `x++`.

**Move flow:** `Game::make_move` validates turn ownership, delegates to `piece.possible_moves(from, board)`, then calls `Board::move_piece`. `can_move` is a pure geometry check not used by `make_move` — `possible_moves` handles board-aware logic (blocking, captures).

**`GameStatus`** enum returned by `Game::status()`: `Ongoing`, `Check`, `Checkmate`, `Stalemate`. Computed by simulating pseudo-legal moves on a cloned board and testing `board.is_in_check(color)`.

## Server / frontend

`cargo run` starts an Axum HTTP+WebSocket server on `:3000`. `GET /`, `/app.js`, and `/app.css` serve the embedded Vite-built frontend (`static/dist/*` via `include_str!`). `GET /ws` is the WebSocket endpoint.

**In-memory state** (`src/server/`):
- `AppState` — games map + `Arc<Db>` + optional `GoogleVerifier`, shared across all connections
- `GameSession` — owns the `Game`, `Option<Tx>` senders, optional `SessionUser` per color, the move list, and captured-piece lists
- Games are cleaned up when both players disconnect

**Auth** (`src/server/auth.rs`): Google Identity Services button in the page posts the ID token to `POST /auth/google`; the server verifies it against Google's JWKS (`jsonwebtoken` + `reqwest`), upserts the user, and issues an opaque session token in an HTTP-only `session` cookie (Mongo `sessions` collection, 30-day TTL index). The WebSocket upgrade reads the same cookie so connections know who is playing. Anonymous play needs no login.

**HTTP routes:** `GET /auth/config` (Google client id or null), `POST /auth/google`, `GET /auth/me`, `POST /auth/logout`, `GET /api/games` (signed-in user's finished games with full move lists), `GET /stats`.

**Persistence:** a game is written to Mongo once, when it ends, and only if at least one player is signed in. Checkmate → `WhiteWon`/`BlackWon`, stalemate → `Draw`, mid-game disconnect → `Abandoned`. Anonymous-vs-anonymous games are never stored.

**WebSocket protocol** (JSON text frames):

| Direction | Message |
|-----------|---------|
| Client → | `{"type":"create"}` |
| Client → | `{"type":"join","game_id":"…"}` |
| Client → | `{"type":"move","from":{"x":…,"y":…},"to":{…}}` |
| Client → | `{"type":"moves","x":…,"y":…}` |
| Server → | `{"type":"joined","game_id":"…","color":"white"\|"black"}` |
| Server → | `{"type":"state","board":[[…]],"turn":…,"status":…,"players":{"white":name\|null,"black":…},"captured":{"white":[piece,…],"black":[…]},"lastMove":{…}?}` |
| Server → | `{"type":"possible_moves","moves":[{"x":…,"y":…},…]}` |
| Server → | `{"type":"error","message":"…"}` |
| Server → | `{"type":"opponent_disconnected"}` |

The board in `state` messages is an 8×8 JSON array (row 0 = black back rank, row 7 = white back rank). Each cell is `null` or `{"type":"pawn"|…,"color":"white"|"black"}`. `captured.white` lists the pieces white has taken; the frontend renders them on each player row with a +N material score and also uses them for the replay viewer reached via "my games".

**Not yet implemented:** en passant, castling, pawn promotion, reconnecting to an in-progress game.
