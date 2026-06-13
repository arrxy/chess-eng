use std::time::Duration;

use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message,
    tungstenite::client::IntoClientRequest, tungstenite::http::HeaderValue,
};

pub type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Open a WebSocket. When `cookie` is set it is sent as `session=<cookie>` so
/// the server treats the connection as signed-in.
pub async fn connect(url: &str, cookie: Option<&str>) -> Result<Ws> {
    let mut req = url.into_client_request()?;
    if let Some(c) = cookie {
        req.headers_mut()
            .insert("Cookie", HeaderValue::from_str(&format!("session={c}"))?);
    }
    let (ws, _resp) = connect_async(req).await?;
    Ok(ws)
}

pub async fn send(ws: &mut Ws, v: &Value) -> Result<()> {
    ws.send(Message::Text(v.to_string())).await?;
    Ok(())
}

/// Read frames until one of type `typ` arrives. A `{type:error}` frame aborts
/// with an error; other frame types are skipped.
pub async fn recv_until(ws: &mut Ws, typ: &str, timeout_ms: u64) -> Result<Value> {
    let deadline = Duration::from_millis(timeout_ms);
    loop {
        let msg = tokio::time::timeout(deadline, ws.next())
            .await
            .map_err(|_| anyhow!("timeout waiting for `{typ}`"))?
            .ok_or_else(|| anyhow!("connection closed waiting for `{typ}`"))??;
        match msg {
            Message::Text(t) => {
                let v: Value = serde_json::from_str(&t)?;
                let kind = v["type"].as_str().unwrap_or("");
                if kind == typ {
                    return Ok(v);
                }
                if kind == "error" {
                    return Err(anyhow!(
                        "server error: {}",
                        v["message"].as_str().unwrap_or("?")
                    ));
                }
                // otherwise ignore (e.g. opponent_reconnected) and keep reading
            }
            Message::Close(_) => return Err(anyhow!("connection closed waiting for `{typ}`")),
            _ => {} // ping/pong/binary
        }
    }
}

pub async fn close(ws: &mut Ws) {
    let _ = ws.close(None).await;
}
