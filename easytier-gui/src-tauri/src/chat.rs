use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

/// Port de decouverte (meme numero pour la salle). Bind sur IP EasyTier locale
/// pour autoriser 2 launchers sur le meme PC (2 IP virtuelles differentes).
fn discovery_port(network: &str) -> u16 {
    let mut h: u32 = 2166136261;
    for b in network.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619);
    }
    40000 + (h % 20000) as u16
}

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
    #[serde(default)]
    pub reply_port: u16,
}

pub struct ChatState {
    pub socket: Mutex<Option<Arc<UdpSocket>>>,
    pub disc: Mutex<Option<Arc<UdpSocket>>>,
    pub port: Mutex<u16>,
    pub endpoints: Mutex<HashMap<IpAddr, u16>>,
    pub network_name: Mutex<String>,
    pub local_ip: Mutex<Option<IpAddr>>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            socket: Mutex::new(None),
            disc: Mutex::new(None),
            port: Mutex::new(0),
            endpoints: Mutex::new(HashMap::new()),
            network_name: Mutex::new("fangame".into()),
            local_ip: Mutex::new(None),
        }
    }
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_ip(s: &str) -> Option<IpAddr> {
    s.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()
}

#[tauri::command]
pub async fn chat_set_network_name(
    state: State<'_, ChatState>,
    name: String,
) -> Result<(), String> {
    *state.network_name.lock().await = if name.trim().is_empty() {
        "fangame".into()
    } else {
        name.trim().into()
    };
    Ok(())
}

#[tauri::command]
pub async fn chat_set_local_ip(state: State<'_, ChatState>, ip: String) -> Result<(), String> {
    *state.local_ip.lock().await = parse_ip(&ip);
    Ok(())
}

#[tauri::command]
pub async fn chat_start(app: AppHandle, state: State<'_, ChatState>) -> Result<(), String> {
    // --- data plane dynamique ---
    {
        let mut guard = state.socket.lock().await;
        if guard.is_none() {
            let sock = UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(|e| format!("chat data bind: {e}"))?;
            let port = sock.local_addr().map_err(|e| e.to_string())?.port();
            println!("[CHAT] data 0.0.0.0:{port}");
            *state.port.lock().await = port;
            let sock = Arc::new(sock);
            *guard = Some(sock.clone());
            let app2 = app.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 65535];
                loop {
                    match sock.recv_from(&mut buf).await {
                        Ok((n, from)) => {
                            if n == 0 {
                                continue;
                            }
                            let Ok(s) = std::str::from_utf8(&buf[..n]) else { continue };
                            let Ok(mut pkt) = serde_json::from_str::<ChatPacket>(s) else {
                                continue;
                            };
                            if pkt.reply_port == 0 {
                                pkt.reply_port = from.port();
                            }
                            let _ = app2.emit("chat_message", &pkt);
                            let _ = app2.emit(
                                "chat_endpoint",
                                serde_json::json!({
                                    "ip": from.ip().to_string(),
                                    "port": pkt.reply_port,
                                }),
                            );
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    }

    // --- discovery plane (bootstrap) ---
    {
        let mut guard = state.disc.lock().await;
        if guard.is_none() {
            let net = state.network_name.lock().await.clone();
            let dport = discovery_port(&net);
            let bind_addr = if let Some(ip) = *state.local_ip.lock().await {
                format!("{ip}:{dport}")
            } else {
                // fallback 1 launcher / machine ; 2e instance meme IP peut echouer -> ignore
                format!("0.0.0.0:{dport}")
            };
            match UdpSocket::bind(&bind_addr).await {
                Ok(sock) => {
                    println!("[CHAT] discovery {bind_addr}");
                    let sock = Arc::new(sock);
                    *guard = Some(sock.clone());
                    let app2 = app.clone();
                    tokio::spawn(async move {
                        let mut buf = vec![0u8; 4096];
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
                                    let port = if pkt.reply_port > 0 {
                                        pkt.reply_port
                                    } else {
                                        from.port()
                                    };
                                    let _ = app2.emit(
                                        "chat_endpoint",
                                        serde_json::json!({
                                            "ip": from.ip().to_string(),
                                            "port": port,
                                        }),
                                    );
                                    if pkt.kind == "chat" || pkt.kind == "cmd" {
                                        let _ = app2.emit("chat_message", &pkt);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }
                Err(e) => eprintln!("[CHAT] discovery bind skip: {e}"),
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn chat_stop(state: State<'_, ChatState>) -> Result<(), String> {
    *state.socket.lock().await = None;
    *state.disc.lock().await = None;
    *state.port.lock().await = 0;
    state.endpoints.lock().await.clear();
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
    let Some(ip) = parse_ip(&ip) else {
        return Ok(());
    };
    state.endpoints.lock().await.insert(ip, port);
    Ok(())
}

/// Bootstrap: annonce reply_port vers discovery_port de chaque peer (meme si endpoint inconnu).
#[tauri::command]
pub async fn chat_bootstrap(state: State<'_, ChatState>, peers: Vec<String>) -> Result<(), String> {
    let my_port = *state.port.lock().await;
    if my_port == 0 {
        return Ok(());
    }
    let net = state.network_name.lock().await.clone();
    let dport = discovery_port(&net);
    let sock = {
        let d = state.disc.lock().await;
        if let Some(s) = d.as_ref() {
            s.clone()
        } else {
            let g = state.socket.lock().await;
            match g.as_ref() {
                Some(s) => s.clone(),
                None => return Ok(()),
            }
        }
    };
    let pkt = ChatPacket {
        v: 1,
        kind: "announce".into(),
        pseudo: String::new(),
        text: String::new(),
        plugin: String::new(),
        action: String::new(),
        ts: now_ts(),
        reply_port: my_port,
    };
    let data = serde_json::to_vec(&pkt).map_err(|e| e.to_string())?;
    for ip_s in peers {
        let Some(ip) = parse_ip(&ip_s) else { continue };
        let _ = sock.send_to(&data, SocketAddr::new(ip, dport)).await;
        // si endpoint data deja connu, annonce aussi dessus
        if let Some(&p) = state.endpoints.lock().await.get(&ip) {
            let _ = sock.send_to(&data, SocketAddr::new(ip, p)).await;
        }
    }
    Ok(())
}

async fn send_to_peers(state: &ChatState, sock: &UdpSocket, data: &[u8], peers: Vec<String>) {
    let eps = state.endpoints.lock().await.clone();
    let net = state.network_name.lock().await.clone();
    let dport = discovery_port(&net);
    for ip_s in peers {
        let Some(ip) = parse_ip(&ip_s) else { continue };
        if let Some(&port) = eps.get(&ip) {
            let _ = sock.send_to(data, SocketAddr::new(ip, port)).await;
        } else {
            // bootstrap: discovery tant que le port data est inconnu
            let _ = sock.send_to(data, SocketAddr::new(ip, dport)).await;
            println!("[CHAT TX] bootstrap discovery {ip}:{dport}");
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