use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use mongodb::bson::DateTime;
use serde_json::{Value, json};
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

use super::{AppState, LocalGameSession, SessionUser, color_str, new_game_id, piece_type_str, promotion_from_str};
use crate::board::game::GameStatus;
use crate::db::game_schema::{self, Move as MoveRecord};
use crate::pieces::pieces::{Color, PieceType, Position};
use crate::redis_state::{
    self, RedisGameState, TTL_ACTIVE, TTL_DISCONNECTED, TTL_PERSISTED, TTL_WAITING,
    hydrate::{board_json_from_redis, game_to_redis, new_redis_state, redis_to_game},
    lock::RedisLock,
};

fn persist_game(state: &AppState, doc: game_schema::Game) {
    let db = state.db.clone();
    tokio::spawn(async move {
        if let Err(e) = db.games.insert_one(doc).await {
            eprintln!("failed to persist game: {e}");
        }
    });
}

fn has_signed_in_player(s: &RedisGameState) -> bool {
    s.white_user_id.is_some() || s.black_user_id.is_some()
}

fn to_game_doc(s: &RedisGameState, status: game_schema::GameStatus) -> game_schema::Game {
    use mongodb::bson::oid::ObjectId;
    use std::str::FromStr;
    game_schema::Game {
        id: None,
        white_user_id: s
            .white_user_id
            .as_deref()
            .and_then(|h| ObjectId::from_str(h).ok()),
        black_user_id: s
            .black_user_id
            .as_deref()
            .and_then(|h| ObjectId::from_str(h).ok()),
        white_name: s.white_user_name.clone(),
        black_name: s.black_user_name.clone(),
        moves: s.moves.clone(),
        status,
        created_at: DateTime::from_millis(s.created_at_ms),
        updated_at: DateTime::now(),
    }
}

fn captured_json(pieces: &[PieceType]) -> serde_json::Value {
    json!(pieces.iter().map(|&p| piece_type_str(p)).collect::<Vec<_>>())
}

fn players_json(s: &RedisGameState) -> serde_json::Value {
    json!({
        "white": s.white_user_name,
        "black": s.black_user_name,
    })
}

fn state_json_from_redis(
    rs: &RedisGameState,
    game_status: &str,
    last_move: Option<Value>,
    server_id: &str,
) -> String {
    let mut msg = json!({
        "type": "state",
        "board": board_json_from_redis(rs),
        "turn": color_str(rs.turn),
        "status": game_status,
        "players": players_json(rs),
        "captured": {
            "white": captured_json(&rs.captured_by_white),
            "black": captured_json(&rs.captured_by_black),
        },
        "delivered_by": server_id,
    });
    if let Some(lm) = last_move {
        msg["lastMove"] = lm;
    }
    msg.to_string()
}

