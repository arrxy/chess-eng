//! Library surface for the chess engine, so other crates (e.g. the `stress`
//! load-test harness) can reuse move generation and game state. Only the pure
//! engine modules are exposed; the server/db/runtime code lives in `main.rs`.

pub mod board;
pub mod pieces;
