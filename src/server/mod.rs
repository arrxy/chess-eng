pub mod auth;
pub mod ws;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use axum::extract::ws::Message;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::DateTime;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;
use crate::board::game::{Game, GameStatus};
use crate::db::game_schema::{self, Move as MoveRecord};
use crate::db::mongo::Db;
use crate::pieces::pieces::{Color, PieceType};
use auth::GoogleVerifier;

pub type Tx = UnboundedSender<Message>;

/// Authenticated identity attached to a WebSocket connection.
#[derive(Clone)]
pub struct SessionUser {
    pub id: ObjectId,
    pub name: String,
    pub picture: Option<String>,
}

pub struct GameSession {
    pub game: Game,
    pub white_tx: Option<Tx>,
    pub black_tx: Option<Tx>,
    pub white_user: Option<SessionUser>,
    pub black_user: Option<SessionUser>,
    /// Both players have joined at least once.
    pub started: bool,
    /// The game has been written to MongoDB (finished or abandoned).
    pub persisted: bool,
    pub moves: Vec<MoveRecord>,
    pub captured_by_white: Vec<PieceType>,
    pub captured_by_black: Vec<PieceType>,
    pub created_at: DateTime,
}

impl GameSession {
    pub fn new(game: Game, white_tx: Tx, white_user: Option<SessionUser>) -> Self {
        Self {
            game,
            white_tx: Some(white_tx),
            black_tx: None,
            white_user,
            black_user: None,
            started: false,
            persisted: false,
            moves: Vec::new(),
            captured_by_white: Vec::new(),
            captured_by_black: Vec::new(),
            created_at: DateTime::now(),
        }
    }

    pub fn has_signed_in_player(&self) -> bool {
        self.white_user.is_some() || self.black_user.is_some()
    }

    /// Snapshot for MongoDB. Only called for games worth saving.
    pub fn to_game_doc(&self, status: game_schema::GameStatus) -> game_schema::Game {
        game_schema::Game {
            id: None,
            white_user_id: self.white_user.as_ref().map(|u| u.id),
            black_user_id: self.black_user.as_ref().map(|u| u.id),
            white_name: self.white_user.as_ref().map(|u| u.name.clone()),
            black_name: self.black_user.as_ref().map(|u| u.name.clone()),
            moves: self.moves.clone(),
            status,
            created_at: self.created_at,
            updated_at: DateTime::now(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub games: Arc<Mutex<HashMap<String, GameSession>>>,
    pub db: Arc<Db>,
    pub google: Option<Arc<GoogleVerifier>>,
}

impl AppState {
    pub fn new(db: Db, google: Option<GoogleVerifier>) -> Self {
        Self {
            games: Arc::new(Mutex::new(HashMap::new())),
            db: Arc::new(db),
            google: google.map(Arc::new),
        }
    }
}

pub fn new_game_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

pub fn piece_type_str(t: PieceType) -> &'static str {
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

pub fn board_json(game: &Game) -> serde_json::Value {
    let board = game.board();
    let squares: Vec<Vec<serde_json::Value>> = (0..8usize)
        .map(|row| {
            (0..8usize)
                .map(|col| match &board.board[row][col] {
                    None => serde_json::Value::Null,
                    Some(p) => serde_json::json!({
                        "type": piece_type_str(p.piece_type()),
                        "color": color_str(p.color()),
                    }),
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

fn captured_json(pieces: &[PieceType]) -> serde_json::Value {
    serde_json::json!(pieces.iter().map(|&p| piece_type_str(p)).collect::<Vec<_>>())
}

pub fn players_json(session: &GameSession) -> serde_json::Value {
    serde_json::json!({
        "white": session.white_user.as_ref().map(|u| &u.name),
        "black": session.black_user.as_ref().map(|u| &u.name),
    })
}

/// Full `state` frame sent to both players after a join or a move.
pub fn state_json(session: &GameSession, last_move: Option<serde_json::Value>) -> String {
    let status = match session.game.status() {
        GameStatus::Ongoing => "ongoing",
        GameStatus::Check => "check",
        GameStatus::Checkmate => "checkmate",
        GameStatus::Stalemate => "stalemate",
    };
    let mut msg = serde_json::json!({
        "type": "state",
        "board": board_json(&session.game),
        "turn": color_str(session.game.current_turn()),
        "status": status,
        "players": players_json(session),
        "captured": {
            "white": captured_json(&session.captured_by_white),
            "black": captured_json(&session.captured_by_black),
        },
    });
    if let Some(lm) = last_move {
        msg["lastMove"] = lm;
    }
    msg.to_string()
}
