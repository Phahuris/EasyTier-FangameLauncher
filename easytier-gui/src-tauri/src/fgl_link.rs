//! FGL_Link — bootstrap endpoint + TOUS les logs sur le Bureau

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

fn extract_seq(payload: &str) -> u64 {
    let body = payload.strip_prefix("PLAYER|").unwrap_or(payload);
    body.rsplit('|')
        .next()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

fn fgl_trace(msg: &str) {
    use std::io::Write;
    // Bureau UNIQUEMENT (pas TEMP)
    let path = std::env::var("USERPROFILE")
        .ok()
        .map(|u| std::path::PathBuf::from(u).join("Desktop").join("fgl_player_trace.log"))
        .unwrap_or_else(|| std::env::temp_dir().join("fgl_player_trace.log"));
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
    pub endpoints: Mutex<HashMap<IpAddr, u16>>,
    pub my_pseudo: Mutex<String>,
    pub discovery_bound: Mutex<bool>,
}

impl Default for LinkState {
    fn default() -> Self {
        Self {
            sock: Mutex::new(None),
            port: Mutex::new(0),
            peer_ips: Mutex::new(Vec::new()),
            endpoints: Mutex::new(HashMap::new()),
            my_pseudo: Mutex::new(String::new()),
            discovery_bound: Mutex::new(false),
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

fn dest_addrs(ip: IpAddr, port: u16) -> Vec<SocketAddr> {
    let mut v = vec![SocketAddr::new(ip, port)];
    if !ip.is_loopback() {
        v.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    v
}

async fn learn_endpoint(link: &LinkState, from_ip: IpAddr, remote_data: u16) -> bool {
    if remote_data == 0 {
        fgl_trace(&format!("PEER_LEARN_SKIP from_ip={} err=reply_port_0", from_ip));
        return false;
    }
    let mut learned_new = false;
    let peers: Vec<IpAddr> = link.peer_ips.lock().await.clone();
    fgl_trace(&format!(
        "PEER_LEARN_TRY from_ip={} reply_port={} peers={:?}",
        from_ip, remote_data, peers
    ));

    {
        let mut eps = link.endpoints.lock().await;

        if !from_ip.is_loopback() {
            match eps.get(&from_ip).copied() {
                Some(p) if p == remote_data => {
                    fgl_trace(&format!("PEER_LEARN_SAME ip={} port={}", from_ip, remote_data));
                }
                Some(old) => {
                    eps.insert(from_ip, remote_data);
                    learned_new = true;
                    fgl_trace(&format!(
                        "PEER_LEARN ip={} port={} old_port={} (src EasyTier)",
                        from_ip, remote_data, old
                    ));
                }
                None => {
                    eps.insert(from_ip, remote_data);
                    learned_new = true;
                    fgl_trace(&format!(
                        "PEER_LEARN ip={} port={} (src EasyTier)",
                        from_ip, remote_data
                    ));
                }
            }
        } else {
            fgl_trace(&format!(
                "PEER_LEARN src=127.0.0.1 port={} -> map EasyTier peers only",
                remote_data
            ));
        }

        for peer in &peers {
            if peer.is_loopback() {
                continue;
            }
            match eps.get(peer).copied() {
                Some(p) => {
                    fgl_trace(&format!(
                        "PEER_LEARN_FILL_SKIP peer={} already_port={}",
                        peer, p
                    ));
                }
                None => {
                    eps.insert(*peer, remote_data);
                    learned_new = true;
                    fgl_trace(&format!(
                        "PEER_LEARN_FILL peer={} port={} (from_src={})",
                        peer, remote_data, from_ip
                    ));
                }
            }
        }
        fgl_trace(&format!("ENDPOINTS_NOW {:?}", *eps));
    }

    if !from_ip.is_loopback() {
        let mut list = link.peer_ips.lock().await;
        if !list.contains(&from_ip) {
            list.push(from_ip);
            fgl_trace(&format!("PEER_IPS_ADD ip={}", from_ip));
        }
    }
    learned_new
}

async fn send_announce_to(sock: &UdpSocket, dest: SocketAddr, pseudo: &str, my_port: u16) {
    let reply = LinkPacket {
        v: 1,
        kind: "announce".into(),
        from: pseudo.to_string(),
        payload: String::new(),
        reply_port: my_port,
        ts: now_ts(),
    };
    match serde_json::to_vec(&reply) {
        Ok(data) => match sock.send_to(&data, dest).await {
            Ok(n) => fgl_trace(&format!("TX_ANNOUNCE to={} bytes={}", dest, n)),
            Err(e) => fgl_trace(&format!("TX_ANNOUNCE_ERROR to={} err={}", dest, e)),
        },
        Err(e) => fgl_trace(&format!("TX_ANNOUNCE_ERROR to={} err=serialize {}", dest, e)),
    }
}

async fn spawn_reader(app: AppHandle, sock: Arc<UdpSocket>, label: &str) {
    let label = label.to_string();
    fgl_trace(&format!("SPAWN_READER label={}", label));
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    if n == 0 {
                        continue;
                    }
                    fgl_trace(&format!("RX_RAW {} from={} bytes={}", label, from, n));
                    let Ok(txt) = std::str::from_utf8(&buf[..n]) else {
                        fgl_trace(&format!("RX_LINK_ERROR {} from={} err=utf8", label, from));
                        continue;
                    };
                    let Ok(pkt) = serde_json::from_str::<LinkPacket>(txt) else {
                        fgl_trace(&format!(
                            "RX_LINK_ERROR {} from={} err=json preview={:.60}",
                            label, from, txt
                        ));
                        continue;
                    };

                    fgl_trace(&format!(
                        "RX_LINK {} kind={} from_field={} from_ip={} reply_port={} src_port={}",
                        label, pkt.kind, pkt.from, from.ip(), pkt.reply_port, from.port()
                    ));

                    if let Some(link) = app.try_state::<LinkState>() {
                        if pkt.reply_port > 0 {
                            let learned = learn_endpoint(&link, from.ip(), pkt.reply_port).await;
                            fgl_trace(&format!(
                                "PEER_LEARN_DONE learned_new={} from_ip={} port={}",
                                learned, from.ip(), pkt.reply_port
                            ));

                            if pkt.kind != "announce" {
                                let my_port = *link.port.lock().await;
                                let sock_data = {
                                    let g = link.sock.lock().await;
                                    g.as_ref().cloned()
                                };
                                let pseudo = link.my_pseudo.lock().await.clone();
                                if my_port == 0 {
                                    fgl_trace("PEER_REPLY_SKIP err=my_port_0");
                                } else if let Some(sock_data) = sock_data {
                                    let dest = SocketAddr::new(from.ip(), pkt.reply_port);
                                    fgl_trace(&format!("PEER_REPLY_TRY to={}", dest));
                                    send_announce_to(&sock_data, dest, &pseudo, my_port).await;
                                    if from.ip().is_loopback() {
                                        let peers = link.peer_ips.lock().await.clone();
                                        for peer in peers {
                                            if peer.is_loopback() {
                                                continue;
                                            }
                                            let d = SocketAddr::new(peer, pkt.reply_port);
                                            fgl_trace(&format!("PEER_REPLY_TRY to={} (easytier)", d));
                                            send_announce_to(&sock_data, d, &pseudo, my_port).await;
                                        }
                                    }
                                } else {
                                    fgl_trace("PEER_REPLY_SKIP err=no_sock");
                                }
                            } else {
                                fgl_trace("PEER_REPLY_SKIP kind=announce");
                            }
                        } else {
                            fgl_trace(&format!(
                                "PEER_LEARN_SKIP {} kind={} err=no_reply_port",
                                label, pkt.kind
                            ));
                        }
                    } else {
                        fgl_trace("RX_LINK_ERROR no LinkState");
                    }

                    if pkt.kind == "player" && !pkt.payload.is_empty() {
                        if let Some(ipc) = app.try_state::<crate::fgl_ipc::IpcState>() {
                            let line = if pkt.payload.starts_with("PLAYER|") {
                                pkt.payload.clone()
                            } else {
                                format!("PLAYER|{}", pkt.payload)
                            };
                            let seq = extract_seq(&line);
                            fgl_trace(&format!(
                                "RX_EASYTIER seq={} from={} bytes={}",
                                seq, pkt.from, line.len()
                            ));
                            match crate::fgl_ipc::deliver_to_game(&ipc, &line).await {
                                Ok(()) => fgl_trace(&format!("TX_IPC seq={} (after RX_EASYTIER)", seq)),
                                Err(e) => fgl_trace(&format!("TX_IPC_ERROR seq={} err={}", seq, e)),
                            }
                        } else {
                            fgl_trace("RX_EASYTIER_ERROR no IpcState");
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
                Err(e) => {
                    fgl_trace(&format!("SPAWN_READER_END {} err={}", label, e));
                    break;
                }
            }
        }
    });
}

pub async fn relay_player(state: &LinkState, payload: &str) -> u32 {
    let seq = extract_seq(payload);
    let my_port = *state.port.lock().await;
    fgl_trace(&format!("RELAY_PLAYER_BEGIN seq={} my_port={}", seq, my_port));
    if my_port == 0 {
        fgl_trace(&format!("TX_EASYTIER_ERROR seq={} err=link_port_0", seq));
        return 0;
    }
    let sock = {
        let g = state.sock.lock().await;
        match g.as_ref() {
            Some(s) => s.clone(),
            None => {
                fgl_trace(&format!("TX_EASYTIER_ERROR seq={} err=no_sock", seq));
                return 0;
            }
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
        fgl_trace(&format!("TX_EASYTIER_ERROR seq={} err=serialize", seq));
        return 0;
    };
    let peers = state.peer_ips.lock().await.clone();
    let eps = state.endpoints.lock().await.clone();
    let dport = discovery_port("fangame");
    fgl_trace(&format!(
        "RELAY_PLAYER_STATE seq={} peers={:?} endpoints={:?} dport={}",
        seq, peers, eps, dport
    ));
    let mut sent = 0u32;

    for ip in &peers {
        if let Some(&port) = eps.get(ip) {
            if port > 0 {
                for dest in dest_addrs(*ip, port) {
                    match sock.send_to(&data, dest).await {
                        Ok(n) => {
                            sent += 1;
                            fgl_trace(&format!(
                                "TX_EASYTIER seq={} dest={} bytes={} (data)",
                                seq, dest, n
                            ));
                        }
                        Err(e) => fgl_trace(&format!(
                            "TX_EASYTIER_ERROR seq={} dest={} err={} (data)",
                            seq, dest, e
                        )),
                    }
                }
            }
        } else {
            fgl_trace(&format!(
                "TX_EASYTIER_ERROR seq={} ip={} err=no_endpoint",
                seq, ip
            ));
        }
        for dest in dest_addrs(*ip, dport) {
            match sock.send_to(&data, dest).await {
                Ok(n) => {
                    sent += 1;
                    fgl_trace(&format!(
                        "TX_EASYTIER seq={} dest={} bytes={} (discovery)",
                        seq, dest, n
                    ));
                }
                Err(e) => fgl_trace(&format!(
                    "TX_EASYTIER_ERROR seq={} dest={} err={} (discovery)",
                    seq, dest, e
                )),
            }
        }
    }
    fgl_trace(&format!("RELAY_PLAYER_END seq={} sent={}", seq, sent));
    sent
}

async fn bootstrap_announce(state: &LinkState) {
    let my_port = *state.port.lock().await;
    if my_port == 0 {
        fgl_trace("BOOTSTRAP_ANNOUNCE_SKIP err=my_port_0");
        return;
    }
    let sock = {
        let g = state.sock.lock().await;
        match g.as_ref() {
            Some(s) => s.clone(),
            None => {
                fgl_trace("BOOTSTRAP_ANNOUNCE_SKIP err=no_sock");
                return;
            }
        }
    };
    let pseudo = state.my_pseudo.lock().await.clone();
    let peers = state.peer_ips.lock().await.clone();
    let eps = state.endpoints.lock().await.clone();
    let dport = discovery_port("fangame");
    fgl_trace(&format!(
        "BOOTSTRAP_ANNOUNCE my_port={} peers={:?} endpoints={:?} dport={}",
        my_port, peers, eps, dport
    ));
    for ip in peers {
        for dest in dest_addrs(ip, dport) {
            fgl_trace(&format!("BOOTSTRAP_TX discovery dest={}", dest));
            send_announce_to(&sock, dest, &pseudo, my_port).await;
        }
        if let Some(&port) = eps.get(&ip) {
            if port > 0 {
                for dest in dest_addrs(ip, port) {
                    fgl_trace(&format!("BOOTSTRAP_TX data dest={}", dest));
                    send_announce_to(&sock, dest, &pseudo, my_port).await;
                }
            }
        }
    }
    fgl_trace("BOOTSTRAP_ANNOUNCE_DONE");
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
            fgl_trace(&format!("LINK_START_ALREADY data_port={}", p));
            return Ok(p);
        }
    }

    let sock = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("fgl_link bind 0.0.0.0:0: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    println!("[FGL_LINK] data 0.0.0.0:{port}");
    fgl_trace(&format!("LINK_LISTEN data_port={}", port));
    *state.port.lock().await = port;
    let sock = Arc::new(sock);
    {
        let mut guard = state.sock.lock().await;
        *guard = Some(sock.clone());
    }

    spawn_reader(app.clone(), sock, "data").await;

    let dport = discovery_port("fangame");
    match UdpSocket::bind(("0.0.0.0", dport)).await {
        Ok(ds) => {
            println!("[FGL_LINK] discovery 0.0.0.0:{dport}");
            fgl_trace(&format!("LINK_LISTEN discovery_port={}", dport));
            *state.discovery_bound.lock().await = true;
            spawn_reader(app, Arc::new(ds), "discovery").await;
        }
        Err(e) => {
            println!("[FGL_LINK] discovery {dport} skip: {e}");
            fgl_trace(&format!("LINK_LISTEN discovery_port={} SKIP err={}", dport, e));
            *state.discovery_bound.lock().await = false;
        }
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
    let p = pseudo.trim().to_string();
    fgl_trace(&format!("SET_PSEUDO {}", p));
    *state.my_pseudo.lock().await = p;
    Ok(())
}

#[tauri::command]
pub async fn fgl_link_set_peers(
    state: State<'_, LinkState>,
    ips: Vec<String>,
) -> Result<(), String> {
    let mut list = state.peer_ips.lock().await.clone();
    for s in ips {
        if let Some(ip) = parse_ip(&s) {
            if !list.contains(&ip) {
                list.push(ip);
                fgl_trace(&format!("PEER_SET_ADD ip={}", ip));
            }
        } else {
            fgl_trace(&format!("PEER_SET_BAD ip_str={}", s));
        }
    }
    *state.peer_ips.lock().await = list.clone();
    fgl_trace(&format!("[PEER SET] count={} ips={:?}", list.len(), list));
    bootstrap_announce(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn fgl_link_remember(
    state: State<'_, LinkState>,
    ip: String,
    port: u16,
) -> Result<(), String> {
    if port == 0 {
        fgl_trace("PEER_REMEMBER_SKIP port=0");
        return Ok(());
    }
    let Some(parsed) = parse_ip(&ip) else {
        fgl_trace(&format!("PEER_REMEMBER_SKIP bad_ip={}", ip));
        return Ok(());
    };
    state.endpoints.lock().await.insert(parsed, port);
    fgl_trace(&format!("PEER_LEARN (remember) ip={} port={}", parsed, port));
    let mut list = state.peer_ips.lock().await;
    if !list.contains(&parsed) {
        list.push(parsed);
    }
    Ok(())
}

#[tauri::command]
pub async fn fgl_link_announce(state: State<'_, LinkState>) -> Result<(), String> {
    fgl_trace("FGL_LINK_ANNOUNCE_CMD");
    bootstrap_announce(&state).await;
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
        kind: kind.clone(),
        from: pseudo,
        payload,
        reply_port: my_port,
        ts: now_ts(),
    };
    let data = serde_json::to_vec(&pkt).map_err(|e| e.to_string())?;
    let eps = state.endpoints.lock().await.clone();
    let dport = discovery_port("fangame");
    let mut sent = 0u32;
    fgl_trace(&format!("FGL_LINK_SEND kind={} ips={:?} eps={:?}", kind, ips, eps));
    for s in ips {
        let Some(ip) = parse_ip(&s) else {
            continue;
        };
        if let Some(&port) = eps.get(&ip) {
            if port > 0 {
                for dest in dest_addrs(ip, port) {
                    if sock.send_to(&data, dest).await.is_ok() {
                        sent += 1;
                        fgl_trace(&format!("FGL_LINK_SEND_OK dest={}", dest));
                    }
                }
            }
        }
        for dest in dest_addrs(ip, dport) {
            if sock.send_to(&data, dest).await.is_ok() {
                sent += 1;
                fgl_trace(&format!("FGL_LINK_SEND_OK dest={} (discovery)", dest));
            }
        }
    }
    Ok(sent)
}