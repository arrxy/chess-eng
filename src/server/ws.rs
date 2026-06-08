use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc::unbounded_channel;

use crate::board::game::{Game, GameStatus};
use crate::pieces::pieces::{Color, Position};
use super::{AppState, GameSession, board_json, color_str, new_game_id};

pub async fn handle_socket(socket: WebSocket, state: AppState) {
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
                            GameSession {
                                game: Game::new(),
                                white_tx: Some(tx.clone()),
                                black_tx: None,
                            },
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
                                game_id = Some(gid.clone());
                                my_color = Some(Color::Black);

                                let brd = board_json(&session.game);
                                let state_msg = json!({
                                    "type": "state",
                                    "board": brd,
                                    "turn": "white",
                                    "status": "ongoing"
                                })
                                .to_string();

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

                        let mut games = state.games.lock().unwrap();
                        if let Some(session) = games.get_mut(&gid) {
                            if session.game.current_turn() != color {
                                let _ = tx.send(Message::Text(
                                    json!({"type":"error","message":"not your turn"}).to_string(),
                                ));
                                continue;
                            }
                            let from = Position { x: fx, y: fy };
                            let to = Position { x: tx_, y: ty_ };
                            if session.game.make_move(from, to) {
                                let brd = board_json(&session.game);
                                let turn = color_str(session.game.current_turn());
                                let status = match session.game.status() {
                                    GameStatus::Ongoing => "ongoing",
                                    GameStatus::Check => "check",
                                    GameStatus::Checkmate => "checkmate",
                                    GameStatus::Stalemate => "stalemate",
                                };
                                let state_msg = json!({
                                    "type": "state",
                                    "board": brd,
                                    "turn": turn,
                                    "status": status
                                })
                                .to_string();
                                let _ = tx.send(Message::Text(state_msg.clone()));
                                let other = match color {
                                    Color::White => &session.black_tx,
                                    Color::Black => &session.white_tx,
                                };
                                if let Some(otx) = other {
                                    let _ = otx.send(Message::Text(state_msg));
                                }
                            } else {
                                let _ = tx.send(Message::Text(
                                    json!({"type":"error","message":"invalid move"}).to_string(),
                                ));
                            }
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

    // Cleanup: remove sender slot and notify opponent
    if let (Some(gid), Some(color)) = (game_id, my_color) {
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
        }
        games.retain(|_, s| s.white_tx.is_some() || s.black_tx.is_some());
    }
}
