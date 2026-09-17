//! IPC local jeu <-> launcher. Pas de fichiers peers.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub struct IpcState {
    pub socket: Mutex<Option<Arc<UdpSocket>>>,
    pub port: Mutex<u16>,
    pub game_addr: Arc<Mutex<Option<SocketAddr>>>,
    pub inbox: Mutex<VecDeque<String>>,
    pub pending: Mutex<VecDeque<String>>,
}

impl Default for IpcState {
    fn default() -> Self {
        Self {
            socket: Mutex::new(None),
            port: Mutex::new(0),
            game_addr: Arc::new(Mutex::new(None)),
            inbox: Mutex::new(VecDeque::new()),
            pending: Mutex::new(VecDeque::new()),
        }
    }
}


pub async fn deliver_to_game(state: &IpcState, payload: &str) -> Result<(), String> {
    let sock = {
        let g = state.socket.lock().await;
        g.as_ref()
            .cloned()
            .ok_or_else(|| "ipc not started".to_string())?
    };
    let addr_opt = *state.game_addr.lock().await;
    if let Some(addr) = addr_opt {
        state.inbox.lock().await.push_back(payload.to_string());
        sock.send_to(payload.as_bytes(), addr)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        let mut q = state.pending.lock().await;
        if q.len() < 256 {
            q.push_back(payload.to_string());
        }
        Ok(())
    }
}

async fn flush_pending(state: &IpcState, sock: &UdpSocket, addr: std::net::SocketAddr) {
    let mut q = state.pending.lock().await;
    while let Some(msg) = q.pop_front() {
        let _ = sock.send_to(msg.as_bytes(), addr).await;
    }
}
#[tauri::command]
pub async fn fgl_ipc_start(app: AppHandle, state: State<'_, IpcState>) -> Result<u16, String> {
    let mut guard = state.socket.lock().await;
    if let Some(sock) = guard.as_ref() {
        let p = sock
            .local_addr()
            .map(|a| a.port())
            .unwrap_or(*state.port.lock().await);
        return Ok(p);
    }
    let sock = UdpSocket::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("fgl_ipc bind: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    println!("[FGL_IPC] 127.0.0.1:{port}");
    *state.port.lock().await = port;
    let sock = Arc::new(sock);
    *guard = Some(sock.clone());
    drop(guard);

    let game_addr = state.game_addr.clone();
    let app2 = app.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    if n == 0 {
                        continue;
                    }
                    let first = {
                        let mut ga = game_addr.lock().await;
                        let was = ga.is_none();
                        *ga = Some(from);
                        was
                    };
                    if first {
                        if let Some(ipc) = app2.try_state::<IpcState>() {
                            flush_pending(&ipc, &sock, from).await;
                        }
                    }
                    let Ok(txt) = std::str::from_utf8(&buf[..n]) else {
                        continue;
                    };
                    let _ = app2.emit(
                        "fgl_from_game",
                        serde_json::json!({
                            "raw": txt,
                            "ip": from.ip().to_string(),
                            "port": from.port(),
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
pub async fn fgl_ipc_get_port(state: State<'_, IpcState>) -> Result<u16, String> {
    let p = *state.port.lock().await;
    if p == 0 {
        Err("ipc not started".into())
    } else {
        Ok(p)
    }
}

#[tauri::command]
pub async fn fgl_ipc_note_game_addr(state: State<'_, IpcState>, port: u16) -> Result<(), String> {
    if port == 0 {
        return Ok(());
    }
    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    *state.game_addr.lock().await = Some(addr);
    Ok(())
}

#[tauri::command]
pub async fn fgl_ipc_deliver(
    state: State<'_, IpcState>,
    payload: String,
    game_port: Option<u16>,
) -> Result<(), String> {
    if let Some(p) = game_port {
        if p > 0 {
            let addr: SocketAddr = format!("127.0.0.1:{p}")
                .parse()
                .map_err(|e: std::net::AddrParseError| e.to_string())?;
            *state.game_addr.lock().await = Some(addr);
        }
    }
    deliver_to_game(&state, &payload).await
}

#[tauri::command]
pub async fn fgl_ipc_push_inbox(state: State<'_, IpcState>, line: String) -> Result<(), String> {
    state.inbox.lock().await.push_back(line);
    Ok(())
}

#[tauri::command]
pub async fn fgl_ipc_poll_inbox(state: State<'_, IpcState>) -> Result<Vec<String>, String> {
    let mut q = state.inbox.lock().await;
    let mut out = Vec::new();
    while let Some(x) = q.pop_front() {
        out.push(x);
        if out.len() >= 64 {
            break;
        }
    }
    Ok(out)
}