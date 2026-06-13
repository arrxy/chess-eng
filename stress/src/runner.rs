use std::time::Instant;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use serde_json::{Value, json};

use chess::board::game::{Game, GameStatus};
use chess::pieces::pieces::{Color, PieceType, Position};

use crate::client::{self, Ws};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailReason {
    Connect,
    Timeout,
    ServerError,
    Consistency,
    Desync,
}

pub struct GameResult {
    pub ok: bool,
    pub reason: Option<FailReason>,
    pub detail: Option<String>,
    pub move_latencies_us: Vec<u64>,
    pub plies: u32,
}

impl GameResult {
    fn fail(reason: FailReason, detail: String, lats: Vec<u64>, plies: u32) -> Self {
        Self {
            ok: false,
            reason: Some(reason),
            detail: Some(detail),
            move_latencies_us: lats,
            plies,
        }
    }
}

pub struct GameConfig {
    pub url: String,
    pub cookie: Option<String>,
    pub max_plies: u32,
    pub move_timeout_ms: u64,
    pub seed: u64,
    /// Optional handshake-rate limiter shared across all games in a stage.
    pub gate: Option<std::sync::Arc<crate::ratelimit::RateGate>>,
}

fn ptype(t: PieceType) -> &'static str {
    match t {
        PieceType::King => "king",
        PieceType::Queen => "queen",
        PieceType::Rook => "rook",
        PieceType::Bishop => "bishop",
        PieceType::Knight => "knight",
        PieceType::Pawn => "pawn",
        PieceType::Empty => "empty",
    }
}

fn pcolor(c: Color) -> &'static str {
    match c {
        Color::White => "white",
        Color::Black => "black",
    }
}

/// Serialize the mirror board into the exact JSON shape the server emits.
fn board_to_json(game: &Game) -> Value {
    let b = game.board();
    let rows: Vec<Vec<Value>> = (0..8usize)
        .map(|r| {
            (0..8usize)
                .map(|c| match &b.board[r][c] {
                    None => Value::Null,
                    Some(p) => json!({ "type": ptype(p.piece_type()), "color": pcolor(p.color()) }),
                })
                .collect()
        })
        .collect();
    json!(rows)
}

/// All legal (from, to) moves for the side to move.
fn legal_moves(game: &Game) -> Vec<(Position, Position)> {
    let mut v = Vec::new();
    for x in 0..8u8 {
        for y in 0..8u8 {
            let from = Position { x, y };
            for to in game.possible_moves_from(from) {
                v.push((from, to));
            }
        }
    }
    v
}

/// Is this move a pawn reaching the last rank (i.e. a promotion)?
fn is_promotion(game: &Game, from: Position, to: Position) -> bool {
    match game.board().get_piece(from) {
        Some(p) if matches!(p.piece_type(), PieceType::Pawn) => {
            (p.color() == Color::White && to.x == 0) || (p.color() == Color::Black && to.x == 7)
        }
        _ => false,
    }
}

