# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build          # compile
cargo run            # start server at http://localhost:3000
cargo check          # fast compile check
cargo clippy         # lint
cargo fmt            # format
```

No tests are currently defined. Add them with `#[cfg(test)]` modules and run with `cargo test`.

## Architecture

This is a Rust chess engine (Rust edition 2024, no external dependencies).

**Module layout:**

- `src/board/` — game state
  - `board.rs` — `Board` struct: 8×8 `Vec<Vec<Option<Box<dyn Piece>>>>`, piece placement/movement helpers
  - `game.rs` — `Game` struct: owns `Board` + two `Player`s + `turn: Color`; `make_move` is the main entry point
  - `player.rs` — `Player` struct: name + color, no game logic
- `src/pieces/` — piece logic
  - `pieces.rs` — shared types: `Position { x, y: u8 }`, `Color`, `PieceType`, and the `Piece` trait
  - one file per piece type (`pawn.rs`, `rook.rs`, `bishop.rs`, `knight.rs`, `queen.rs`, `king.rs`)

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

`cargo run` starts an Axum HTTP+WebSocket server on `:3000`. `GET /` serves the embedded single-page frontend (`static/index.html` via `include_str!`). `GET /ws` is the WebSocket endpoint.

**In-memory state** (`src/server/`):
- `AppState` — `Arc<Mutex<HashMap<String, GameSession>>>`, shared across all connections
- `GameSession` — owns the `Game` plus `Option<Tx>` senders for white and black
- Games are cleaned up when both players disconnect

**WebSocket protocol** (JSON text frames):

| Direction | Message |
|-----------|---------|
| Client → | `{"type":"create"}` |
| Client → | `{"type":"join","game_id":"…"}` |
| Client → | `{"type":"move","from":{"x":…,"y":…},"to":{…}}` |
| Client → | `{"type":"moves","x":…,"y":…}` |
| Server → | `{"type":"joined","game_id":"…","color":"white"\|"black"}` |
| Server → | `{"type":"state","board":[[…]],"turn":"white"\|"black","status":"ongoing"\|"check"\|"checkmate"\|"stalemate"}` |
| Server → | `{"type":"possible_moves","moves":[{"x":…,"y":…},…]}` |
| Server → | `{"type":"error","message":"…"}` |
| Server → | `{"type":"opponent_disconnected"}` |

The board in `state` messages is an 8×8 JSON array (row 0 = black back rank, row 7 = white back rank). Each cell is `null` or `{"type":"pawn"|…,"color":"white"|"black"}`.

**Not yet implemented:** en passant, castling, pawn promotion.
