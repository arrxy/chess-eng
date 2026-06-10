use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use mongodb::bson::DateTime;
use serde_json::{json, Value};
use tokio::sync::mpsc::unbounded_channel;

use crate::board::game::{Game, GameStatus};
use crate::db::game_schema::{self, Move as MoveRecord};
use crate::pieces::pieces::{Color, PieceType, Position};
use super::{promotion_from_str, state_json, AppState, GameSession, SessionUser, new_game_id};

fn persist_game(state: &AppState, doc: game_schema::Game) {
    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = db.games.insert_one(doc).await {
            eprintln!("failed to persist game: {e}");
        }
    });
}

pub async fn handle_socket(socket: WebSocket, state: AppState, user: Option<SessionUser>) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut game_id: Option<String> = None;
    let mut my_color: Option<Color> = None;

    loop {
        match ws_rx.next().await {
            None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
            Some(Ok(Message::Text(text))) => {
                let v: Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match v["type"].as_str().unwrap_or("") {
                    "create" => {
                        let gid = new_game_id();
                        state.games.lock().unwrap().insert(
                            gid.clone(),
                            GameSession::new(Game::new(), tx.clone(), user.clone()),
                        );
                        game_id = Some(gid.clone());
                        my_color = Some(Color::White);
                        let _ = tx.send(Message::Text(
                            json!({"type":"joined","game_id":gid,"color":"white"}).to_string(),
                        ));
                    }

                    "join" => {
                        let gid = match v["game_id"].as_str() {
                            Some(id) => id.to_string(),
                            None => continue,
                        };
                        let mut games = state.games.lock().unwrap();
                        match games.get_mut(&gid) {
                            None => {
                                let _ = tx.send(Message::Text(
                                    json!({"type":"error","message":"game not found"}).to_string(),
                                ));
                            }
                            Some(s) if s.black_tx.is_some() => {
                                let _ = tx.send(Message::Text(
                                    json!({"type":"error","message":"game is full"}).to_string(),
                                ));
                            }
                            Some(session) => {
                                session.black_tx = Some(tx.clone());
                                session.black_user = user.clone();
                                session.started = true;
                                game_id = Some(gid.clone());
                                my_color = Some(Color::Black);

                                let state_msg = state_json(session, None);
                                let _ = tx.send(Message::Text(
                                    json!({"type":"joined","game_id":&gid,"color":"black"})
                                        .to_string(),
                                ));
                                let _ = tx.send(Message::Text(state_msg.clone()));
                                if let Some(wtx) = &session.white_tx {
                                    let _ = wtx.send(Message::Text(state_msg));
                                }
                            }
                        }
                    }

                    "move" => {
                        let (gid, color) = match (&game_id, my_color) {
                            (Some(g), Some(c)) => (g.clone(), c),
                            _ => continue,
                        };
                        let fx = v["from"]["x"].as_u64().unwrap_or(0) as u8;
                        let fy = v["from"]["y"].as_u64().unwrap_or(0) as u8;
                        let tx_ = v["to"]["x"].as_u64().unwrap_or(0) as u8;
                        let ty_ = v["to"]["y"].as_u64().unwrap_or(0) as u8;
                        let promotion = v["promotion"].as_str().and_then(promotion_from_str);

                        let mut finished_doc: Option<game_schema::Game> = None;
                        {
                            let mut games = state.games.lock().unwrap();
                            if let Some(session) = games.get_mut(&gid) {
                                if session.game.current_turn() != color {
                                    let _ = tx.send(Message::Text(
                                        json!({"type":"error","message":"not your turn"})
                                            .to_string(),
                                    ));
                                    continue;
                                }
                                let from = Position { x: fx, y: fy };
                                let to = Position { x: tx_, y: ty_ };

                                // Look before the move so we can record the piece and any capture
                                let piece = session.game.board().get_piece(from).map(|p| p.piece_type());
                                let mut captured = session.game.board().get_piece(to).map(|p| p.piece_type());

                                if session.game.make_move(from, to, promotion) {
                                    let moved_pawn = matches!(piece, Some(PieceType::Pawn));
                                    // en passant: a pawn capture that lands on an empty square
                                    if captured.is_none() && moved_pawn && fy != ty_ {
                                        captured = Some(PieceType::Pawn);
                                    }
                                    let last_rank = match color {
                                        Color::White => 0,
                                        Color::Black => 7,
                                    };
                                    let promoted = if moved_pawn && tx_ == last_rank {
                                        Some(promotion.unwrap_or(PieceType::Queen))
                                    } else {
                                        None
                                    };
                                    if let Some(piece) = piece {
                                        session.moves.push(MoveRecord {
                                            color,
                                            piece,
                                            from_x: fx as usize,
                                            from_y: fy as usize,
                                            to_x: tx_ as usize,
                                            to_y: ty_ as usize,
                                            captured,
                                            promotion: promoted,
                                            created_at: DateTime::now(),
                                        });
                                    }
                                    if let Some(taken) = captured {
                                        match color {
                                            Color::White => session.captured_by_white.push(taken),
                                            Color::Black => session.captured_by_black.push(taken),
                                        }
                                    }

                                    let state_msg = state_json(
                                        session,
                                        Some(json!({
                                            "from": { "x": fx, "y": fy },
                                            "to":   { "x": tx_, "y": ty_ }
                                        })),
                                    );
                                    let _ = tx.send(Message::Text(state_msg.clone()));
                                    let other = match color {
                                        Color::White => &session.black_tx,
                                        Color::Black => &session.white_tx,
                                    };
                                    if let Some(otx) = other {
                                        let _ = otx.send(Message::Text(state_msg));
                                    }

                                    let final_status = match session.game.status() {
                                        // turn already switched: the side to move is the loser
                                        GameStatus::Checkmate => Some(match color {
                                            Color::White => game_schema::GameStatus::WhiteWon,
                                            Color::Black => game_schema::GameStatus::BlackWon,
                                        }),
                                        GameStatus::Stalemate => {
                                            Some(game_schema::GameStatus::Draw)
                                        }
                                        _ => None,
                                    };
                                    if let Some(status) = final_status {
                                        if !session.persisted && session.has_signed_in_player() {
                                            session.persisted = true;
                                            finished_doc = Some(session.to_game_doc(status));
                                        }
                                    }
                                } else {
                                    let _ = tx.send(Message::Text(
                                        json!({"type":"error","message":"invalid move"})
                                            .to_string(),
                                    ));
                                }
                            }
                        }
                        if let Some(doc) = finished_doc {
                            persist_game(&state, doc);
                        }
                    }

                    "moves" => {
                        let (gid, color) = match (&game_id, my_color) {
                            (Some(g), Some(c)) => (g.clone(), c),
                            _ => continue,
                        };
                        let x = v["x"].as_u64().unwrap_or(0) as u8;
                        let y = v["y"].as_u64().unwrap_or(0) as u8;

                        let games = state.games.lock().unwrap();
                        if let Some(session) = games.get(&gid) {
                            if session.game.current_turn() != color {
                                let _ = tx.send(Message::Text(
                                    json!({"type":"possible_moves","moves":[]}).to_string(),
                                ));
                                continue;
                            }
                            let moves: Vec<Value> = session
                                .game
                                .possible_moves_from(Position { x, y })
                                .iter()
                                .map(|p| json!({"x": p.x, "y": p.y}))
                                .collect();
                            let _ = tx.send(Message::Text(
                                json!({"type":"possible_moves","moves":moves}).to_string(),
                            ));
                        }
                    }

                    _ => {}
                }
            }
            Some(Ok(_)) => {} // ignore binary / ping / pong
        }
    }

    // Cleanup: remove sender slot, notify opponent, persist abandoned games
    if let (Some(gid), Some(color)) = (game_id, my_color) {
        let mut abandoned_doc: Option<game_schema::Game> = None;
        {
            let mut games = state.games.lock().unwrap();
            if let Some(session) = games.get_mut(&gid) {
                let disconnect_msg = json!({"type":"opponent_disconnected"}).to_string();
                match color {
                    Color::White => {
                        session.white_tx = None;
                        if let Some(btx) = &session.black_tx {
                            let _ = btx.send(Message::Text(disconnect_msg));
                        }
                    }
                    Color::Black => {
                        session.black_tx = None;
                        if let Some(wtx) = &session.white_tx {
                            let _ = wtx.send(Message::Text(disconnect_msg));
                        }
                    }
                }
                if session.started && !session.persisted && session.has_signed_in_player() {
                    session.persisted = true;
                    abandoned_doc =
                        Some(session.to_game_doc(game_schema::GameStatus::Abandoned));
                }
            }
            games.retain(|_, s| s.white_tx.is_some() || s.black_tx.is_some());
        }
        if let Some(doc) = abandoned_doc {
            persist_game(&state, doc);
        }
    }
}