/// Play one full game over two fresh connections, verifying every `state`.
/// `permit_drop` is invoked once both sockets are connected, so the caller can
/// release a connection-throttle permit before the (longer) move loop.
pub async fn run_game(cfg: &GameConfig, permit_drop: impl FnOnce()) -> GameResult {
    let mut lats: Vec<u64> = Vec::new();

    // --- handshake -------------------------------------------------------
    if let Some(g) = &cfg.gate {
        g.acquire().await;
    }
    let mut white = match client::connect(&cfg.url, cfg.cookie.as_deref()).await {
        Ok(w) => w,
        Err(e) => {
            permit_drop();
            return GameResult::fail(FailReason::Connect, format!("white connect: {e}"), lats, 0);
        }
    };
    if let Err(e) = client::send(&mut white, &json!({"type":"create"})).await {
        permit_drop();
        return GameResult::fail(FailReason::Connect, format!("create: {e}"), lats, 0);
    }
    let joined = match client::recv_until(&mut white, "joined", cfg.move_timeout_ms).await {
        Ok(v) => v,
        Err(e) => {
            permit_drop();
            return GameResult::fail(FailReason::Timeout, format!("white joined: {e}"), lats, 0);
        }
    };
    let gid = joined["game_id"].as_str().unwrap_or("").to_string();

    if let Some(g) = &cfg.gate {
        g.acquire().await;
    }
    let mut black = match client::connect(&cfg.url, cfg.cookie.as_deref()).await {
        Ok(b) => b,
        Err(e) => {
            permit_drop();
            return GameResult::fail(FailReason::Connect, format!("black connect: {e}"), lats, 0);
        }
    };
    permit_drop(); // both connected; let another game start connecting

    if let Err(e) = client::send(&mut black, &json!({"type":"join","game_id":gid})).await {
        return GameResult::fail(FailReason::Connect, format!("join: {e}"), lats, 0);
    }
    // black: joined then state; white also receives the broadcast state.
    if let Err(e) = client::recv_until(&mut black, "joined", cfg.move_timeout_ms).await {
        return GameResult::fail(FailReason::Timeout, format!("black joined: {e}"), lats, 0);
    }
    if let Err(e) = client::recv_until(&mut black, "state", cfg.move_timeout_ms).await {
        return GameResult::fail(
            FailReason::Timeout,
            format!("black initial state: {e}"),
            lats,
            0,
        );
    }
    if let Err(e) = client::recv_until(&mut white, "state", cfg.move_timeout_ms).await {
        return GameResult::fail(
            FailReason::Timeout,
            format!("white initial state: {e}"),
            lats,
            0,
        );
    }

    // --- move loop -------------------------------------------------------
    let mut game = Game::new();
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let mut plies = 0u32;

    while plies < cfg.max_plies {
        if matches!(game.status(), GameStatus::Checkmate | GameStatus::Stalemate) {
            break;
        }
        let moves = legal_moves(&game);
        if moves.is_empty() {
            break;
        }
        let (from, to) = *moves.choose(&mut rng).unwrap();
        let promo = is_promotion(&game, from, to);

        let mut msg = json!({
            "type": "move",
            "from": { "x": from.x, "y": from.y },
            "to":   { "x": to.x,   "y": to.y },
        });
        if promo {
            msg["promotion"] = json!("queen");
        }

        // The mover's socket depends on whose turn it is.
        let mover_is_white = game.current_turn() == Color::White;
        let (mover, other): (&mut Ws, &mut Ws) = if mover_is_white {
            (&mut white, &mut black)
        } else {
            (&mut black, &mut white)
        };

        let t0 = Instant::now();
        if let Err(e) = client::send(mover, &msg).await {
            return GameResult::fail(
                FailReason::ServerError,
                format!("send move: {e}"),
                lats,
                plies,
            );
        }

        // Apply to the mirror and compute the expected post-move state.
        let promo_pt = if promo { Some(PieceType::Queen) } else { None };
        if !game.make_move(from, to, promo_pt) {
            return GameResult::fail(
                FailReason::Desync,
                format!("mirror rejected a move it generated: {from:?}->{to:?}"),
                lats,
                plies,
            );
        }
        let expected_board = board_to_json(&game);
        let expected_turn = pcolor(game.current_turn());

        let mover_state = match client::recv_until(mover, "state", cfg.move_timeout_ms).await {
            Ok(v) => v,
            Err(e) => {
                return GameResult::fail(
                    FailReason::Timeout,
                    format!("mover state: {e}"),
                    lats,
                    plies,
                );
            }
        };
        let other_state = match client::recv_until(other, "state", cfg.move_timeout_ms).await {
            Ok(v) => v,
            Err(e) => {
                return GameResult::fail(
                    FailReason::Timeout,
                    format!("other state: {e}"),
                    lats,
                    plies,
                );
            }
        };
        lats.push(t0.elapsed().as_micros() as u64);

        // Consistency: both received boards must equal the independently
        // computed board, and agree on turn + captured.
        if mover_state["board"] != expected_board {
            return GameResult::fail(
                FailReason::Consistency,
                format!("ply {plies}: mover board != expected"),
                lats,
                plies,
            );
        }
        if other_state["board"] != expected_board {
            return GameResult::fail(
                FailReason::Consistency,
                format!("ply {plies}: opponent board != expected"),
                lats,
                plies,
            );
        }
        if mover_state["turn"].as_str() != Some(expected_turn) {
            return GameResult::fail(
                FailReason::Consistency,
                format!("ply {plies}: turn != {expected_turn}"),
                lats,
                plies,
            );
        }
        if mover_state["captured"] != other_state["captured"] {
            return GameResult::fail(
                FailReason::Consistency,
                format!("ply {plies}: captured lists differ between clients"),
                lats,
                plies,
            );
        }

        plies += 1;
    }

    client::close(&mut white).await;
    client::close(&mut black).await;
    GameResult {
        ok: true,
        reason: None,
        detail: None,
        move_latencies_us: lats,
        plies,
    }
}
