//! Etape 3 : IPC local jeu <-> CE launcher uniquement.
//! Bind 127.0.0.1:0 (port libre par instance). Aucun reseau distant ici.

use std::sync::Arc;
use tauri::State;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub struct IpcState {
    pub socket: Mutex<Option<Arc<UdpSocket>>>,
    pub port: Mutex<u16>,
}

impl Default for IpcState {
    fn default() -> Self {
        Self {
            socket: Mutex::new(None),
            port: Mutex::new(0),
        }
    }
}

#[tauri::command]
pub async fn fgl_ipc_start(state: State<'_, IpcState>) -> Result<u16, String> {
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
        .map_err(|e| format!("fgl_ipc bind 127.0.0.1:0: {e}"))?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    println!("[FGL_IPC] 127.0.0.1:{port}");
    *state.port.lock().await = port;
    *guard = Some(Arc::new(sock));
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