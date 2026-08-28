use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FangameDetectResult {
    pub root: String,
    pub game_exe: Option<String>,
    pub game_exe_size: u64,
    pub game_title: Option<String>,
    pub has_data_dir: bool,
    pub data_dir: Option<String>,
    pub scripts_rxdata: Option<String>,
    pub game_rgssad: Option<String>,
    pub mode: String,
    pub ok: bool,
    pub message: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FangameFingerprint {
    pub title: String,
    pub exe_size: u64,
}

fn find_in_dir_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let target = name.to_lowercase();
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_file() {
            if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                if fname.to_lowercase() == target {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn find_dir_ci(parent: &Path, name: &str) -> Option<PathBuf> {
    let target = name.to_lowercase();
    let rd = std::fs::read_dir(parent).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                if fname.to_lowercase() == target {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn read_game_ini_title(root: &Path) -> Option<String> {
    let ini = find_in_dir_ci(root, "Game.ini")?;
    let text = std::fs::read_to_string(&ini).ok()?;
    for line in text.lines() {
        let line = line.trim();
        // Title=Something
        if let Some(rest) = line.strip_prefix("Title=") {
            let t = rest.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        // title = Something
        if line.to_lowercase().starts_with("title=") {
            let t = line.splitn(2, '=').nth(1)?.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub fn detect_fangame(path: &str) -> FangameDetectResult {
    let input = PathBuf::from(path.trim());
    if path.trim().is_empty() || !input.exists() {
        return FangameDetectResult {
            root: String::new(),
            game_exe: None,
            game_exe_size: 0,
            game_title: None,
            has_data_dir: false,
            data_dir: None,
            scripts_rxdata: None,
            game_rgssad: None,
            mode: "unknown".into(),
            ok: false,
            message: "Invalid or missing path.".into(),
            cancelled: false,
        };
    }

    let (root, game_exe) = if input.is_file() {
        let name = input
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let parent = input.parent().unwrap_or(&input).to_path_buf();
        let exe = if name.ends_with(".exe") {
            Some(input.clone())
        } else {
            None
        };
        (parent, exe)
    } else {
        let exe = find_in_dir_ci(&input, "Game.exe");
        (input, exe)
    };

    let game_exe_size = game_exe.as_ref().map(|p| file_size(p)).unwrap_or(0);
    let game_title = read_game_ini_title(&root);

    let data_dir = find_dir_ci(&root, "Data");
    let has_data_dir = data_dir.as_ref().map(|d| d.is_dir()).unwrap_or(false);

    let scripts_rxdata = data_dir
        .as_ref()
        .and_then(|d| find_in_dir_ci(d, "Scripts.rxdata"))
        .map(|p| p.display().to_string());

    let game_rgssad = find_in_dir_ci(&root, "Game.rgssad").map(|p| p.display().to_string());

    let (mode, ok, message) = if scripts_rxdata.is_some() {
        ("scripts_rxdata".into(), true, "Data/Scripts.rxdata found.".into())
    } else if game_rgssad.is_some() {
        ("rgssad".into(), true, "Game.rgssad found (no Data folder).".into())
    } else if has_data_dir {
        ("unknown".into(), false, "Data folder exists but Scripts.rxdata not found.".into())
    } else {
        ("unknown".into(), false, "Neither Data/Scripts.rxdata nor Game.rgssad found.".into())
    };

    FangameDetectResult {
        root: root.display().to_string(),
        game_exe: game_exe.map(|p| p.display().to_string()),
        game_exe_size,
        game_title,
        has_data_dir,
        data_dir: data_dir.map(|p| p.display().to_string()),
        scripts_rxdata,
        game_rgssad,
        mode,
        ok,
        message,
        cancelled: false,
    }
}

#[tauri::command]
pub fn pick_fangame_and_detect() -> FangameDetectResult {
    let file = rfd::FileDialog::new()
        .add_filter("RPG Maker Game", &["exe"])
        .add_filter("All files", &["*"])
        .set_title("Select Game.exe (fangame)")
        .pick_file();

    match file {
        Some(path) => detect_fangame(&path.display().to_string()),
        None => FangameDetectResult {
            root: String::new(),
            game_exe: None,
            game_exe_size: 0,
            game_title: None,
            has_data_dir: false,
            data_dir: None,
            scripts_rxdata: None,
            game_rgssad: None,
            mode: "unknown".into(),
            ok: false,
            message: "Cancelled.".into(),
            cancelled: true,
        },
    }
}

#[tauri::command]
pub fn detect_fangame_path(path: String) -> FangameDetectResult {
    detect_fangame(&path)
}

/// Empreinte pour verif host/join (title + taille exe). Ne pas logger la taille cote UI.
#[tauri::command]
pub fn get_fangame_fingerprint(path: String) -> Result<FangameFingerprint, String> {
    let info = detect_fangame(&path);
    if !info.ok {
        return Err(info.message);
    }
    let title = info
        .game_title
        .clone()
        .unwrap_or_else(|| "UNKNOWN".into());
    if info.game_exe_size == 0 {
        return Err("Game.exe size is 0".into());
    }
    Ok(FangameFingerprint {
        title,
        exe_size: info.game_exe_size,
    })
}

#[tauri::command]
pub fn launch_fangame(path: String) -> Result<(), String> {
    let info = detect_fangame(&path);
    let exe = info
        .game_exe
        .ok_or_else(|| "Game.exe not found".to_string())?;
    let root = info.root;
    Command::new(&exe)
        .current_dir(&root)
        .spawn()
        .map_err(|e| format!("Failed to launch game: {}", e))?;
    Ok(())
}

// ----- RGSSAD v1 (RPG Maker XP) : extraire Scripts.rxdata -----

fn rgssad_decrypt_name(data: &[u8], key: &mut u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        let dec = b ^ (*key as u8);
        out.push(dec);
        *key = key.wrapping_mul(7).wrapping_add(3);
    }
    out
}

fn rgssad_decrypt_data(data: &[u8], mut key: u32) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut i = 0;
    while i < out.len() {
        // 4-byte blocks XOR with little-endian key
        let kbytes = key.to_le_bytes();
        for j in 0..4 {
            if i + j < out.len() {
                out[i + j] ^= kbytes[j];
            }
        }
        key = key.wrapping_mul(7).wrapping_add(3);
        i += 4;
    }
    out
}

/// Extrait Data/Scripts.rxdata depuis Game.rgssad vers un fichier de sortie.
#[tauri::command]
pub fn extract_scripts_from_rgssad(rgssad_path: String, out_path: String) -> Result<String, String> {
    let mut f = File::open(&rgssad_path).map_err(|e| e.to_string())?;
    let mut magic = [0u8; 7];
    f.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"RGSSAD\0" {
        return Err("Not a valid RGSSAD archive".into());
    }
    let mut ver = [0u8; 1];
    f.read_exact(&mut ver).map_err(|e| e.to_string())?;
    if ver[0] != 1 {
        return Err(format!("Unsupported RGSSAD version {}", ver[0]));
    }

    let mut key: u32 = 0xDEAD_CAFE;
    let file_len = f.metadata().map_err(|e| e.to_string())?.len();

    loop {
        let pos = f.stream_position().map_err(|e| e.to_string())?;
        if pos + 4 > file_len {
            break;
        }
        let mut len_buf = [0u8; 4];
        if f.read_exact(&mut len_buf).is_err() {
            break;
        }
        let name_len = u32::from_le_bytes(len_buf) ^ key;
        key = key.wrapping_mul(7).wrapping_add(3);
        if name_len == 0 || name_len > 4096 {
            break;
        }
        let mut name_enc = vec![0u8; name_len as usize];
        if f.read_exact(&mut name_enc).is_err() {
            break;
        }
        let name_bytes = rgssad_decrypt_name(&name_enc, &mut key);
        let name = String::from_utf8_lossy(&name_bytes).replace('\\', "/");

        let mut size_buf = [0u8; 4];
        if f.read_exact(&mut size_buf).is_err() {
            break;
        }
        let size = u32::from_le_bytes(size_buf) ^ key;
        key = key.wrapping_mul(7).wrapping_add(3);
        if size > 64 * 1024 * 1024 {
            break;
        }
        let mut data_enc = vec![0u8; size as usize];
        if f.read_exact(&mut data_enc).is_err() {
            break;
        }
        let data_key = key;
        // advance key as if decrypting for next entry
        let mut k = key;
        let mut i = 0usize;
        while i < data_enc.len() {
            k = k.wrapping_mul(7).wrapping_add(3);
            i += 4;
        }
        key = k;

        let lower = name.to_lowercase();
        if lower.ends_with("scripts.rxdata") || lower == "data/scripts.rxdata" {
            let dec = rgssad_decrypt_data(&data_enc, data_key);
            if let Some(parent) = Path::new(&out_path).parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut out = File::create(&out_path).map_err(|e| e.to_string())?;
            out.write_all(&dec).map_err(|e| e.to_string())?;
            return Ok(out_path);
        }
    }
    Err("Scripts.rxdata not found inside Game.rgssad".into())
}

/// Prepare scripts path: si mode rgssad, extrait vers <root>/_fgl_extract/Scripts.rxdata
#[tauri::command]
pub fn prepare_scripts_rxdata(game_path: String) -> Result<String, String> {
    let info = detect_fangame(&game_path);
    if let Some(s) = info.scripts_rxdata {
        return Ok(s);
    }
    let rgssad = info
        .game_rgssad
        .ok_or_else(|| "No Scripts.rxdata and no Game.rgssad".to_string())?;
    let out_dir = PathBuf::from(&info.root).join("_fgl_extract");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let out = out_dir.join("Scripts.rxdata");
    extract_scripts_from_rgssad(rgssad, out.display().to_string())
}