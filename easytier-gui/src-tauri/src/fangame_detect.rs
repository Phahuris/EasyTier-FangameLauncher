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

fn parse_title_from_ini_text(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        let lower = line.to_lowercase();
        if lower.starts_with("title=") {
            let t = line.splitn(2, '=').nth(1)?.trim();
            let t = t.replace('\u{fffd}', "");
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn from_western(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

fn read_game_ini_title(root: &Path) -> Option<String> {
    let ini = find_in_dir_ci(root, "Game.ini")?;
    let bytes = std::fs::read(&ini).ok()?;
    if bytes.is_empty() {
        return None;
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        if let Some(t) = parse_title_from_ini_text(&String::from_utf16_lossy(&u16s)) {
            return Some(t);
        }
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        if let Some(t) = parse_title_from_ini_text(&String::from_utf16_lossy(&u16s)) {
            return Some(t);
        }
    }
    let nulls = bytes.iter().filter(|&&b| b == 0).count();
    if nulls > bytes.len() / 4 {
        let u16s: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        if let Some(t) = parse_title_from_ini_text(&String::from_utf16_lossy(&u16s)) {
            return Some(t);
        }
    }
    if let Ok(text) = std::str::from_utf8(&bytes) {
        if let Some(t) = parse_title_from_ini_text(text) {
            if !t.contains('\u{fffd}') {
                return Some(t);
            }
        }
    }
    parse_title_from_ini_text(&from_western(&bytes))
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
        (input.clone(), find_in_dir_ci(&input, "Game.exe"))
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

#[tauri::command]
pub fn get_fangame_fingerprint(path: String) -> Result<FangameFingerprint, String> {
    let info = detect_fangame(&path);
    if !info.ok {
        return Err(info.message);
    }
    let title = info.game_title.clone().unwrap_or_else(|| "UNKNOWN".into());
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
    let exe = info.game_exe.ok_or_else(|| "Game.exe not found".to_string())?;
    Command::new(&exe)
        .current_dir(&info.root)
        .spawn()
        .map_err(|e| format!("Failed to launch game: {}", e))?;
    Ok(())
}

fn decrypt_integer(value: u32, key: &mut u32) -> u32 {
    let result = value ^ *key;
    *key = key.wrapping_mul(7).wrapping_add(3);
    result
}

fn decrypt_filename(encrypted: &[u8], key: &mut u32) -> String {
    let mut dec = Vec::with_capacity(encrypted.len());
    for &b in encrypted {
        dec.push(b ^ (*key as u8));
        *key = key.wrapping_mul(7).wrapping_add(3);
    }
    String::from_utf8_lossy(&dec).to_string()
}

fn decrypt_file_data(encrypted: &[u8], key: u32) -> Vec<u8> {
    let mut out = vec![0u8; encrypted.len()];
    let mut temp_key = key;
    let mut key_bytes = temp_key.to_le_bytes();
    let mut j = 0usize;
    for i in 0..encrypted.len() {
        if j == 4 {
            j = 0;
            temp_key = temp_key.wrapping_mul(7).wrapping_add(3);
            key_bytes = temp_key.to_le_bytes();
        }
        out[i] = encrypted[i] ^ key_bytes[j];
        j += 1;
    }
    out
}

#[tauri::command]
pub fn extract_scripts_from_rgssad(rgssad_path: String, out_path: String) -> Result<String, String> {
    let mut f = File::open(&rgssad_path).map_err(|e| e.to_string())?;
    let mut magic = [0u8; 6];
    f.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"RGSSAD" {
        return Err("Not a valid RGSSAD archive".into());
    }
    let mut zero = [0u8; 1];
    f.read_exact(&mut zero).map_err(|e| e.to_string())?;
    let mut ver = [0u8; 1];
    f.read_exact(&mut ver).map_err(|e| e.to_string())?;
    if ver[0] != 1 {
        return Err(format!("Unsupported RGSSAD version {}", ver[0]));
    }

    let mut key: u32 = 0xDEAD_CAFE;
    let file_len = f.metadata().map_err(|e| e.to_string())?.len();
    let mut found_names: Vec<String> = Vec::new();

    loop {
        let pos = f.stream_position().map_err(|e| e.to_string())?;
        if pos + 4 > file_len {
            break;
        }
        let mut len_buf = [0u8; 4];
        if f.read_exact(&mut len_buf).is_err() {
            break;
        }
        let name_len = decrypt_integer(u32::from_le_bytes(len_buf), &mut key);
        if name_len == 0 || name_len > 8192 {
            break;
        }
        if pos + 4 + name_len as u64 + 4 > file_len {
            break;
        }
        let mut name_enc = vec![0u8; name_len as usize];
        if f.read_exact(&mut name_enc).is_err() {
            break;
        }
        let name = decrypt_filename(&name_enc, &mut key);
        found_names.push(name.clone());

        let mut size_buf = [0u8; 4];
        if f.read_exact(&mut size_buf).is_err() {
            break;
        }
        let size = decrypt_integer(u32::from_le_bytes(size_buf), &mut key);
        if size > 128 * 1024 * 1024 {
            break;
        }
        let file_key = key;
        let data_pos = f.stream_position().map_err(|e| e.to_string())?;
        if data_pos + size as u64 > file_len {
            break;
        }

        let lower = name.replace('\\', "/").to_lowercase();
        let is_scripts = lower.ends_with("scripts.rxdata");

        if is_scripts {
            let mut data_enc = vec![0u8; size as usize];
            f.read_exact(&mut data_enc).map_err(|e| e.to_string())?;
            let dec = decrypt_file_data(&data_enc, file_key);
            if let Some(parent) = Path::new(&out_path).parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let mut out = File::create(&out_path).map_err(|e| e.to_string())?;
            out.write_all(&dec).map_err(|e| e.to_string())?;
            return Ok(out_path);
        } else {
            f.seek(SeekFrom::Current(size as i64))
                .map_err(|e| e.to_string())?;
        }
    }

    let sample: Vec<&str> = found_names.iter().take(8).map(|s| s.as_str()).collect();
    Err(format!(
        "Scripts.rxdata not found ({} entries, e.g. {:?})",
        found_names.len(),
        sample
    ))
}

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


fn find_ruby_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FGL_RUBY") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let candidates = [
        "ruby",
        "ruby.exe",
        r"C:\Ruby187\bin\ruby.exe",
        r"C:\Ruby\bin\ruby.exe",
        r"C:\Ruby18\bin\ruby.exe",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
        // PATH lookup
        if let Ok(out) = Command::new("where").arg(c).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(line) = s.lines().next() {
                    let pb = PathBuf::from(line.trim());
                    if pb.is_file() {
                        return Some(pb);
                    }
                }
            }
        }
    }
    None
}

