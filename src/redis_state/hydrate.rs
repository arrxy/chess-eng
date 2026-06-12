use crate::board::board::{Board, CastlingRights};
use crate::board::game::Game;
use crate::pieces::bishop::Bishop;
use crate::pieces::king::King;
use crate::pieces::knight::Knight;
use crate::pieces::pawn::Pawn;
use crate::pieces::pieces::{Color, Piece, PieceType, Position};
use crate::pieces::queen::Queen;
use crate::pieces::rook::Rook;
use crate::server::{SessionUser, color_str, piece_type_str};
use mongodb::bson::DateTime;

use super::{RedisGameState, SerializedPiece};

fn make_piece(piece_type: PieceType, color: Color) -> Box<dyn Piece> {
    match piece_type {
        PieceType::King => Box::new(King::new(color)),
        PieceType::Queen => Box::new(Queen::new(color)),
        PieceType::Rook => Box::new(Rook::new(color)),
        PieceType::Bishop => Box::new(Bishop::new(color)),
        PieceType::Knight => Box::new(Knight::new(color)),
        PieceType::Pawn => Box::new(Pawn::new(color)),
        PieceType::Empty => panic!("Empty is not a placeable piece"),
    }
}

pub fn redis_to_game(state: &RedisGameState) -> Game {
    let board_cells: Vec<Vec<Option<Box<dyn Piece>>>> = state
        .board
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| cell.as_ref().map(|sp| make_piece(sp.piece_type, sp.color)))
                .collect()
        })
        .collect();

    let castling = CastlingRights {
        white_kingside: state.castling_white_kingside,
        white_queenside: state.castling_white_queenside,
        black_kingside: state.castling_black_kingside,
        black_queenside: state.castling_black_queenside,
    };

    let en_passant_target = state
        .en_passant_target
        .map(|(x, y)| Position { x, y });

    let board = Board {
        board: board_cells,
        en_passant_target,
        castling,
    };

    Game::from_parts(board, state.turn)
}

pub fn game_to_redis(
    game: &Game,
    white_user: Option<&SessionUser>,
    black_user: Option<&SessionUser>,
    existing: &RedisGameState,
) -> RedisGameState {
    let board_ref = game.board();
    let board = (0..8usize)
        .map(|row| {
            (0..8usize)
                .map(|col| {
                    board_ref.board[row][col].as_ref().map(|p| SerializedPiece {
                        piece_type: p.piece_type(),
                        color: p.color(),
                    })
                })
                .collect()
        })
        .collect();

    RedisGameState {
        board,
        turn: game.current_turn(),
        castling_white_kingside: board_ref.castling.white_kingside,
        castling_white_queenside: board_ref.castling.white_queenside,
        castling_black_kingside: board_ref.castling.black_kingside,
        castling_black_queenside: board_ref.castling.black_queenside,
        en_passant_target: board_ref
            .en_passant_target
            .map(|p| (p.x, p.y)),
        white_user_id: white_user
            .map(|u| u.id.to_hex())
            .or_else(|| existing.white_user_id.clone()),
        white_user_name: white_user
            .map(|u| u.name.clone())
            .or_else(|| existing.white_user_name.clone()),
        black_user_id: black_user
            .map(|u| u.id.to_hex())
            .or_else(|| existing.black_user_id.clone()),
        black_user_name: black_user
            .map(|u| u.name.clone())
            .or_else(|| existing.black_user_name.clone()),
        moves: existing.moves.clone(),
        captured_by_white: existing.captured_by_white.clone(),
        captured_by_black: existing.captured_by_black.clone(),
        started: existing.started,
        persisted: existing.persisted,
        created_at_ms: existing.created_at_ms,
        updated_at_ms: DateTime::now().timestamp_millis(),
        mongo_game_id: existing.mongo_game_id.clone(),
        final_status: existing.final_status,
    }
}

/// Build a fresh RedisGameState from a brand-new game (create handler).
pub fn new_redis_state(game: &Game, white_user: Option<&SessionUser>) -> RedisGameState {
    let board_ref = game.board();
    let board = (0..8usize)
        .map(|row| {
            (0..8usize)
                .map(|col| {
                    board_ref.board[row][col].as_ref().map(|p| SerializedPiece {
                        piece_type: p.piece_type(),
                        color: p.color(),
                    })
                })
                .collect()
        })
        .collect();

    let now = DateTime::now().timestamp_millis();
    RedisGameState {
        board,
        turn: game.current_turn(),
        castling_white_kingside: true,
        castling_white_queenside: true,
        castling_black_kingside: true,
        castling_black_queenside: true,
        en_passant_target: None,
        moves: Vec::new(),
        captured_by_white: Vec::new(),
        captured_by_black: Vec::new(),
        white_user_id: white_user.map(|u| u.id.to_hex()),
        white_user_name: white_user.map(|u| u.name.clone()),
        black_user_id: None,
        black_user_name: None,
        started: false,
        persisted: false,
        created_at_ms: now,
        updated_at_ms: now,
        mongo_game_id: None,
        final_status: None,
    }
}

/// Build the board JSON array (same format as before).
pub fn board_json_from_redis(state: &RedisGameState) -> serde_json::Value {
    let squares: Vec<Vec<serde_json::Value>> = state
        .board
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    None => serde_json::Value::Null,
                    Some(sp) => serde_json::json!({
                        "type": piece_type_str(sp.piece_type),
                        "color": color_str(sp.color),
                    }),
                })
                .collect()
        })
        .collect();
    serde_json::json!(squares)
}
