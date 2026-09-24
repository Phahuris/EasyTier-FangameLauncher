//! Etape 4 : transport generique Launcher <-> Launcher sur le reseau EasyTier.
//! - bind 0.0.0.0:0 (port data dynamique)
//! - peers = IP virtuelles EasyTier (fournies par le GUI)
//! - chaque paquet porte reply_port pour apprendre l endpoint distant
//! PAS de gameplay, PAS de FGL_Net, PAS de modification du chat.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

fn discovery_port(network: &str) -> u16 {
    let mut h: u32 = 2166136261;
    for b in network.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619);
    }
    40000 + (h % 20000) as u16
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LinkPacket {
    pub v: u32,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub reply_port: u16,
    #[serde(default)]
    pub ts: u64,
}

#[derive(Clone, Debug)]
struct Endpoint {
    ip: IpAddr,
    port: u16,
}

pub struct LinkState {
    pub sock: Mutex<Option<Arc<UdpSocket>>>,
    pub port: Mutex<u16>,
    /// IP EasyTier des peers (sans port)
    pub peer_ips: Mutex<Vec<IpAddr>>,
    /// ip -> dernier port data connu
    pub endpoints: Mutex<HashMap<IpAddr, u16>>,
    pub my_pseudo: Mutex<String>,
}

impl Default for LinkState {
    fn default() -> Self {
        Self {
            sock: Mutex::new(None),
            port: Mutex::new(0),
            peer_ips: Mutex::new(Vec::new()),
            endpoints: Mutex::new(HashMap::new()),
            my_pseudo: Mutex::new(String::new()),
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
pub async fn fgl_link_start(app: AppHandle, state: State<'_, LinkState>) -> Result<u16, String> {
    let mut guard = state.sock.lock().await;
    if let Some(sock) = guard.as_ref() {
        let p = sock
            .local_addr()
            .map(|a| a.port())
            .unwrap_or(*state.port.lock().await);
        return Ok(p);
    }
    let sock = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("fgl_link bind 0.0.0.0:0: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    println!("[FGL_LINK] bind 0.0.0.0:{port}");
    *state.port.lock().await = port;
    let sock = Arc::new(sock);
    *guard = Some(sock.clone());
    drop(guard);

    let app2 = app.clone();
    // Note: endpoints updates via fgl_link_on_packet side — reader emits only
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    if n == 0 {
                        continue;
                    }
                    let Ok(txt) = std::str::from_utf8(&buf[..n]) else {
                        continue;
                    };
                    let Ok(pkt) = serde_json::from_str::<LinkPacket>(txt) else {
                        continue;
                    };
                    let _ = app2.emit(
                        "fgl_link_message",
                        serde_json::json!({
                            "kind": pkt.kind,
                            "from": pkt.from,
                            "payload": pkt.payload,
                            "reply_port": pkt.reply_port,
                            "ip": from.ip().to_string(),
                            "src_port": from.port(),
                        }),
                    );
                }
                Err(_) => break,
            }
        }
    });

    Ok(port)
}

#[tauri::command]
pub async fn fgl_link_get_port(state: State<'_, LinkState>) -> Result<u16, String> {
    let p = *state.port.lock().await;
    if p == 0 {
        Err("link not started".into())
    } else {
        Ok(p)
    }
}

#[tauri::command]
pub async fn fgl_link_set_pseudo(state: State<'_, LinkState>, pseudo: String) -> Result<(), String> {
    *state.my_pseudo.lock().await = pseudo.trim().to_string();
    Ok(())
}

/// IPs EasyTier des autres joueurs (deja connues via le GUI / collect_network_info).
#[tauri::command]
pub async fn fgl_link_set_peers(state: State<'_, LinkState>, ips: Vec<String>) -> Result<(), String> {
    let mut list = Vec::new();
    for s in ips {
        if let Some(ip) = parse_ip(&s) {
            list.push(ip);
        }
    }
    *state.peer_ips.lock().await = list;
    Ok(())
}

/// Enregistre le port data d un peer (apres reception ou annonce).
#[tauri::command]
pub async fn fgl_link_remember(
    state: State<'_, LinkState>,
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

/// Annonce periodique : envoie kind=announce avec reply_port vers chaque peer
/// sur le port deja connu (si connu). Le premier contact peut venir de l autre
/// cote qui connait deja notre port, ou d un echange ulterieur.
#[tauri::command]
pub async fn fgl_link_announce(state: State<'_, LinkState>) -> Result<(), String> {
    let my_port = *state.port.lock().await;
    if my_port == 0 {
        return Ok(());
    }
    let pseudo = state.my_pseudo.lock().await.clone();
    let sock = {
        let g = state.sock.lock().await;
        match g.as_ref() {
            Some(s) => s.clone(),
            None => return Ok(()),
        }
    };
    let pkt = LinkPacket {
        v: 1,
        kind: "announce".into(),
        from: pseudo,
        payload: String::new(),
        reply_port: my_port,
        ts: now_ts(),
    };
    let data = serde_json::to_vec(&pkt).map_err(|e| e.to_string())?;
    let peers = state.peer_ips.lock().await.clone();
    let eps = state.endpoints.lock().await.clone();
    for ip in peers {
        if let Some(&port) = eps.get(&ip) {
            let _ = sock.send_to(&data, SocketAddr::new(ip, port)).await;
        } else {
            let dport = discovery_port("fangame");
            let _ = sock.send_to(&data, SocketAddr::new(ip, dport)).await;
        }
    }
    }
    Ok(())
}

/// Envoi generique d information Launcher -> Launcher (etape 4).
#[tauri::command]
pub async fn fgl_link_send(
    state: State<'_, LinkState>,
    kind: String,
    payload: String,
    ips: Vec<String>,
) -> Result<u32, String> {
    let my_port = *state.port.lock().await;
    if my_port == 0 {
        return Err("link not started".into());
    }
    let sock = {
        let g = state.sock.lock().await;
        match g.as_ref() {
            Some(s) => s.clone(),
            None => return Err("link not started".into()),
        }
    };
    let pseudo = state.my_pseudo.lock().await.clone();
    let pkt = LinkPacket {
        v: 1,
        kind,
        from: pseudo,
        payload,
        reply_port: my_port,
        ts: now_ts(),
    };
    let data = serde_json::to_vec(&pkt).map_err(|e| e.to_string())?;
    let eps = state.endpoints.lock().await.clone();
    let mut sent = 0u32;
    for s in ips {
        let Some(ip) = parse_ip(&s) else { continue };
        let port = match eps.get(&ip) {
            Some(&p) if p > 0 => p,
            _ => continue, // pas d endpoint connu: pas d envoi fantome
        };
        if sock.send_to(&data, SocketAddr::new(ip, port)).await.is_ok() {
            sent += 1;
        }
    }
    Ok(sent)
}