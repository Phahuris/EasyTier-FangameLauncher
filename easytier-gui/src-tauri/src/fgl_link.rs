//! FGL_Link: Launcher <-> Launcher sur le reseau EasyTier (UDP).
//! Data socket: 0.0.0.0:0 + reply_port dans chaque paquet.
//! Discovery socket: 0.0.0.0:discovery_port (si libre) pour le 1er contact.
//! Endpoints IP -> port conserves (pas d'effacement au refresh).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;


fn fgl_trace(msg: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("fgl_player_trace.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{}", msg);
    }
}
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

pub struct LinkState {
    pub sock: Mutex<Option<Arc<UdpSocket>>>,
    pub port: Mutex<u16>,
    pub peer_ips: Mutex<Vec<IpAddr>>,
    /// IP EasyTier -> port UDP FGL_Link (ne jamais clear globalement)
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

async fn spawn_reader(app: AppHandle, sock: Arc<UdpSocket>) {
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

                    // Bootstrap FGL_Link:
                    // - apprendre endpoints[from.ip()] = pkt.reply_port (port DATA distant)
                    // - si ce n'est pas deja un announce, repondre sur ip:reply_port
                    //   pour que le pair apprenne NOTRE port DATA (via notre reply_port)
                    // - kind "announce" => pas de reponse (coupe la boucle)
                    // - kind "announce" n'est jamais livre a FGL_Net (seul "player" l'est)
                    if let Some(link) = app.try_state::<LinkState>() {
                        let remote_data = pkt.reply_port;
                        if remote_data > 0 {
                            let ip = from.ip();
                            let mut learned_new = false;
                            {
                                let mut eps = link.endpoints.lock().await;
                                match eps.get(&ip).copied() {
                                    Some(p) if p == remote_data => {}
                                    _ => {
                                        eps.insert(ip, remote_data);
                                        learned_new = true;
                                        fgl_trace(&format!(
                                            "[PEER LEARN] ip={} port={}",
                                            ip, remote_data
                                        ));
                                    }
                                }
                            }
                            {
                                let mut list = link.peer_ips.lock().await;
                                if !list.contains(&ip) {
                                    list.push(ip);
                                }
                            }
                            // Reponse bootstrap: player/autre -> announce ; announce -> silence
                            if pkt.kind != "announce" {
                                let my_port = *link.port.lock().await;
                                let sock_data = {
                                    let g = link.sock.lock().await;
                                    g.as_ref().cloned()
                                };
                                if my_port > 0 {
                                    if let Some(sock_data) = sock_data {
                                        let pseudo = link.my_pseudo.lock().await.clone();
                                        let reply = LinkPacket {
                                            v: 1,
                                            kind: "announce".into(),
                                            from: pseudo,
                                            payload: String::new(),
                                            reply_port: my_port,
                                            ts: now_ts(),
                                        };
                                        if let Ok(data) = serde_json::to_vec(&reply) {
                                            let dest = SocketAddr::new(ip, remote_data);
                                            match sock_data.send_to(&data, dest).await {
                                                Ok(_) => {
                                                    fgl_trace(&format!(
                                                        "[PEER REPLY] to={}:{} learned_new={}",
                                                        ip, remote_data, learned_new
                                                    ));
                                                }
                                                Err(e) => {
                                                    fgl_trace(&format!(
                                                        "[PEER REPLY FAIL] to={}:{} err={}",
                                                        ip, remote_data, e
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Link -> IPC local game
                    if pkt.kind == "player" && !pkt.payload.is_empty() {
                        if let Some(ipc) = app.try_state::<crate::fgl_ipc::IpcState>() {
                            let line = if pkt.payload.starts_with("PLAYER|") {
                                pkt.payload.clone()
                            } else {
                                format!("PLAYER|{}", pkt.payload)
                            };
                            fgl_trace(&format!("[PLAYER IN LINK] from={} kind=player bytes={}", pkt.from, line.len()));
                            let _ = crate::fgl_ipc::deliver_to_game(&ipc, &line).await;
                            fgl_trace("[PLAYER IN IPC] delivered");
                        }
                    }

                    let _ = app.emit(
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
}


/// Appele depuis fgl_ipc: PLAYER local -> autres launchers.
pub async fn relay_player(state: &LinkState, payload: &str) -> u32 {
    let my_port = *state.port.lock().await;
    if my_port == 0 {
        return 0;
    }
    let sock = {
        let g = state.sock.lock().await;
        match g.as_ref() {
            Some(s) => s.clone(),
            None => return 0,
        }
    };
    let pseudo = state.my_pseudo.lock().await.clone();
    let body = payload.strip_prefix("PLAYER|").unwrap_or(payload);
    let pkt = LinkPacket {
        v: 1,
        kind: "player".into(),
        from: pseudo,
        payload: body.to_string(),
        reply_port: my_port,
        ts: now_ts(),
    };
    let Ok(data) = serde_json::to_vec(&pkt) else {
        return 0;
    };
    let peers = state.peer_ips.lock().await.clone();
    let eps = state.endpoints.lock().await.clone();
    let dport = discovery_port("fangame");
    let n_peers = peers.len();
    let n_eps = eps.len();
    let mut sent = 0u32;
    for ip in peers {
        if let Some(&port) = eps.get(&ip) {
            if port > 0 {
                match sock.send_to(&data, SocketAddr::new(ip, port)).await {
                    Ok(_) => {
                        sent += 1;
                        fgl_trace(&format!("[PLAYER OUT LINK PEER] ip={} port={}", ip, port));
                    }
                    Err(e) => fgl_trace(&format!("[PLAYER OUT LINK PEER FAIL] ip={} port={} err={}", ip, port, e)),
                }
            }
        } else {
            fgl_trace(&format!("[PLAYER OUT LINK PEER] ip={} no endpoint", ip));
        }
        match sock.send_to(&data, SocketAddr::new(ip, dport)).await {
            Ok(_) => {
                sent += 1;
                fgl_trace(&format!("[PLAYER OUT LINK DISCOVERY] ip={} port={}", ip, dport));
            }
            Err(e) => fgl_trace(&format!("[PLAYER OUT LINK DISCOVERY FAIL] ip={} port={} err={}", ip, dport, e)),
        }
    }
    if sent > 0 {
        fgl_trace(&format!("[PLAYER OUT LINK] peers={} sent={}", n_peers, sent));
    } else {
        fgl_trace(&format!("[PLAYER OUT LINK] no send peers={} eps={}", n_peers, n_eps));
    }
    sent
}
#[tauri::command]
pub async fn fgl_link_start(app: AppHandle, state: State<'_, LinkState>) -> Result<u16, String> {
    {
        let guard = state.sock.lock().await;
        if let Some(sock) = guard.as_ref() {
            let p = sock
                .local_addr()
                .map(|a| a.port())
                .unwrap_or(*state.port.lock().await);
            return Ok(p);
        }
    }

    let sock = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("fgl_link bind 0.0.0.0:0: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    println!("[FGL_LINK] data 0.0.0.0:{port}");
    *state.port.lock().await = port;
    let sock = Arc::new(sock);
    {
        let mut guard = state.sock.lock().await;
        *guard = Some(sock.clone());
    }

    spawn_reader(app.clone(), sock).await;

    // Bootstrap: ecoute discovery_port si libre (2e instance meme PC: skip OK)
    let dport = discovery_port("fangame");
    match UdpSocket::bind(("0.0.0.0", dport)).await {
        Ok(ds) => {
            println!("[FGL_LINK] discovery 0.0.0.0:{dport}");
            spawn_reader(app, Arc::new(ds)).await;
        }
        Err(e) => println!("[FGL_LINK] discovery {dport} skip: {e}"),
    }

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
pub async fn fgl_link_set_pseudo(
    state: State<'_, LinkState>,
    pseudo: String,
) -> Result<(), String> {
    *state.my_pseudo.lock().await = pseudo.trim().to_string();
    Ok(())
}

#[tauri::command]
pub async fn fgl_link_set_peers(
    state: State<'_, LinkState>,
    ips: Vec<String>,
) -> Result<(), String> {
    // Merge: ajoute les IPs, ne retire pas les endpoints deja appris
    let mut list = state.peer_ips.lock().await.clone();
    for s in ips {
        if let Some(ip) = parse_ip(&s) {
            if !list.contains(&ip) {
                list.push(ip);
            }
        }
    }
    *state.peer_ips.lock().await = list.clone();
    fgl_trace(&format!("[PEER SET] count={} ips={:?}", list.len(), list));
    Ok(())
}

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
    fgl_trace(&format!("[PEER REMEMBER] ip={} port={}", ip, port));
    // garder l'IP dans peer_ips aussi
    let mut list = state.peer_ips.lock().await;
    if !list.contains(&ip) {
        list.push(ip);
    }
    Ok(())
}

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
    let dport = discovery_port("fangame");
    for ip in peers {
        if let Some(&port) = eps.get(&ip) {
            let _ = sock.send_to(&data, SocketAddr::new(ip, port)).await;
        }
        let _ = sock.send_to(&data, SocketAddr::new(ip, dport)).await;
    }
    Ok(())
}

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
    let dport = discovery_port("fangame");
    let mut sent = 0u32;
    for s in ips {
        let Some(ip) = parse_ip(&s) else {
            continue;
        };
        if let Some(&port) = eps.get(&ip) {
            if port > 0 && sock.send_to(&data, SocketAddr::new(ip, port)).await.is_ok() {
                sent += 1;
            }
        }
        if sock
            .send_to(&data, SocketAddr::new(ip, dport))
            .await
            .is_ok()
        {
            sent += 1;
        }
    }
    Ok(sent)
}