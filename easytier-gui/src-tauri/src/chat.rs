use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub const CHAT_PORT: u16 = 37777;

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatPacket {
    pub v: u32,

    #[serde(rename = "type")]
    pub kind: String,

    pub pseudo: String,

    #[serde(default)]
    pub text: String,

    #[serde(default)]
    pub plugin: String,

    #[serde(default)]
    pub action: String,

    #[serde(default)]
    pub ts: u64,
}

pub struct ChatState {
    pub socket: Mutex<Option<Arc<UdpSocket>>>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            socket: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub async fn chat_start(
    app: AppHandle,
    state: State<'_, ChatState>,
) -> Result<(), String> {
    let mut guard = state.socket.lock().await;

    if guard.is_some() {
        return Ok(());
    }

    let sock = UdpSocket::bind(format!("0.0.0.0:{}", CHAT_PORT))
        .await
        .map_err(|e| format!("chat bind: {}", e))?;

    let sock = Arc::new(sock);

    *guard = Some(sock.clone());

    drop(guard);

    let app2 = app.clone();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];

        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        if let Ok(pkt) = serde_json::from_str::<ChatPacket>(s) {
                            println!(
                                "[CHAT RX] {}:{} -> {} : {}",
                                from.ip(),
                                from.port(),
                                pkt.pseudo,
                                pkt.text
                            );

                            let _ = app2.emit("chat_message", pkt);
                        }
                    }
                }

                Err(e) => {
                    eprintln!("[CHAT RX] socket stopped: {}", e);
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn chat_stop(
    state: State<'_, ChatState>,
) -> Result<(), String> {
    let mut guard = state.socket.lock().await;
    *guard = None;
    Ok(())
}

fn parse_peer_ip(ip: &str) -> Option<IpAddr> {
    let clean = ip
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');

    clean.parse::<IpAddr>().ok()
}

async fn send_to_peers(
    sock: &UdpSocket,
    data: &[u8],
    peers: Vec<String>,
) {
    for ip in peers {
        let ip = ip.trim();

        if ip.is_empty() {
            continue;
        }

        let Some(ip) = parse_peer_ip(ip) else {
            eprintln!("[CHAT TX] IP EasyTier invalide: {}", ip);
            continue;
        };

        let addr = SocketAddr::new(ip, CHAT_PORT);

        match sock.send_to(data, addr).await {
            Ok(n) => {
                println!(
                    "[CHAT TX] {} octets -> {}",
                    n,
                    addr
                );
            }

            Err(e) => {
                eprintln!(
                    "[CHAT TX] erreur vers {} : {}",
                    addr,
                    e
                );
            }
        }
    }
}

#[tauri::command]
pub async fn chat_send(
    state: State<'_, ChatState>,
    pseudo: String,
    text: String,
    peers: Vec<String>,
) -> Result<(), String> {
    let guard = state.socket.lock().await;

    let Some(sock) = guard.as_ref() else {
        return Err("chat non demarre".into());
    };

    let text = text.trim();

    if text.is_empty() {
        return Ok(());
    }

    let pkt = ChatPacket {
        v: 1,
        kind: "chat".into(),
        pseudo,
        text: text.to_string(),
        plugin: String::new(),
        action: String::new(),
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    let data =
        serde_json::to_vec(&pkt)
            .map_err(|e| e.to_string())?;

    send_to_peers(sock, &data, peers).await;

    Ok(())
}

#[tauri::command]
pub async fn chat_send_cmd(
    state: State<'_, ChatState>,
    pseudo: String,
    plugin: String,
    action: String,
    peers: Vec<String>,
) -> Result<(), String> {
    let guard = state.socket.lock().await;

    let Some(sock) = guard.as_ref() else {
        return Err("chat non demarre".into());
    };

    let pkt = ChatPacket {
        v: 1,
        kind: "cmd".into(),
        pseudo,
        text: String::new(),
        plugin,
        action,
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };

    let data =
        serde_json::to_vec(&pkt)
            .map_err(|e| e.to_string())?;

    send_to_peers(sock, &data, peers).await;

    Ok(())
}