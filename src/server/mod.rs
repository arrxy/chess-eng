pub mod ws;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use axum::extract::ws::Message;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;
use crate::board::game::Game;
use crate::pieces::pieces::{Color, PieceType};

pub type Tx = UnboundedSender<Message>;

pub struct GameSession {
    pub game: Game,
    pub white_tx: Option<Tx>,
    pub black_tx: Option<Tx>,
}

#[derive(Clone)]
pub struct AppState {
    pub games: Arc<Mutex<HashMap<String, GameSession>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            games: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub fn new_game_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

pub fn board_json(game: &Game) -> serde_json::Value {
    let board = game.board();
    let squares: Vec<Vec<serde_json::Value>> = (0..8usize)
        .map(|row| {
            (0..8usize)
                .map(|col| match &board.board[row][col] {
                    None => serde_json::Value::Null,
                    Some(p) => {
                        let t = match p.piece_type() {
                            PieceType::King => "king",
                            PieceType::Queen => "queen",
                            PieceType::Rook => "rook",
                            PieceType::Bishop => "bishop",
                            PieceType::Knight => "knight",
                            PieceType::Pawn => "pawn",
                            PieceType::Empty => "empty",
                        };
                        let c = match p.color() {
                            Color::White => "white",
                            Color::Black => "black",
                        };
                        serde_json::json!({ "type": t, "color": c })
                    }
                })
                .collect()
        })
        .collect();
    serde_json::json!(squares)
}

pub fn color_str(c: Color) -> &'static str {
    match c {
        Color::White => "white",
        Color::Black => "black",
    }
}
