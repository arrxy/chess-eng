use axum::extract::ws::Message;
use bb8_redis::redis::Client;
use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::server::Tx;
use super::pubsub_channel;

/// Subscribes to `game:{game_id}:events` and forwards payloads to `tx`.
/// Stops when `cancel` is triggered (player disconnect).
/// Skips messages that carry `"delivered_by"` matching `own_server_id`
/// to avoid double-delivery when both players are on the same server.
pub async fn pubsub_listener(
    redis_url: String,
    game_id: String,
    tx: Tx,
    cancel: CancellationToken,
    own_server_id: String,
) {
    let client = match Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pubsub: failed to open Redis client: {e}");
            return;
        }
    };
    let conn = match client.get_async_pubsub().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pubsub: failed to get pubsub connection: {e}");
            return;
        }
    };
    let channel = pubsub_channel(&game_id);
    let mut pubsub = conn;
    if let Err(e) = pubsub.subscribe(&channel).await {
        eprintln!("pubsub: failed to subscribe to {channel}: {e}");
        return;
    }
    let mut stream = pubsub.on_message();
    loop {
        tokio::select! {
            Some(msg) = stream.next() => {
                if let Ok(payload) = msg.get_payload::<String>() {
                    // Skip if this server already sent the message directly.
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&payload) {
                        if v.get("delivered_by").and_then(|s| s.as_str()) == Some(&own_server_id) {
                            continue;
                        }
                        // Strip the delivery metadata before forwarding.
                        let mut fwd = v;
                        fwd.as_object_mut().map(|o| o.remove("delivered_by"));
                        let _ = tx.send(Message::Text(fwd.to_string().into()));
                    }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}