fn game_status_str(rs: &RedisGameState) -> &'static str {
    // Reconstruct the game to compute status — only called after a move.
    let game = redis_to_game(rs);
    match game.status() {
        GameStatus::Ongoing => "ongoing",
        GameStatus::Check => "check",
        GameStatus::Checkmate => "checkmate",
        GameStatus::Stalemate => "stalemate",
    }
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
                        let game = crate::board::game::Game::new();
                        let rs = new_redis_state(&game, user.as_ref());

                        if let Err(e) =
                            redis_state::save_state(&state.redis, &gid, &rs, TTL_WAITING).await
                        {
                            eprintln!("create: save_state failed: {e}");
                            continue;
                        }

                        let cancel = CancellationToken::new();
                        {
                            let mut games = state.games.lock().unwrap();
                            games.insert(gid.clone(), LocalGameSession::new_white(tx.clone(), cancel.clone()));
                        }

                        tokio::spawn(crate::redis_state::pubsub::pubsub_listener(
                            state.redis_url.clone(),
                            gid.clone(),
                            tx.clone(),
                            cancel,
                            state.server_id.clone(),
                        ));

                        game_id = Some(gid.clone());
                        my_color = Some(Color::White);
                        let _ = tx.send(Message::Text(
                            json!({"type":"joined","game_id":gid,"color":"white"}).to_string().into(),
                        ));
                    }

                    "join" => {
                        let gid = match v["game_id"].as_str() {
                            Some(id) => id.to_string(),
                            None => continue,
                        };

                        let rs_opt = match redis_state::load_state(&state.redis, &gid).await {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("join: load_state failed: {e}");
                                continue;
                            }
                        };

                        let rs = match rs_opt {
                            None => {
                                let _ = tx.send(Message::Text(
                                    json!({"type":"error","message":"game not found"}).to_string().into(),
                                ));
                                continue;
                            }
                            Some(s) => s,
                        };

                        if rs.black_user_id.is_some()
                            || state.games.lock().unwrap().get(&gid).map_or(false, |s| s.black_tx.is_some())
                        {
                            let _ = tx.send(Message::Text(
                                json!({"type":"error","message":"game is full"}).to_string().into(),
                            ));
                            continue;
                        }

                        let lock = match RedisLock::acquire(&state.redis, &gid, &state.server_id).await {
                            Ok(l) => l,
                            Err(e) => {
                                eprintln!("join: lock failed: {e}");
                                continue;
                            }
                        };

                        let mut rs = match redis_state::load_state(&state.redis, &gid).await {
                            Ok(Some(s)) => s,
                            _ => { lock.release().await; continue; }
                        };

                        rs.started = true;
                        if let Some(u) = &user {
                            rs.black_user_id = Some(u.id.to_hex());
                            rs.black_user_name = Some(u.name.clone());
                        }

                        if let Err(e) = redis_state::save_state(&state.redis, &gid, &rs, TTL_ACTIVE).await {
                            eprintln!("join: save_state failed: {e}");
                            lock.release().await;
                            continue;
                        }
                        lock.release().await;

                        let cancel = CancellationToken::new();
                        {
                            let mut games = state.games.lock().unwrap();
                            let session = games.entry(gid.clone()).or_insert_with(|| LocalGameSession {
                                white_tx: None,
                                black_tx: None,
                                white_cancel: None,
                                black_cancel: None,
                            });
                            session.black_tx = Some(tx.clone());
                            session.black_cancel = Some(cancel.clone());
                        }

                        tokio::spawn(crate::redis_state::pubsub::pubsub_listener(
                            state.redis_url.clone(),
                            gid.clone(),
                            tx.clone(),
                            cancel,
                            state.server_id.clone(),
                        ));

                        game_id = Some(gid.clone());
                        my_color = Some(Color::Black);

                        let status = game_status_str(&rs);
                        let state_msg = state_json_from_redis(&rs, status, None, &state.server_id);

                        let _ = tx.send(Message::Text(
                            json!({"type":"joined","game_id":&gid,"color":"black"}).to_string().into(),
                        ));
                        let _ = tx.send(Message::Text(state_msg.clone().into()));

                        // Notify white (may be on another server via pub/sub).
                        publish(&state, &gid, &state_msg).await;
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

                        let lock = match RedisLock::acquire(&state.redis, &gid, &state.server_id).await {
                            Ok(l) => l,
                            Err(e) => {
                                eprintln!("move: lock failed: {e}");
                                continue;
                            }
                        };

                        let mut rs = match redis_state::load_state(&state.redis, &gid).await {
                            Ok(Some(s)) => s,
                            _ => { lock.release().await; continue; }
                        };

                        if rs.turn != color {
                            lock.release().await;
                            let _ = tx.send(Message::Text(
                                json!({"type":"error","message":"not your turn"}).to_string().into(),
                            ));
                            continue;
                        }

                        let mut game = redis_to_game(&rs);
                        let from = Position { x: fx, y: fy };
                        let to = Position { x: tx_, y: ty_ };

                        let piece = game.board().get_piece(from).map(|p| p.piece_type());
                        let mut captured = game.board().get_piece(to).map(|p| p.piece_type());

                        if !game.make_move(from, to, promotion) {
                            lock.release().await;
                            let _ = tx.send(Message::Text(
                                json!({"type":"error","message":"invalid move"}).to_string().into(),
                            ));
                            continue;
                        }

                        let moved_pawn = matches!(piece, Some(PieceType::Pawn));
                        if captured.is_none() && moved_pawn && fy != ty_ {
                            captured = Some(PieceType::Pawn);
                        }
                        let last_rank = match color { Color::White => 0, Color::Black => 7 };
                        let promoted = if moved_pawn && tx_ == last_rank {
                            Some(promotion.unwrap_or(PieceType::Queen))
                        } else {
                            None
                        };

                        if let Some(pt) = piece {
                            rs.moves.push(MoveRecord {
                                color,
                                piece: pt,
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
                                Color::White => rs.captured_by_white.push(taken),
                                Color::Black => rs.captured_by_black.push(taken),
                            }
                        }

                        // Sync board state back into RedisGameState.
                        rs = game_to_redis(&game, None, None, &rs);

                        let final_status = match game.status() {
                            GameStatus::Checkmate => Some(match color {
                                Color::White => game_schema::GameStatus::WhiteWon,
                                Color::Black => game_schema::GameStatus::BlackWon,
                            }),
                            GameStatus::Stalemate => Some(game_schema::GameStatus::Draw),
                            _ => None,
                        };

                        let mut finished_doc: Option<game_schema::Game> = None;
                        if let Some(status) = final_status {
                            if !rs.persisted && has_signed_in_player(&rs) {
                                rs.persisted = true;
                                finished_doc = Some(to_game_doc(&rs, status));
                            }
                        }

                        let ttl = if finished_doc.is_some() { TTL_PERSISTED } else { TTL_ACTIVE };
                        if let Err(e) = redis_state::save_state(&state.redis, &gid, &rs, ttl).await {
                            eprintln!("move: save_state failed: {e}");
                        }
                        lock.release().await;

                        let status_str = match game.status() {
                            GameStatus::Ongoing => "ongoing",
                            GameStatus::Check => "check",
                            GameStatus::Checkmate => "checkmate",
                            GameStatus::Stalemate => "stalemate",
                        };
                        let last_move_val = Some(json!({"from":{"x":fx,"y":fy},"to":{"x":tx_,"y":ty_}}));
                        let state_msg = state_json_from_redis(&rs, status_str, last_move_val, &state.server_id);

                        // Send directly to local connections.
                        {
                            let games = state.games.lock().unwrap();
                            if let Some(session) = games.get(&gid) {
                                if let Some(wtx) = &session.white_tx {
                                    let _ = wtx.send(Message::Text(state_msg.clone().into()));
                                }
                                if let Some(btx) = &session.black_tx {
                                    let _ = btx.send(Message::Text(state_msg.clone().into()));
                                }
                            }
                        }
                        // Publish for players on other servers.
                        publish(&state, &gid, &state_msg).await;

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

                        let rs = match redis_state::load_state(&state.redis, &gid).await {
                            Ok(Some(s)) => s,
                            _ => continue,
                        };

                        if rs.turn != color {
                            let _ = tx.send(Message::Text(
                                json!({"type":"possible_moves","moves":[]}).to_string().into(),
                            ));
                            continue;
                        }

                        let game = redis_to_game(&rs);
                        let moves: Vec<Value> = game
                            .possible_moves_from(Position { x, y })
                            .iter()
                            .map(|p| json!({"x": p.x, "y": p.y}))
                            .collect();
                        let _ = tx.send(Message::Text(
                            json!({"type":"possible_moves","moves":moves}).to_string().into(),
                        ));
                    }

                    "reconnect" => {
                        let gid = match v["game_id"].as_str() {
                            Some(id) => id.to_string(),
                            None => continue,
                        };
                        let color_str_val = match v["color"].as_str() {
                            Some(c) => c.to_string(),
                            None => continue,
                        };
                        let reconnect_color = match color_str_val.as_str() {
                            "white" => Color::White,
                            "black" => Color::Black,
                            _ => continue,
                        };

                        let rs = match redis_state::load_state(&state.redis, &gid).await {
                            Ok(Some(s)) => s,
                            _ => {
                                let _ = tx.send(Message::Text(
                                    json!({"type":"error","message":"game not found or expired"}).to_string().into(),
                                ));
                                continue;
                            }
                        };

                        // Validate identity.
                        let user_id_hex = user.as_ref().map(|u| u.id.to_hex());
                        let expected_id = match reconnect_color {
                            Color::White => rs.white_user_id.clone(),
                            Color::Black => rs.black_user_id.clone(),
                        };
                        if user_id_hex.is_none() || user_id_hex != expected_id {
                            let _ = tx.send(Message::Text(
                                json!({"type":"error","message":"unauthorized"}).to_string().into(),
                            ));
                            continue;
                        }

                        let cancel = CancellationToken::new();
                        {
                            let mut games = state.games.lock().unwrap();
                            let session = games.entry(gid.clone()).or_insert_with(|| LocalGameSession {
                                white_tx: None,
                                black_tx: None,
                                white_cancel: None,
                                black_cancel: None,
                            });
                            match reconnect_color {
                                Color::White => {
                                    if let Some(old) = session.white_cancel.take() { old.cancel(); }
                                    session.white_tx = Some(tx.clone());
                                    session.white_cancel = Some(cancel.clone());
                                }
                                Color::Black => {
                                    if let Some(old) = session.black_cancel.take() { old.cancel(); }
                                    session.black_tx = Some(tx.clone());
                                    session.black_cancel = Some(cancel.clone());
                                }
                            }
                        }

                        tokio::spawn(crate::redis_state::pubsub::pubsub_listener(
                            state.redis_url.clone(),
                            gid.clone(),
                            tx.clone(),
                            cancel,
                            state.server_id.clone(),
                        ));

                        game_id = Some(gid.clone());
                        my_color = Some(reconnect_color);

                        let status = game_status_str(&rs);
                        let state_msg = state_json_from_redis(&rs, status, None, &state.server_id);
                        let _ = tx.send(Message::Text(
                            json!({"type":"joined","game_id":&gid,"color":color_str_val}).to_string().into(),
                        ));
                        let _ = tx.send(Message::Text(state_msg.into()));

                        // Notify the opponent.
                        let notif = json!({"type":"opponent_reconnected"}).to_string();
                        let games = state.games.lock().unwrap();
                        if let Some(session) = games.get(&gid) {
                            let other = match reconnect_color {
                                Color::White => &session.black_tx,
                                Color::Black => &session.white_tx,
                            };
                            if let Some(otx) = other {
                                let _ = otx.send(Message::Text(notif.into()));
                            }
                        }
                    }

                    _ => {}
                }
            }
            Some(Ok(_)) => {}
        }
    }

    // Cleanup
    if let (Some(gid), Some(color)) = (game_id, my_color) {
        let mut abandoned_doc: Option<game_schema::Game> = None;

        {
            let mut games = state.games.lock().unwrap();
            if let Some(session) = games.get_mut(&gid) {
                let disconnect_msg = json!({"type":"opponent_disconnected"}).to_string();
                match color {
                    Color::White => {
                        if let Some(cancel) = session.white_cancel.take() { cancel.cancel(); }
                        session.white_tx = None;
                        if let Some(btx) = &session.black_tx {
                            let _ = btx.send(Message::Text(disconnect_msg.into()));
                        }
                    }
                    Color::Black => {
                        if let Some(cancel) = session.black_cancel.take() { cancel.cancel(); }
                        session.black_tx = None;
                        if let Some(wtx) = &session.white_tx {
                            let _ = wtx.send(Message::Text(disconnect_msg.into()));
                        }
                    }
                }
            }
            games.retain(|_, s| s.white_tx.is_some() || s.black_tx.is_some());
        }

        // Load Redis state to check if we need to persist an abandoned game.
        if let Ok(Some(mut rs)) = redis_state::load_state(&state.redis, &gid).await {
            if rs.started && !rs.persisted && has_signed_in_player(&rs) {
                rs.persisted = true;
                abandoned_doc = Some(to_game_doc(&rs, game_schema::GameStatus::Abandoned));
                let _ = redis_state::save_state(&state.redis, &gid, &rs, TTL_PERSISTED).await;
            } else {
                let _ = redis_state::save_state(&state.redis, &gid, &rs, TTL_DISCONNECTED).await;
            }
        }

        if let Some(doc) = abandoned_doc {
            persist_game(&state, doc);
        }
    }
}

async fn publish(state: &AppState, game_id: &str, msg: &str) {
    use bb8_redis::redis::AsyncCommands;
    if let Ok(mut conn) = state.redis.get().await {
        let channel = crate::redis_state::pubsub_channel(game_id);
        let _: Result<i64, _> = conn.publish(channel, msg).await;
    }
}
