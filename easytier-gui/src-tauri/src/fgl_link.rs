//! FGL_Link — anti-echo + endpoint distant = autre port local (1v1 meme PC)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
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

fn fgl_trace(_msg: &str) {
    // disabled: no fgl_player_trace.log (low-end PC)
}

fn discovery_port(network: &str) -> u16 {
    let mut h: u32 = 2166136261;
    for b in network.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619);
    }
    40000 + (h % 20000) as u16
}

fn local_ports_path() -> std::path::PathBuf {
    std::env::temp_dir().join("fgl_UNUSED_do_not_use.txt")
}

fn register_local_port(_my_port: u16) {\n    // no disk\n}\n

fn read_local_ports() -> std::collections::BTreeSet<u16> {\n    std::collections::BTreeSet::new()\n}\n

/// Autre instance sur ce PC (registre Bureau).
fn other_local_port(_my_port: u16) -> Option<u16> {\n    None\n}\n

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
    pub last_bootstrap: Mutex<Option<Instant>>,
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
            last_bootstrap: Mutex::new(None),
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

fn dest_addrs(ip: IpAddr, port: u16, my_port: u16) -> Vec<SocketAddr> {
    let mut v = Vec::new();
    if port == 0 || port == my_port {
        return v;
    }
    if !ip.is_loopback() {
        v.push(SocketAddr::new(ip, port)); // single dest (no dual localhost flood)
    }
    v
}

/// Force tous les peers -> autre port local (1v1 meme machine).
async fn apply_remote_port_fixed(_link: &LinkState, _my_port: u16) {
    // was forcing ports from test txt file — disabled
};
    let peers: Vec<IpAddr> = link
        .peer_ips
        .lock()
        .await
        .iter()
        .copied()
        .filter(|ip| !ip.is_loopback())
        .collect();
    let mut eps = link.endpoints.lock().await;
    for ip in &peers {
        // A) port data appris via announce
        if let Some(port) = eps.get(ip).copied() {
            if port != my_port && port != 0 && sent_to.insert((*ip, port)) {
                for dest in dest_addrs(*ip, port, my_port) {
                    if sock.send_to(&data, dest).await.is_ok() {
                        sent += 1;
                    }
                }
            }
        }
        // B) TOUJOURS discovery_port (meme machine / online) pour learn + livraison
        if sent_to.insert((*ip, dport)) {
            for dest in dest_addrs(*ip, dport, my_port) {
                if sock.send_to(&data, dest).await.is_ok() {
                    sent += 1;
                }
            }
        }
    }
    // plus de branche localhost/other_local_port (fichier test)
    fgl_trace(&format!("RELAY_PLAYER_END seq={} sent={}", seq, sent));
    sent
}

async fn bootstrap_announce(state: &LinkState) {
    {
        let mut last = state.last_bootstrap.lock().await;
        if let Some(t) = *last {
            if t.elapsed() < Duration::from_secs(3) {
                fgl_trace("BOOTSTRAP_ANNOUNCE_SKIP debounce");
                return;
            }
        }
        *last = Some(Instant::now());
    }
    let my_port = *state.port.lock().await;
    if my_port == 0 {
        return;
    }
    register_local_port(my_port);
    apply_remote_port_fixed(state, my_port).await;

    let sock = {
        let g = state.sock.lock().await;
        match g.as_ref() {
            Some(s) => s.clone(),
            None => return,
        }
    };
    let pseudo = state.my_pseudo.lock().await.clone();
    let peers: Vec<IpAddr> = state
        .peer_ips
        .lock()
        .await
        .iter()
        .copied()
        .filter(|ip| !ip.is_loopback())
        .collect();
    let dport = discovery_port("fangame");
    fgl_trace(&format!(
        "BOOTSTRAP_ANNOUNCE my_port={} peers={:?} other={:?}",
        my_port,
        peers,
        other_local_port(my_port)
    ));
    for ip in &peers {
        for dest in dest_addrs(*ip, dport, my_port) { // bootstrap discovery ON (peers learn ports)
            send_announce_to(&sock, dest, &pseudo, my_port).await;
        }
    }
    if let Some(rp) = other_local_port(my_port) {
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), rp);
        send_announce_to(&sock, dest, &pseudo, my_port).await;
    }
    fgl_trace("BOOTSTRAP_ANNOUNCE_DONE");
}

