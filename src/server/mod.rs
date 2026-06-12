pub mod auth;
pub mod ws;

use crate::db::mongo::Db;
use crate::pieces::pieces::{Color, PieceType};
use crate::redis_state::pool::RedisPool;
use auth::GoogleVerifier;
use axum::extract::ws::Message;
use mongodb::bson::oid::ObjectId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::repository::game_repository::GameRepository;
use crate::repository::session_repository::SessionRepository;
use crate::repository::user_repository;
pub use user_repository::UserRepository;

pub type Tx = UnboundedSender<Message>;

/// Authenticated identity attached to a WebSocket connection.
#[derive(Clone)]
pub struct SessionUser {
    pub id: ObjectId,
    pub name: String,
    pub picture: Option<String>,
}

/// Only what cannot leave this process: the WebSocket senders and their
/// associated pub/sub cancellation tokens.
pub struct LocalGameSession {
    pub white_tx: Option<Tx>,
    pub black_tx: Option<Tx>,
    pub white_cancel: Option<CancellationToken>,
    pub black_cancel: Option<CancellationToken>,
}

impl LocalGameSession {
    pub fn new_white(tx: Tx, cancel: CancellationToken) -> Self {
        Self {
            white_tx: Some(tx),
            black_tx: None,
            white_cancel: Some(cancel),
            black_cancel: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub games: Arc<Mutex<HashMap<String, LocalGameSession>>>,
    pub redis: RedisPool,
    pub redis_url: String,
    pub server_id: String,
    pub db: Arc<Db>,
    pub user_repository: UserRepository,
    pub session_repository: SessionRepository,
    pub game_repository: GameRepository,
    pub google: Option<Arc<GoogleVerifier>>,
}

impl AppState {
    pub fn new(
        db: Db,
        google: Option<GoogleVerifier>,
        redis: RedisPool,
        redis_url: String,
        server_id: String,
        game_repository: GameRepository,
    ) -> Self {
        Self {
            games: Arc::new(Mutex::new(HashMap::new())),
            redis,
            redis_url,
            server_id,
            user_repository: UserRepository::new(db.users.clone()),
            session_repository: SessionRepository::new(db.sessions.clone()),
            game_repository,
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

/// Pieces a pawn may promote to; anything else is rejected.
pub fn promotion_from_str(s: &str) -> Option<PieceType> {
    match s {
        "queen" => Some(PieceType::Queen),
        "rook" => Some(PieceType::Rook),
        "bishop" => Some(PieceType::Bishop),
        "knight" => Some(PieceType::Knight),
        _ => None,
    }
}

pub fn color_str(c: Color) -> &'static str {
    match c {
        Color::White => "white",
        Color::Black => "black",
    }
}
