use crate::controllers::servers::AppState;
use crate::services::ssh_pool::SshPool;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use russh::ChannelMsg;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct WsAppState {
    pub app: AppState,
    pub pool: SshPool,
}

#[derive(Deserialize, Debug)]
pub struct WsQuery {
    pub server_id: i64,
    pub session: String,
    pub bookmark_id: Option<i64>,
    #[serde(default = "default_cols")]
    pub cols: u32,
    #[serde(default = "default_rows")]
    pub rows: u32,
}

fn default_cols() -> u32 {
    80
}
fn default_rows() -> u32 {
    24
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
pub enum WsClientMsg {
    #[serde(rename = "input")]
    Input { data: String },
    #[serde(rename = "resize")]
    Resize { cols: u32, rows: u32 },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum WsServerMsg {
    #[serde(rename = "status")]
    Status { status: String },
    #[serde(rename = "exit")]
    Exit { code: Option<u32> },
    #[serde(rename = "error")]
    Error { message: String },
}

pub fn router(state: WsAppState) -> Router {
    Router::new()
        .route("/ws/terminal", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<WsAppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, query, state))
}

async fn handle_socket(mut socket: WebSocket, query: WsQuery, state: WsAppState) {
    if !state.app.vault.is_session_valid(&query.session).await {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&WsServerMsg::Error {
                    message: "Unauthorized: Invalid or expired session".to_string(),
                })
                .unwrap(),
            ))
            .await;
        let _ = socket.close().await;
        return;
    }

    // Send connecting status
    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&WsServerMsg::Status {
                status: "connecting".to_string(),
            })
            .unwrap(),
        ))
        .await;

    let kek = match state.app.vault.get_kek().await {
        Ok(k) => k,
        Err(_) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&WsServerMsg::Error {
                        message: "Vault is locked".to_string(),
                    })
                    .unwrap(),
                ))
                .await;
            return;
        }
    };

    let creds = match state.app.db.get_server_credentials(query.server_id, &kek) {
        Ok(c) => c,
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&WsServerMsg::Error {
                        message: format!("Server not found: {}", e),
                    })
                    .unwrap(),
                ))
                .await;
            return;
        }
    };

    let session = match state.pool.get_or_connect(&creds).await {
        Ok(s) => s,
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::to_string(&WsServerMsg::Error {
                        message: format!("SSH connection failed: {}", e),
                    })
                    .unwrap(),
                ))
                .await;
            return;
        }
    };

    let mut channel = {
        let handle = session.handle.lock().await;
        match handle.channel_open_session().await {
            Ok(c) => c,
            Err(e) => {
                let _ = socket
                    .send(Message::Text(
                        serde_json::to_string(&WsServerMsg::Error {
                            message: format!("Failed to open channel: {}", e),
                        })
                        .unwrap(),
                    ))
                    .await;
                return;
            }
        }
    };

    if let Err(e) = channel
        .request_pty(
            false,
            "xterm-256color",
            query.cols.max(10),
            query.rows.max(5),
            0,
            0,
            &[],
        )
        .await
    {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&WsServerMsg::Error {
                    message: format!("Failed to request PTY: {}", e),
                })
                .unwrap(),
            ))
            .await;
        return;
    }

    if let Err(e) = channel.request_shell(true).await {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&WsServerMsg::Error {
                    message: format!("Failed to start shell: {}", e),
                })
                .unwrap(),
            ))
            .await;
        return;
    }

    // Send connected status
    let _ = socket
        .send(Message::Text(
            serde_json::to_string(&WsServerMsg::Status {
                status: "connected".to_string(),
            })
            .unwrap(),
        ))
        .await;

    // Execute startup bookmark command if provided
    if let Some(bm_id) = query.bookmark_id {
        if let Ok(bm) = state.app.db.get_bookmark(bm_id) {
            let cmd = if bm.startup_cmd.is_empty() {
                format!("cd \"{}\"\n", bm.path)
            } else {
                format!("cd \"{}\" && {}\n", bm.path, bm.startup_cmd)
            };
            let _ = channel.data(cmd.as_bytes()).await;
        }
    }

    let (mut ws_sender, mut ws_receiver) = socket.split();

    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        if ws_sender.send(Message::Binary(data.to_vec())).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        let _ = ws_sender
                            .send(Message::Text(
                                serde_json::to_string(&WsServerMsg::Exit {
                                    code: Some(exit_status),
                                })
                                .unwrap(),
                            ))
                            .await;
                        break;
                    }
                    Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
            ws_msg = ws_receiver.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(client_msg) = serde_json::from_str::<WsClientMsg>(&text) {
                            match client_msg {
                                WsClientMsg::Input { data } => {
                                    if channel.data(data.as_bytes()).await.is_err() {
                                        break;
                                    }
                                }
                                WsClientMsg::Resize { cols, rows } => {
                                    let _ = channel
                                        .window_change(cols.max(10), rows.max(5), 0, 0)
                                        .await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if channel.data(&bytes[..]).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