async fn bind_udp_reuse(addr: SocketAddr) -> Result<UdpSocket, String> {
    use socket2::{Domain, Protocol, Socket, Type};
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| format!("socket new: {e}"))?;
    socket.set_reuse_address(true).map_err(|e| format!("reuse: {e}"))?;
    socket.set_nonblocking(true).map_err(|e| format!("nb: {e}"))?;
    socket.bind(&addr.into()).map_err(|e| format!("bind {addr}: {e}"))?;
    let std_sock: std::net::UdpSocket = socket.into();
    UdpSocket::from_std(std_sock).map_err(|e| format!("from_std: {e}"))
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
        .map_err(|e| format!("bind: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    fgl_trace(&format!("LINK_LISTEN data_port={}", port));
    *state.port.lock().await = port;
    register_local_port(port);
    let sock = Arc::new(sock);
    *state.sock.lock().await = Some(sock.clone());
    spawn_reader(app.clone(), sock, "data").await;

    let dport = discovery_port("fangame");
    match bind_udp_reuse(SocketAddr::from(([0, 0, 0, 0], dport))).await {
        Ok(ds) => {
            fgl_trace(&format!("LINK_LISTEN discovery_port={} reuse=1", dport));
            *state.discovery_bound.lock().await = true;
            spawn_reader(app, Arc::new(ds), "discovery").await;
        }
        Err(e) => {
            fgl_trace(&format!("LINK_LISTEN discovery SKIP err={}", e));
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
    let mut list = Vec::new();
    for s in ips {
        if let Some(ip) = parse_ip(&s) {
            if !ip.is_loopback() && !list.contains(&ip) {
                list.push(ip);
                fgl_trace(&format!("PEER_SET_ADD ip={}", ip));
            }
        }
    }
    *state.peer_ips.lock().await = list.clone();
    fgl_trace(&format!("[PEER SET] count={} ips={:?}", list.len(), list));
    let my_port = *state.port.lock().await;
    apply_remote_port_fixed(&state, my_port).await;
    bootstrap_announce(&state).await;
    Ok(())
}

#[tauri::command]
pub async fn fgl_link_remember(
    state: State<'_, LinkState>,
    ip: String,
    port: u16,
) -> Result<(), String> {
    let my_port = *state.port.lock().await;
    if port == 0 || port == my_port {
        return Ok(());
    }
    let Some(parsed) = parse_ip(&ip) else {
        return Ok(());
    };
    if parsed.is_loopback() {
        return Ok(());
    }
    // sticky : ne pas ecraser un endpoint deja correct
    {
        let eps = state.endpoints.lock().await;
        if let Some(existing) = eps.get(&parsed).copied() {
            if existing != my_port && existing != port {
                fgl_trace(&format!(
                    "PEER_LEARN_STICKY_SKIP remember ip={} keep={} ignore={}",
                    parsed, existing, port
                ));
                return Ok(());
            }
            if existing == port {
                return Ok(());
            }
        }
    }
    // preferer other_local si dispo
    let use_port = other_local_port(my_port).unwrap_or(port);
    if use_port == my_port {
        return Ok(());
    }
    state.endpoints.lock().await.insert(parsed, use_port);
    fgl_trace(&format!(
        "PEER_LEARN_ACCEPT (remember) ip={} port={}",
        parsed, use_port
    ));
    let mut list = state.peer_ips.lock().await;
    list.retain(|i| !i.is_loopback());
    if !list.contains(&parsed) {
        list.push(parsed);
    }
    apply_remote_port_fixed(&state, my_port).await;
    Ok(())
}

#[tauri::command]
pub async fn fgl_link_announce(state: State<'_, LinkState>) -> Result<(), String> {
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
    apply_remote_port_fixed(&state, my_port).await;
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
        let Some(ip) = parse_ip(&s) else { continue };
        if ip.is_loopback() {
            continue;
        }
        let port = eps.get(&ip).copied().or_else(|| other_local_port(my_port));
        if let Some(port) = port {
            for dest in dest_addrs(ip, port, my_port) {
                if sock.send_to(&data, dest).await.is_ok() {
                    sent += 1;
                }
            }
        }
        for dest in dest_addrs(ip, dport, my_port) {
            if sock.send_to(&data, dest).await.is_ok() {
                sent += 1;
            }
        }
    }
    Ok(sent)
}