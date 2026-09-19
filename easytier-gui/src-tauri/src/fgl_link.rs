//! FGL_Link — anti self-echo + endpoints sticky (pas de map vers my_port)

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

fn fgl_trace(msg: &str) {
    use std::io::Write;
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

fn local_ports_path() -> std::path::PathBuf {
    std::env::var("USERPROFILE")
        .ok()
        .map(|u| std::path::PathBuf::from(u).join("Desktop").join("fgl_link_local_ports.txt"))
        .unwrap_or_else(|| std::env::temp_dir().join("fgl_link_local_ports.txt"))
}

fn register_local_port(my_port: u16) {
    if my_port == 0 {
        return;
    }
    let path = local_ports_path();
    let mut set = read_local_ports();
    set.insert(my_port);
    let body: String = set.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("\n");
    let _ = std::fs::write(&path, body);
    fgl_trace(&format!("LOCAL_PORT_REGISTER port={}", my_port));
}

fn read_local_ports() -> std::collections::BTreeSet<u16> {
    let mut set = std::collections::BTreeSet::new();
    let path = local_ports_path();
    if let Ok(txt) = std::fs::read_to_string(&path) {
        for line in txt.lines() {
            if let Ok(p) = line.trim().parse::<u16>() {
                if p > 0 {
                    set.insert(p);
                }
            }
        }
    }
    set
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

/// Destinations pour un peer : IP EasyTier + loopback UNIQUEMENT si port != my_port.
fn dest_addrs(ip: IpAddr, port: u16, my_port: u16) -> Vec<SocketAddr> {
    let mut v = Vec::new();
    if port == 0 {
        return v;
    }
    // Jamais envoyer vers mon propre data port (anti self-echo)
    if port == my_port {
        return v;
    }
    if !ip.is_loopback() {
        v.push(SocketAddr::new(ip, port));
        // meme PC via loopback vers le port DISTANT seulement
        v.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    v
}

/// Apprend un endpoint distant. JAMAIS my_port. Sticky : ne pas ecraser un vrai port distant par self.
async fn learn_endpoint(
    link: &LinkState,
    from_ip: IpAddr,
    remote_data: u16,
    my_port: u16,
) -> bool {
    if remote_data == 0 {
        fgl_trace(&format!(
            "PEER_LEARN_REJECT_SELF ip={} port=0 reason=zero",
            from_ip
        ));
        return false;
    }
    if remote_data == my_port {
        fgl_trace(&format!(
            "PEER_LEARN_REJECT_SELF ip={} port={} reason=my_port",
            from_ip, remote_data
        ));
        return false;
    }

    let mut learned_new = false;
    let peers: Vec<IpAddr> = link
        .peer_ips
        .lock()
        .await
        .iter()
        .copied()
        .filter(|ip| !ip.is_loopback())
        .collect();

    fgl_trace(&format!(
        "PEER_LEARN_TRY from_ip={} reply_port={} my_port={} peers={:?}",
        from_ip, remote_data, my_port, peers
    ));

    {
        let mut eps = link.endpoints.lock().await;

        // 1) Source EasyTier non-loopback : apprendre / mettre a jour ce peer uniquement
        if !from_ip.is_loopback() {
            match eps.get(&from_ip).copied() {
                Some(p) if p == remote_data => {
                    fgl_trace(&format!(
                        "PEER_LEARN_SAME ip={} port={}",
                        from_ip, remote_data
                    ));
                }
                Some(old) if old == my_port => {
                    // ancien mapping self errone -> corriger
                    eps.insert(from_ip, remote_data);
                    learned_new = true;
                    fgl_trace(&format!(
                        "PEER_LEARN_ACCEPT ip={} port={} old_port={} (fix self-map)",
                        from_ip, remote_data, old
                    ));
                }
                Some(old) => {
                    // sticky : on accepte une MAJ depuis une vraie source ET
                    eps.insert(from_ip, remote_data);
                    learned_new = true;
                    fgl_trace(&format!(
                        "PEER_LEARN_ACCEPT ip={} port={} old_port={} (src EasyTier)",
                        from_ip, remote_data, old
                    ));
                }
                None => {
                    eps.insert(from_ip, remote_data);
                    learned_new = true;
                    fgl_trace(&format!(
                        "PEER_LEARN_ACCEPT ip={} port={} (src EasyTier)",
                        from_ip, remote_data
                    ));
                }
            }
        } else {
            // loopback : ne mappe QUE les peers encore sans endpoint (et jamais my_port)
            fgl_trace(&format!(
                "PEER_LEARN src=127.0.0.1 port={} (fill empty peers only)",
                remote_data
            ));
            for peer in &peers {
                match eps.get(peer).copied() {
                    Some(p) if p != my_port => {
                        fgl_trace(&format!(
                            "PEER_LEARN_FILL_SKIP peer={} already_port={} (sticky)",
                            peer, p
                        ));
                    }
                    Some(p) if p == my_port => {
                        eps.insert(*peer, remote_data);
                        learned_new = true;
                        fgl_trace(&format!(
                            "PEER_LEARN_ACCEPT peer={} port={} old_port={} (fix self-map via loopback)",
                            peer, remote_data, p
                        ));
                    }
                    None => {
                        eps.insert(*peer, remote_data);
                        learned_new = true;
                        fgl_trace(&format!(
                            "PEER_LEARN_ACCEPT peer={} port={} (fill from loopback)",
                            peer, remote_data
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Nettoyage : supprimer toute entree qui pointe vers my_port
        let bad: Vec<IpAddr> = eps
            .iter()
            .filter_map(|(ip, p)| if *p == my_port { Some(*ip) } else { None })
            .collect();
        for ip in bad {
            eps.remove(&ip);
            fgl_trace(&format!(
                "PEER_LEARN_REJECT_SELF ip={} port={} reason=cleanup_my_port",
                ip, my_port
            ));
        }
        // Ne jamais garder 127.0.0.1 comme endpoint
        eps.remove(&IpAddr::V4(Ipv4Addr::LOCALHOST));

        fgl_trace(&format!("ENDPOINTS_NOW {:?}", *eps));
    }

    // Ne JAMAIS ajouter 127.0.0.1 a peer_ips
    if !from_ip.is_loopback() {
        let mut list = link.peer_ips.lock().await;
        list.retain(|ip| !ip.is_loopback());
        if !list.contains(&from_ip) {
            list.push(from_ip);
            fgl_trace(&format!("PEER_IPS_ADD ip={}", from_ip));
        }
    }

    learned_new
}

async fn send_announce_to(sock: &UdpSocket, dest: SocketAddr, pseudo: &str, my_port: u16) {
    if dest.port() == my_port {
        fgl_trace(&format!("TX_ANNOUNCE_SKIP dest={} reason=my_port", dest));
        return;
    }
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
                        let my_port = *link.port.lock().await;
                        let my_pseudo = link.my_pseudo.lock().await.clone();

                        // --- SELF DROP (echo) ---
                        let is_self_port = pkt.reply_port > 0 && pkt.reply_port == my_port;
                        let is_self_name = !my_pseudo.is_empty()
                            && !pkt.from.is_empty()
                            && pkt.from.eq_ignore_ascii_case(&my_pseudo);

                        if is_self_port || is_self_name {
                            fgl_trace(&format!(
                                "SELF_DROP from={} reply_port={} my_port={} my_pseudo={} reason={}",
                                pkt.from,
                                pkt.reply_port,
                                my_port,
                                my_pseudo,
                                if is_self_port { "reply_port==my_port" } else { "from==my_pseudo" }
                            ));
                            // On n'apprend PAS cet endpoint ; on ne TX_IPC pas
                            continue;
                        }

                        // Apprendre endpoint distant seulement
                        if pkt.reply_port > 0 {
                            let _ = learn_endpoint(&link, from.ip(), pkt.reply_port, my_port).await;
                        }

                        // PAS de PEER_REPLY sur player (evite tempete). Announce ignore aussi en reply.
                        // Bootstrap reste dans bootstrap_announce / set_peers.
                    }

                    // PLAYER distant uniquement -> IPC jeu
                    if pkt.kind == "player" && !pkt.payload.is_empty() {
                        if let Some(link) = app.try_state::<LinkState>() {
                            let my_port = *link.port.lock().await;
                            let my_pseudo = link.my_pseudo.lock().await.clone();
                            let is_self_port = pkt.reply_port > 0 && pkt.reply_port == my_port;
                            let is_self_name = !my_pseudo.is_empty()
                                && !pkt.from.is_empty()
                                && pkt.from.eq_ignore_ascii_case(&my_pseudo);
                            if is_self_port || is_self_name {
                                fgl_trace(&format!(
                                    "PLAYER_DROP_SELF from={} reply_port={}",
                                    pkt.from, pkt.reply_port
                                ));
                                continue;
                            }
                        }

                        if let Some(ipc) = app.try_state::<crate::fgl_ipc::IpcState>() {
                            let line = if pkt.payload.starts_with("PLAYER|") {
                                pkt.payload.clone()
                            } else {
                                format!("PLAYER|{}", pkt.payload)
                            };
                            let seq = extract_seq(&line);
                            fgl_trace(&format!(
                                "RX_EASYTIER from={} seq={} bytes={}",
                                pkt.from,
                                seq,
                                line.len()
                            ));
                            match crate::fgl_ipc::deliver_to_game(&ipc, &line).await {
                                Ok(()) => fgl_trace(&format!(
                                    "TX_IPC_REMOTE from={} seq={}",
                                    pkt.from, seq
                                )),
                                Err(e) => fgl_trace(&format!(
                                    "TX_IPC_ERROR from={} seq={} err={}",
                                    pkt.from, seq, e
                                )),
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

    // peers sans loopback
    let peers: Vec<IpAddr> = state
        .peer_ips
        .lock()
        .await
        .iter()
        .copied()
        .filter(|ip| !ip.is_loopback())
        .collect();
    {
        let mut list = state.peer_ips.lock().await;
        list.retain(|ip| !ip.is_loopback());
    }

    let eps = state.endpoints.lock().await.clone();
    let dport = discovery_port("fangame");
    fgl_trace(&format!(
        "RELAY_PLAYER_STATE seq={} peers={:?} endpoints={:?} dport={}",
        seq, peers, eps, dport
    ));
    let mut sent = 0u32;

    for ip in &peers {
        if let Some(&port) = eps.get(ip) {
            if port > 0 && port != my_port {
                for dest in dest_addrs(*ip, port, my_port) {
                    match sock.send_to(&data, dest).await {
                        Ok(n) => {
                            sent += 1;
                            fgl_trace(&format!(
                                "TX_EASYTIER target={} seq={} bytes={} (data)",
                                dest, seq, n
                            ));
                        }
                        Err(e) => fgl_trace(&format!(
                            "TX_EASYTIER_ERROR target={} seq={} err={} (data)",
                            dest, seq, e
                        )),
                    }
                }
            } else if port == my_port {
                fgl_trace(&format!(
                    "TX_EASYTIER_SKIP ip={} port={} reason=endpoint_is_my_port seq={}",
                    ip, port, seq
                ));
            }
        } else {
            fgl_trace(&format!(
                "TX_EASYTIER_ERROR seq={} ip={} err=no_endpoint",
                seq, ip
            ));
        }
        // Discovery (bootstrap) — pas vers my_port
        for dest in dest_addrs(*ip, dport, my_port) {
            match sock.send_to(&data, dest).await {
                Ok(n) => {
                    sent += 1;
                    fgl_trace(&format!(
                        "TX_EASYTIER target={} seq={} bytes={} (discovery)",
                        dest, seq, n
                    ));
                }
                Err(e) => fgl_trace(&format!(
                    "TX_EASYTIER_ERROR target={} seq={} err={} (discovery)",
                    dest, seq, e
                )),
            }
        }
    }
    fgl_trace(&format!("RELAY_PLAYER_END seq={} sent={}", seq, sent));
    sent
}

async fn bootstrap_announce(state: &LinkState) {
    {
        let mut last = state.last_bootstrap.lock().await;
        if let Some(t) = *last {
            if t.elapsed() < Duration::from_secs(2) {
                fgl_trace("BOOTSTRAP_ANNOUNCE_SKIP debounce");
                return;
            }
        }
        *last = Some(Instant::now());
    }

    let my_port = *state.port.lock().await;
    if my_port == 0 {
        fgl_trace("BOOTSTRAP_ANNOUNCE_SKIP err=my_port_0");
        return;
    }
    register_local_port(my_port);

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
    let peers: Vec<IpAddr> = state
        .peer_ips
        .lock()
        .await
        .iter()
        .copied()
        .filter(|ip| !ip.is_loopback())
        .collect();
    {
        let mut list = state.peer_ips.lock().await;
        list.retain(|ip| !ip.is_loopback());
    }
    let eps = state.endpoints.lock().await.clone();
    let dport = discovery_port("fangame");
    let local_ports = read_local_ports();
    fgl_trace(&format!(
        "BOOTSTRAP_ANNOUNCE my_port={} peers={:?} endpoints={:?} dport={} local_ports={:?}",
        my_port, peers, eps, dport, local_ports
    ));

    for ip in &peers {
        for dest in dest_addrs(*ip, dport, my_port) {
            fgl_trace(&format!("BOOTSTRAP_TX discovery dest={}", dest));
            send_announce_to(&sock, dest, &pseudo, my_port).await;
        }
        if let Some(&port) = eps.get(ip) {
            if port > 0 && port != my_port {
                for dest in dest_addrs(*ip, port, my_port) {
                    fgl_trace(&format!("BOOTSTRAP_TX data dest={}", dest));
                    send_announce_to(&sock, dest, &pseudo, my_port).await;
                }
            }
        }
    }

    // Meme PC : autres ports data locaux (pas mon port)
    for &p in &local_ports {
        if p == my_port {
            continue;
        }
        let dest = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), p);
        fgl_trace(&format!("BOOTSTRAP_TX local_data dest={}", dest));
        send_announce_to(&sock, dest, &pseudo, my_port).await;
        for ip in &peers {
            let d = SocketAddr::new(*ip, p);
            fgl_trace(&format!("BOOTSTRAP_TX local_data_et dest={}", d));
            send_announce_to(&sock, d, &pseudo, my_port).await;
        }
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
    socket
        .set_reuse_address(true)
        .map_err(|e| format!("reuse_address: {e}"))?;
    socket
        .set_nonblocking(true)
        .map_err(|e| format!("nonblocking: {e}"))?;
    socket
        .bind(&addr.into())
        .map_err(|e| format!("bind {addr}: {e}"))?;
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
        .map_err(|e| format!("fgl_link bind 0.0.0.0:0: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    println!("[FGL_LINK] data 0.0.0.0:{port}");
    fgl_trace(&format!("LINK_LISTEN data_port={}", port));
    *state.port.lock().await = port;
    register_local_port(port);
    let sock = Arc::new(sock);
    {
        let mut guard = state.sock.lock().await;
        *guard = Some(sock.clone());
    }

    spawn_reader(app.clone(), sock, "data").await;

    let dport = discovery_port("fangame");
    let daddr = SocketAddr::from(([0, 0, 0, 0], dport));
    match bind_udp_reuse(daddr).await {
        Ok(ds) => {
            println!("[FGL_LINK] discovery 0.0.0.0:{dport} (reuse)");
            fgl_trace(&format!("LINK_LISTEN discovery_port={} reuse=1", dport));
            *state.discovery_bound.lock().await = true;
            spawn_reader(app, Arc::new(ds), "discovery").await;
        }
        Err(e) => {
            println!("[FGL_LINK] discovery {dport} skip: {e}");
            fgl_trace(&format!(
                "LINK_LISTEN discovery_port={} SKIP err={}",
                dport, e
            ));
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
    list.retain(|ip| !ip.is_loopback());
    for s in ips {
        if let Some(ip) = parse_ip(&s) {
            if ip.is_loopback() {
                fgl_trace(&format!("PEER_SET_REJECT loopback {}", s));
                continue;
            }
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
    let my_port = *state.port.lock().await;
    if port == 0 || port == my_port {
        fgl_trace(&format!(
            "PEER_LEARN_REJECT_SELF remember ip={} port={} my_port={}",
            ip, port, my_port
        ));
        return Ok(());
    }
    let Some(parsed) = parse_ip(&ip) else {
        fgl_trace(&format!("PEER_REMEMBER_SKIP bad_ip={}", ip));
        return Ok(());
    };
    if parsed.is_loopback() {
        fgl_trace("PEER_REMEMBER_SKIP loopback");
        return Ok(());
    }
    state.endpoints.lock().await.insert(parsed, port);
    fgl_trace(&format!("PEER_LEARN_ACCEPT (remember) ip={} port={}", parsed, port));
    let mut list = state.peer_ips.lock().await;
    list.retain(|ip| !ip.is_loopback());
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
        if ip.is_loopback() {
            continue;
        }
        if let Some(&port) = eps.get(&ip) {
            if port > 0 && port != my_port {
                for dest in dest_addrs(ip, port, my_port) {
                    if sock.send_to(&data, dest).await.is_ok() {
                        sent += 1;
                        fgl_trace(&format!("FGL_LINK_SEND_OK dest={}", dest));
                    }
                }
            }
        }
        for dest in dest_addrs(ip, dport, my_port) {
            if sock.send_to(&data, dest).await.is_ok() {
                sent += 1;
                fgl_trace(&format!("FGL_LINK_SEND_OK dest={} (discovery)", dest));
            }
        }
    }
    Ok(sent)
}