fn fgl_scripts_rb_path() -> Result<PathBuf, String> {
    // dev: src-tauri/tools ; prod: a cote de l exe
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("tools").join("fgl_scripts.rb"));
            cands.push(dir.join("fgl_scripts.rb"));
        }
    }
    cands.push(PathBuf::from("tools/fgl_scripts.rb"));
    cands.push(PathBuf::from("easytier-gui/src-tauri/tools/fgl_scripts.rb"));
    // CARGO_MANIFEST_DIR au compile
    cands.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools").join("fgl_scripts.rb"));
    for c in cands {
        if c.is_file() {
            return Ok(c);
        }
    }
    Err("fgl_scripts.rb not found".into())
}

fn run_fgl_scripts(args: &[&str]) -> Result<String, String> {
    let ruby = find_ruby_exe().ok_or_else(|| {
        "Ruby not found. Install Ruby 1.8.x or set FGL_RUBY to ruby.exe".to_string()
    })?;
    let script = fgl_scripts_rb_path()?;
    let mut cmd = Command::new(&ruby);
    cmd.arg(&script);
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.output().map_err(|e| format!("spawn ruby: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        return Err(format!("ruby failed: {}{}", stderr, stdout));
    }
    Ok(stdout)
}

/// Liste les scripts (index, id, name) — une ligne par script
#[tauri::command]
pub fn list_rxdata_scripts(scripts_path: String) -> Result<String, String> {
    run_fgl_scripts(&["list", &scripts_path])
}

/// Injecte le script test FGL_Test (backup .fglbak une fois)
#[tauri::command]
pub fn inject_fgl_test_script(scripts_path: String) -> Result<String, String> {
    run_fgl_scripts(&["inject", &scripts_path])
}