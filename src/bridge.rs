// src/bridge.rs
// Run with: cargo run -- bridge
// Opens http://localhost:9001 — serve OpenRing UI from there

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Multipart, State,
    },
    http::{HeaderValue, Method, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::UdpSocket,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

// ── Shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    /// Live discovered devices: ip → DeviceInfo
    pub devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    /// Broadcast channel — sends ProgressEvent to all connected WebSocket clients
    pub tx: broadcast::Sender<ProgressEvent>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DeviceInfo {
    pub id:   String,
    pub name: String,
    pub ip:   String,
    pub kind: String, // "pc" | "phone" | "laptop" | "tablet"
}

#[derive(Clone, Serialize, Debug)]
#[serde(tag = "type")]
pub enum ProgressEvent {
    #[serde(rename = "discovered")]
    Discovered { device: DeviceInfo },
    #[serde(rename = "progress")]
    Progress { transfer_id: String, percent: f32, speed_mbps: f32 },
    #[serde(rename = "done")]
    Done { transfer_id: String, hash: String, bytes: u64 },
    #[serde(rename = "error")]
    Error { transfer_id: String, message: String },
}

// ── Build the router ──────────────────────────────────────────────────────────

pub fn make_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    Router::new()
        .route("/api/discover",   get(get_devices))
        .route("/api/send",       post(send_file))
        .route("/api/ws",         get(ws_handler))
        .route("/api/health",     get(health))
        .with_state(state)
        .layer(cors)
}

// ── Routes ────────────────────────────────────────────────────────────────────

/// GET /api/health
async fn health() -> &'static str { "OpenRing SPL Bridge v1.0" }

/// GET /api/discover — returns current device list
async fn get_devices(State(state): State<AppState>) -> Json<Vec<DeviceInfo>> {
    let map = state.devices.lock().unwrap();
    Json(map.values().cloned().collect())
}

/// POST /api/send — multipart: field "file" + field "ip" + optional field "port"
async fn send_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::from("transfer");
    let mut target_ip = String::new();
    let mut port: u16 = crate::config::SERVER_PORT as u16;

    // Parse multipart fields
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        match field.name().unwrap_or("") {
            "file" => {
                file_name = field
                    .file_name()
                    .unwrap_or("transfer")
                    .to_string();
                file_bytes = Some(field.bytes().await.unwrap_or_default().to_vec());
            }
            "ip" => {
                target_ip = field.text().await.unwrap_or_default();
            }
            "port" => {
                port = field
                    .text()
                    .await
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(port);
            }
            _ => {}
        }
    }

    let bytes = match file_bytes {
        Some(b) if !b.is_empty() => b,
        _ => return (StatusCode::BAD_REQUEST, "missing file").into_response(),
    };
    if target_ip.is_empty() {
        return (StatusCode::BAD_REQUEST, "missing ip").into_response();
    }

    // Write to temp file (SPL needs a file path)
    let tmp_path = std::env::temp_dir().join(&file_name);
    if let Err(e) = std::fs::write(&tmp_path, &bytes) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    let transfer_id = Uuid::new_v4().to_string();
    let tx          = state.tx.clone();
    let tid_clone   = transfer_id.clone();
    let ip_clone    = target_ip.clone();
    let size        = bytes.len() as u64;

    // Spawn SPL send in background, stream progress via broadcast channel
    tokio::spawn(async move {
        use crate::sender::send_file_spl;

        let result = send_file_spl(
            &ip_clone,
            port,
            &tmp_path,
            size,
            |percent, speed_mbps| {
                let _ = tx.send(ProgressEvent::Progress {
                    transfer_id: tid_clone.clone(),
                    percent,
                    speed_mbps,
                });
            },
        )
        .await;

        match result {
            Ok(hash) => {
                let _ = tx.send(ProgressEvent::Done {
                    transfer_id: tid_clone.clone(),
                    hash,
                    bytes: size,
                });
            }
            Err(e) => {
                let _ = tx.send(ProgressEvent::Error {
                    transfer_id: tid_clone.clone(),
                    message: e.to_string(),
                });
            }
        }

        let _ = std::fs::remove_file(&tmp_path);
    });

    Json(serde_json::json!({ "transfer_id": transfer_id, "ok": true })).into_response()
}

/// GET /api/ws — WebSocket, receives ProgressEvent broadcasts
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                if socket.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

// ── Discovery listener (runs forever in background) ───────────────────────────

pub fn start_discovery_listener(state: AppState) {
    let tx = state.tx.clone();
    let devices = state.devices.clone();

    std::thread::spawn(move || {
        let sock = match UdpSocket::bind(format!("0.0.0.0:{}", crate::config::DISCOVERY_PORT)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[bridge] discovery listener bind failed: {e}");
                return;
            }
        };
        sock.set_read_timeout(Some(Duration::from_secs(1))).ok();

        let mut buf = [0u8; 1024];
        loop {
            if let Ok((len, addr)) = sock.recv_from(&mut buf) {
                if &buf[..len] == b"SPL_HERE" {
                    let ip = addr.ip().to_string();
                    let mut map = devices.lock().unwrap();
                    if !map.contains_key(&ip) {
                        let dev = DeviceInfo {
                            id:   Uuid::new_v4().to_string(),
                            name: hostname_for(&ip),
                            ip:   ip.clone(),
                            kind: "pc".into(),
                        };
                        map.insert(ip, dev.clone());
                        let _ = tx.send(ProgressEvent::Discovered { device: dev });
                    }
                }
            }
        }
    });
}

/// Broadcast SPL_DISCOVER periodically
pub fn start_discovery_broadcaster() {
    std::thread::spawn(move || {
        let sock = UdpSocket::bind("0.0.0.0:0").expect("bind");
        sock.set_broadcast(true).ok();
        loop {
            sock.send_to(
                b"SPL_DISCOVER",
                format!("255.255.255.255:{}", crate::config::DISCOVERY_PORT),
            )
            .ok();
            std::thread::sleep(Duration::from_secs(3));
        }
    });
}

/// Respond to SPL_DISCOVER with SPL_HERE (so other devices find us)
pub fn start_discovery_responder() {
    std::thread::spawn(move || {
        let sock = UdpSocket::bind(format!("0.0.0.0:{}", crate::config::DISCOVERY_PORT)).ok();
        if sock.is_none() { return; } // already bound by listener
        let sock = sock.unwrap();
        let mut buf = [0u8; 1024];
        loop {
            if let Ok((len, addr)) = sock.recv_from(&mut buf) {
                if &buf[..len] == b"SPL_DISCOVER" {
                    sock.send_to(b"SPL_HERE", addr).ok();
                }
            }
        }
    });
}

fn hostname_for(ip: &str) -> String {
    // Optional: reverse DNS lookup. Falls back to IP.
    ip.to_string()
}
