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
    println!("[CHAT] ===== CHAT START =====");
    println!("[CHAT] Port UDP: {}", CHAT_PORT);

    let mut guard = state.socket.lock().await;

    if guard.is_some() {
        println!("[CHAT] Socket déjà démarré");
        return Ok(());
    }

    println!("[CHAT] Tentative bind 0.0.0.0:{}", CHAT_PORT);

    let sock = UdpSocket::bind(format!("0.0.0.0:{}", CHAT_PORT))
        .await
        .map_err(|e| {
            eprintln!("[CHAT ERROR] Impossible de bind le port {}: {}", CHAT_PORT, e);
            format!("chat bind: {}", e)
        })?;

    println!("[CHAT] Socket UDP créé avec succès");

    match sock.local_addr() {
        Ok(addr) => println!("[CHAT] Adresse locale: {}", addr),
        Err(e) => eprintln!("[CHAT ERROR] Impossible de lire local_addr: {}", e),
    }

    let sock = Arc::new(sock);

    *guard = Some(sock.clone());

    drop(guard);

    println!("[CHAT] Socket enregistré dans ChatState");
    println!("[CHAT] Démarrage du reader UDP");

    let app2 = app.clone();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];

        println!("[CHAT RX] Reader UDP démarré");

        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    println!(
                        "[CHAT RX] Paquet reçu: {} octets depuis {}",
                        n, from
                    );

                    if n == 0 {
                        println!("[CHAT RX] Paquet vide ignoré");
                        continue;
                    }

                    match std::str::from_utf8(&buf[..n]) {
                        Ok(s) => {
                            println!("[CHAT RX] Payload UTF-8: {}", s);

                            match serde_json::from_str::<ChatPacket>(s) {
                                Ok(pkt) => {
                                    println!(
                                        "[CHAT RX] JSON OK | type={} | version={} | pseudo={} | plugin={} | action={} | texte={}",
                                        pkt.kind,
                                        pkt.v,
                                        pkt.pseudo,
                                        pkt.plugin,
                                        pkt.action,
                                        pkt.text
                                    );

                                    match app2.emit("chat_message", pkt) {
                                        Ok(_) => {
                                            println!("[CHAT RX] Event chat_message émis vers le GUI");
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "[CHAT ERROR] Échec émission chat_message: {}",
                                                e
                                            );
                                        }
                                    }
                                }

                                Err(e) => {
                                    eprintln!(
                                        "[CHAT ERROR] JSON invalide depuis {}: {}",
                                        from, e
                                    );
                                }
                            }
                        }

                        Err(e) => {
                            eprintln!(
                                "[CHAT ERROR] Payload non UTF-8 depuis {}: {}",
                                from, e
                            );
                        }
                    }
                }

                Err(e) => {
                    eprintln!("[CHAT ERROR] recv_from a échoué: {}", e);
                    eprintln!("[CHAT RX] Reader UDP arrêté");
                    break;
                }
            }
        }
    });

    println!("[CHAT] ===== CHAT START OK =====");

    Ok(())
}

#[tauri::command]
pub async fn chat_stop(
    state: State<'_, ChatState>,
) -> Result<(), String> {
    println!("[CHAT] ===== CHAT STOP =====");

    let mut guard = state.socket.lock().await;

    if guard.is_some() {
        println!("[CHAT] Fermeture du socket");
    } else {
        println!("[CHAT] Aucun socket actif");
    }

    *guard = None;

    println!("[CHAT] Socket retiré de ChatState");
    println!("[CHAT] ===== CHAT STOP OK =====");

    Ok(())
}

fn parse_peer_ip(ip: &str) -> Option<IpAddr> {
    let original = ip;

    let clean = ip
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');

    println!(
        "[CHAT] Parsing peer IP | brut='{}' | nettoyé='{}'",
        original, clean
    );

    match clean.parse::<IpAddr>() {
        Ok(ip) => {
            println!("[CHAT] Peer IP valide: {}", ip);
            Some(ip)
        }

        Err(e) => {
            eprintln!(
                "[CHAT ERROR] Peer IP invalide | brut='{}' | nettoyé='{}' | erreur={}",
                original, clean, e
            );
            None
        }
    }
}

