use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

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

    /// Port UDP de l'envoyeur (dynamique) pour repondre / 2 launchers meme PC
    #[serde(default)]
    pub reply_port: u16,
}

pub struct ChatState {
    pub socket: Mutex<Option<Arc<UdpSocket>>>,
    pub port: Mutex<u16>,
    /// IP EasyTier -> dernier port chat connu
    pub endpoints: Mutex<HashMap<IpAddr, u16>>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            socket: Mutex::new(None),
            port: Mutex::new(0),
            endpoints: Mutex::new(HashMap::new()),
        }
    }
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tauri::command]
pub async fn chat_start(app: AppHandle, state: State<'_, ChatState>) -> Result<(), String> {
    println!("[CHAT] ===== CHAT START =====");
    let mut guard = state.socket.lock().await;
    if guard.is_some() {
        println!("[CHAT] Socket deja demarre port={}", *state.port.lock().await);
        return Ok(());
    }

    // Port dynamique : 2 launchers sur le meme PC OK
    let sock = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("chat bind 0.0.0.0:0: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    println!("[CHAT] bind OK 0.0.0.0:{port}");
    *state.port.lock().await = port;

    let sock = Arc::new(sock);
    *guard = Some(sock.clone());
    drop(guard);

    let app2 = app.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    if n == 0 {
                        continue;
                    }
                    let Ok(s) = std::str::from_utf8(&buf[..n]) else {
                        continue;
                    };
                    let Ok(pkt) = serde_json::from_str::<ChatPacket>(s) else {
                        continue;
                    };
                    // Note: endpoints mis a jour cote send apres emit via reply_port dans le GUI
                    // Ici on re-emet avec ip source pour que le front puisse remember
                    let mut pkt_out = pkt;
                    if pkt_out.reply_port == 0 {
                        pkt_out.reply_port = from.port();
                    }
                    let _ = app2.emit("chat_message", &pkt_out);
                    let _ = app2.emit(
                        "chat_endpoint",
                        serde_json::json!({
                            "ip": from.ip().to_string(),
                            "port": if pkt_out.reply_port > 0 { pkt_out.reply_port } else { from.port() },
                        }),
                    );
                }
                Err(_) => break,
            }
        }
    });
    println!("[CHAT] ===== CHAT START OK =====");
    Ok(())
}

#[tauri::command]
pub async fn chat_stop(state: State<'_, ChatState>) -> Result<(), String> {
    let mut guard = state.socket.lock().await;
    *guard = None;
    *state.port.lock().await = 0;
    Ok(())
}

#[tauri::command]
pub async fn chat_remember_endpoint(
    state: State<'_, ChatState>,
    ip: String,
    port: u16,
) -> Result<(), String> {
    if port == 0 {
        return Ok(());
    }
    let clean = ip
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    let Ok(ip) = clean.parse::<IpAddr>() else {
        return Ok(());
    };
    state.endpoints.lock().await.insert(ip, port);
    Ok(())
}

fn parse_peer_ip(ip: &str) -> Option<IpAddr> {
    let clean = ip
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    clean.parse().ok()
}

async fn send_to_peers(state: &ChatState, sock: &UdpSocket, data: &[u8], peers: Vec<String>) {
    let eps = state.endpoints.lock().await.clone();
    for ip_s in peers {
        let Some(ip) = parse_peer_ip(&ip_s) else {
            continue;
        };
        // Port connu via reply_port, sinon on ne spam pas 37777
        let Some(&port) = eps.get(&ip) else {
            println!("[CHAT TX] pas d'endpoint pour {ip} — skip (attend announce/reply_port)");
            continue;
        };
        let addr = SocketAddr::new(ip, port);
        match sock.send_to(data, addr).await {
            Ok(n) => println!("[CHAT TX] {n} octets -> {addr}"),
            Err(e) => eprintln!("[CHAT TX] fail {addr}: {e}"),
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
    let my_port = *state.port.lock().await;
    let pkt = ChatPacket {
        v: 1,
        kind: "chat".into(),
        pseudo,
        text: text.to_string(),
        plugin: String::new(),
        action: String::new(),
        ts: now_ts(),
        reply_port: my_port,
    };
    let data = serde_json::to_vec(&pkt).map_err(|e| e.to_string())?;
    send_to_peers(&state, sock, &data, peers).await;
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
    let my_port = *state.port.lock().await;
    let pkt = ChatPacket {
        v: 1,
        kind: "cmd".into(),
        pseudo,
        text: String::new(),
        plugin,
        action,
        ts: now_ts(),
        reply_port: my_port,
    };
    let data = serde_json::to_vec(&pkt).map_err(|e| e.to_string())?;
    send_to_peers(&state, sock, &data, peers).await;
    Ok(())
}