async fn send_to_peers(
    sock: &UdpSocket,
    data: &[u8],
    peers: Vec<String>,
) {
    println!(
        "[CHAT TX] ===== ENVOI ===== | {} octets | {} peer(s)",
        data.len(),
        peers.len()
    );

    if peers.is_empty() {
        println!("[CHAT TX] Aucun peer fourni");
        return;
    }

    for ip in peers {
        let ip = ip.trim();

        println!("[CHAT TX] Peer demandé: '{}'", ip);

        if ip.is_empty() {
            println!("[CHAT TX] Peer vide ignoré");
            continue;
        }

        let Some(ip) = parse_peer_ip(ip) else {
            eprintln!("[CHAT TX] Peer ignoré car IP invalide");
            continue;
        };

        let addr = SocketAddr::new(ip, CHAT_PORT);

        println!("[CHAT TX] Destination finale: {}", addr);

        match sock.send_to(data, addr).await {
            Ok(n) => {
                println!(
                    "[CHAT TX] OK | {} octets envoyés -> {}",
                    n, addr
                );
            }

            Err(e) => {
                eprintln!(
                    "[CHAT ERROR] Échec UDP vers {} | {}",
                    addr, e
                );
            }
        }
    }

    println!("[CHAT TX] ===== FIN ENVOI =====");
}

#[tauri::command]
pub async fn chat_send(
    state: State<'_, ChatState>,
    pseudo: String,
    text: String,
    peers: Vec<String>,
) -> Result<(), String> {
    println!("[CHAT] ===== chat_send =====");
    println!("[CHAT] pseudo='{}'", pseudo);
    println!("[CHAT] texte brut='{}'", text);
    println!("[CHAT] peers={:?}", peers);

    let guard = state.socket.lock().await;

    let Some(sock) = guard.as_ref() else {
        eprintln!("[CHAT ERROR] chat_send appelé alors que le chat n'est pas démarré");
        return Err("chat non demarre".into());
    };

    let text = text.trim();

    if text.is_empty() {
        println!("[CHAT] Message vide ignoré");
        return Ok(());
    }

    println!("[CHAT] Création paquet chat");

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

    println!(
        "[CHAT] Packet: type={} version={} pseudo={} texte={} ts={}",
        pkt.kind,
        pkt.v,
        pkt.pseudo,
        pkt.text,
        pkt.ts
    );

    let data = serde_json::to_vec(&pkt).map_err(|e| {
        eprintln!("[CHAT ERROR] Sérialisation JSON impossible: {}", e);
        e.to_string()
    })?;

    println!(
        "[CHAT] Sérialisation OK | {} octets",
        data.len()
    );

    send_to_peers(sock, &data, peers).await;

    println!("[CHAT] ===== chat_send OK =====");

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
    println!("[CHAT] ===== chat_send_cmd =====");
    println!("[CHAT] pseudo='{}'", pseudo);
    println!("[CHAT] plugin='{}'", plugin);
    println!("[CHAT] action='{}'", action);
    println!("[CHAT] peers={:?}", peers);

    let guard = state.socket.lock().await;

    let Some(sock) = guard.as_ref() else {
        eprintln!("[CHAT ERROR] chat_send_cmd appelé alors que le chat n'est pas démarré");
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

    println!(
        "[CHAT] CMD packet: pseudo={} plugin={} action={} ts={}",
        pkt.pseudo,
        pkt.plugin,
        pkt.action,
        pkt.ts
    );

    let data = serde_json::to_vec(&pkt).map_err(|e| {
        eprintln!("[CHAT ERROR] Sérialisation CMD impossible: {}", e);
        e.to_string()
    })?;

    println!(
        "[CHAT] Sérialisation CMD OK | {} octets",
        data.len()
    );

    send_to_peers(sock, &data, peers).await;

    println!("[CHAT] ===== chat_send_cmd OK =====");

    Ok(())
}
