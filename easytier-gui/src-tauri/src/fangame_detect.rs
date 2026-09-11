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


/// Contenu de tools/fgl_scripts.rb compile dans le binaire (plus de "file not found")
const FGL_SCRIPTS_RB: &str = include_str!("../tools/fgl_scripts.rb");

fn find_ruby_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("FGL_RUBY") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let candidates = [
        r"C:\Ruby187\bin\ruby.exe",
        r"C:\Ruby\bin\ruby.exe",
        r"C:\Ruby18\bin\ruby.exe",
        r"C:\Ruby26\bin\ruby.exe",
        r"C:\Ruby27\bin\ruby.exe",
        r"C:\Ruby30\bin\ruby.exe",
        r"C:\Ruby31\bin\ruby.exe",
        r"C:\Ruby32\bin\ruby.exe",
        r"C:\Ruby33\bin\ruby.exe",
    ];
    for c in candidates {
        let pb = PathBuf::from(c);
        if pb.is_file() {
            return Some(pb);
        }
    }
    // PATH
    if let Ok(out) = Command::new("where").arg("ruby.exe").output() {
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
    if let Ok(out) = Command::new("where").arg("ruby").output() {
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
    None
}

fn materialize_fgl_scripts_rb() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("fangamelauncher");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("fgl_scripts.rb");
    // Toujours reecrire (maj script)
    std::fs::write(&path, FGL_SCRIPTS_RB).map_err(|e| e.to_string())?;
    Ok(path)
}

fn run_fgl_scripts(args: &[&str]) -> Result<String, String> {
    let ruby = find_ruby_exe().ok_or_else(|| {
        "Ruby not found. Set FGL_RUBY to full path of ruby.exe (1.8.x preferred)".to_string()
    })?;
    let script = materialize_fgl_scripts_rb()?;
    let mut cmd = Command::new(&ruby);
    cmd.arg(&script);
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.output().map_err(|e| format!("spawn ruby: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !out.status.success() {
        return Err(format!("ruby: {} {}", stderr, stdout));
    }
    Ok(stdout)
}

#[tauri::command]
pub fn list_rxdata_scripts(scripts_path: String) -> Result<String, String> {
    run_fgl_scripts(&["list", &scripts_path])
}

#[tauri::command]
pub fn check_fgl_plugin(scripts_path: String) -> Result<String, String> {
    run_fgl_scripts(&["check", &scripts_path])
}

#[tauri::command]
pub fn inject_fgl_test_script(scripts_path: String) -> Result<String, String> {
    // check d abord
    let st = run_fgl_scripts(&["check", &scripts_path])?;
    if st.contains("PRESENT") {
        return Ok("SKIP already present".into());
    }
    run_fgl_scripts(&["inject", &scripts_path])
}

/// Pipeline avant partie: extract si besoin -> check -> inject si absent
#[tauri::command]
pub fn prepare_and_patch_fangame(game_path: String) -> Result<String, String> {
    let scripts = prepare_scripts_rxdata(game_path)?;
    let check = run_fgl_scripts(&["check", &scripts])?;
    if check.contains("PRESENT") {
        return Ok(format!("READY scripts={} plugin=already", scripts));
    }
    let inj = run_fgl_scripts(&["inject", &scripts])?;
    Ok(format!("READY scripts={} plugin={}", scripts, inj))
}

#[tauri::command]
pub fn write_game_command(game_path: String, command: String) -> Result<(), String> {
    let root = std::path::PathBuf::from(game_path.trim());
    if !root.exists() {
        return Err("game path not found".into());
    }
    let peers = root.join("FGL_peers");
    std::fs::create_dir_all(&peers).map_err(|e| e.to_string())?;
    let mut best_id: Option<String> = None;
    let mut best_mtime = std::time::SystemTime::UNIX_EPOCH;
    if let Ok(rd) = std::fs::read_dir(&peers) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("txt") {
                continue;
            }
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let lower = name.to_lowercase();
            if lower.starts_with("chal")
                || lower.starts_with("trd_")
                || lower.starts_with("bat_")
                || lower.starts_with("party")
                || lower.starts_with("cmd_")
                || lower.starts_with("partyready")
            {
                continue;
            }
            if let Ok(meta) = p.metadata() {
                if let Ok(m) = meta.modified() {
                    if m > best_mtime {
                        best_mtime = m;
                        best_id = Some(name);
                    }
                }
            }
        }
    }
    let id = best_id
        .ok_or_else(|| "no local player id in FGL_peers (lance le jeu d abord)".to_string())?;
    let cmd_path = peers.join(format!("cmd_{}.txt", id));
    std::fs::write(&cmd_path, command.trim().as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cleanup_fgl_temp(game_path: String) -> Result<u32, String> {
    let root = std::path::PathBuf::from(game_path.trim());
    let peers = root.join("FGL_peers");
    if !peers.exists() {
        return Ok(0);
    }
    let mut n = 0u32;
    if let Ok(rd) = std::fs::read_dir(&peers) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            let drop = name.starts_with("chal")
                || name.starts_with("chalresp")
                || name.starts_with("trd_")
                || name.starts_with("bat_")
                || name.starts_with("party_")
                || name.starts_with("partyready_")
                || name.starts_with("cmd_");
            if drop {
                if std::fs::remove_file(&p).is_ok() {
                    n += 1;
                }
            }
        }
    }
    Ok(n)
}

// ===== TOUT EMBARQUE DANS LE .EXE =====
const FGL_BATTLE_FR_RB: &str = include_str!("../embedded_plugins/FGL_Battle_FR.rb");
const FGL_BATTLE_EN_RB: &str = include_str!("../embedded_plugins/FGL_Battle_EN.rb");
const FGL_TRADE_FR_RB: &str = include_str!("../embedded_plugins/FGL_Trade_FR.rb");
const FGL_TRADE_EN_RB: &str = include_str!("../embedded_plugins/FGL_Trade_EN.rb");
const FGL_NET_FR_RB: &str = include_str!("../embedded_plugins/FGL_Net_FR.rb");
const FGL_NET_EN_RB: &str = include_str!("../embedded_plugins/FGL_Net_EN.rb");
const FGL_BATTLE_MP3: &[u8] = include_bytes!("../embedded_plugins/FGL_Battle.mp3");

fn embedded_plugin_src(base: &str, lang: &str) -> Result<&'static str, String> {
    match (base, lang) {
        ("FGL_Battle", "FR") => Ok(FGL_BATTLE_FR_RB),
        ("FGL_Battle", "EN") => Ok(FGL_BATTLE_EN_RB),
        ("FGL_Trade", "FR") => Ok(FGL_TRADE_FR_RB),
        ("FGL_Trade", "EN") => Ok(FGL_TRADE_EN_RB),
        ("FGL_Net", "FR") => Ok(FGL_NET_FR_RB),
        ("FGL_Net", "EN") => Ok(FGL_NET_EN_RB),
        _ => Err(format!("plugin inconnu: {}_{}", base, lang)),
    }
}
// ===== FGL REGISTRY EMBEDDED (single copy) =====
const FGL_REGISTRY_JSON: &str = include_str!("fgl_registry.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FangameAllowEntry {
    pub id: String,
    pub display_name: String,
    pub title_exact: String,
    pub exe_size_min: u64,
    #[serde(default)]
    pub exe_size_max: u64,
    #[serde(default)]
    pub required_files: Vec<String>,
    #[serde(default)]
    pub required_dirs: Vec<String>,
    #[serde(default)]
    pub required_any_files: Vec<Vec<String>>,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default)]
    pub plugins_source: String,
    #[serde(default)]
    pub audio_source: String,
    #[serde(default)]
    pub plugin_dest: String,
    #[serde(default)]
    pub audio_dest: String,
    #[serde(default)]
    pub audio_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FangameRegistry {
    pub version: u32,
    pub allowed: Vec<FangameAllowEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FangameAllowResult {
    pub allowed: bool,
    pub fangame_id: Option<String>,
    pub display_name: Option<String>,
    pub reason: String,
    pub title: Option<String>,
    pub exe_size: u64,
    pub plugins_ok: bool,
    pub plugins_missing: Vec<String>,
}

fn load_registry() -> Result<FangameRegistry, String> {
    serde_json::from_str(FGL_REGISTRY_JSON).map_err(|e| format!("registry embed invalide: {}", e))
}

fn join_rel(root: &Path, rel: &str) -> PathBuf {
    root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))
}

fn path_is_file(root: &Path, rel: &str) -> bool {
    join_rel(root, rel).is_file()
}

fn path_is_dir(root: &Path, rel: &str) -> bool {
    join_rel(root, rel).is_dir()
}

fn launcher_fangames_root() -> PathBuf {
    for c in [
        PathBuf::from("fangames"),
        PathBuf::from("../fangames"),
        PathBuf::from("../../fangames"),
        PathBuf::from("../../../fangames"),
    ] {
        if c.is_dir() {
            return c;
        }
    }
    if let Ok(p) = std::env::var("FGL_FANGAMES") {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            return pb;
        }
    }
    PathBuf::from("fangames")
}

fn check_plugins_on_disk(
    root: &Path,
    entry: &FangameAllowEntry,
    lang: &str,
) -> (bool, Vec<String>) {
    let dest = if entry.plugin_dest.is_empty() {
        root.join("Data").join("Scripts").join("052_AddOns")
    } else {
        join_rel(root, &entry.plugin_dest)
    };
    let plugins = if entry.plugins.is_empty() {
        vec![
            "FGL_Battle".into(),
            "FGL_Trade".into(),
            "FGL_Net".into(),
        ]
    } else {
        entry.plugins.clone()
    };
    let mut missing = Vec::new();
    for base in plugins {
        let p = dest.join(format!("{}_{}.rb", base, lang));
        if !p.is_file() {
            missing.push(format!("{}_{}.rb", base, lang));
        }
    }
    (missing.is_empty(), missing)
}

fn validate_entry(
    root: &Path,
    entry: &FangameAllowEntry,
    title_str: &str,
    exe_size: u64,
) -> Result<(), String> {
    if title_str.trim() != entry.title_exact.trim() {
        return Err(format!(
            "Titre incorrect (obtenu {:?}, requis {:?})",
            title_str.trim(),
            entry.title_exact.trim()
        ));
    }
    if exe_size < entry.exe_size_min {
        return Err(format!(
            "Game.exe trop petit ({} < min {})",
            exe_size, entry.exe_size_min
        ));
    }
    if entry.exe_size_max > 0 && exe_size > entry.exe_size_max {
        return Err(format!(
            "Game.exe trop grand ({} > max {})",
            exe_size, entry.exe_size_max
        ));
    }
    for d in &entry.required_dirs {
        if !path_is_dir(root, d) {
            return Err(format!("Dossier manquant: {}", d));
        }
    }
    for f in &entry.required_files {
        if !path_is_file(root, f) {
            return Err(format!("Fichier manquant: {}", f));
        }
    }
    for group in &entry.required_any_files {
        if group.is_empty() {
            continue;
        }
        let ok = group.iter().any(|f| path_is_file(root, f));
        if !ok {
            return Err(format!("Fichier manquant (un de): {:?}", group));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn is_fangame_allowed(path: String) -> Result<FangameAllowResult, String> {
    is_fangame_allowed_lang(path, "FR".into())
}

#[tauri::command]
pub fn is_fangame_allowed_lang(path: String, lang: String) -> Result<FangameAllowResult, String> {
    let info = detect_fangame(&path);
    let title = info.game_title.clone();
    let exe_size = info.game_exe_size;
    let lang_u = if lang.to_lowercase().starts_with("en") {
        "EN"
    } else {
        "FR"
    };

    if !info.ok {
        return Ok(FangameAllowResult {
            allowed: false,
            fangame_id: None,
            display_name: None,
            reason: format!("Fangame invalide: {}", info.message),
            title,
            exe_size,
            plugins_ok: false,
            plugins_missing: vec![],
        });
    }

    let reg = load_registry()?;
    let title_str = title.clone().unwrap_or_default();
    let root = PathBuf::from(&info.root);

    let mut matched: Option<&FangameAllowEntry> = None;
    for entry in &reg.allowed {
        if title_str.trim() == entry.title_exact.trim() {
            matched = Some(entry);
            break;
        }
    }

    let Some(entry) = matched else {
        return Ok(FangameAllowResult {
            allowed: false,
            fangame_id: None,
            display_name: None,
            reason: format!(
                "Fangame non autorise (titre {:?}). Seul infinitefusion est supporte.",
                title_str
            ),
            title,
            exe_size,
            plugins_ok: false,
            plugins_missing: vec![],
        });
    };

    if let Err(reason) = validate_entry(&root, entry, &title_str, exe_size) {
        return Ok(FangameAllowResult {
            allowed: false,
            fangame_id: Some(entry.id.clone()),
            display_name: Some(entry.display_name.clone()),
            reason,
            title,
            exe_size,
            plugins_ok: false,
            plugins_missing: vec![],
        });
    }

    let (plugins_ok, plugins_missing) = check_plugins_on_disk(&root, entry, lang_u);
    Ok(FangameAllowResult {
        allowed: true,
        fangame_id: Some(entry.id.clone()),
        display_name: Some(entry.display_name.clone()),
        reason: if plugins_ok {
            "OK".into()
        } else {
            format!("OK fangame, plugins manquants: {:?}", plugins_missing)
        },
        title,
        exe_size,
        plugins_ok,
        plugins_missing,
    })
}

#[tauri::command]
pub fn verify_fgl_plugins(game_path: String, lang: String) -> Result<FangameAllowResult, String> {
    is_fangame_allowed_lang(game_path, lang)
}

#[tauri::command]
pub fn install_fgl_plugins(game_path: String, lang: String) -> Result<String, String> {
    let allow = is_fangame_allowed_lang(game_path.clone(), lang.clone())?;
    if !allow.allowed {
        return Err(allow.reason);
    }
    let entry_id = allow
        .fangame_id
        .unwrap_or_else(|| "infinite_fusion".into());
    let reg = load_registry()?;
    let entry = reg
        .allowed
        .iter()
        .find(|e| e.id == entry_id)
        .cloned()
        .ok_or_else(|| "entree registry introuvable".to_string())?;

    let info = detect_fangame(&game_path);
    let root = PathBuf::from(&info.root);
    let lang_u = if lang.to_lowercase().starts_with("en") {
        "EN"
    } else {
        "FR"
    };
    let other = if lang_u == "FR" { "EN" } else { "FR" };

    let plugin_dest = if entry.plugin_dest.is_empty() {
        root.join("Data").join("Scripts").join("052_AddOns")
    } else {
        join_rel(&root, &entry.plugin_dest)
    };
    std::fs::create_dir_all(&plugin_dest).map_err(|e| e.to_string())?;

    let plugins = if entry.plugins.is_empty() {
        vec![
            "FGL_Battle".into(),
            "FGL_Trade".into(),
            "FGL_Net".into(),
        ]
    } else {
        entry.plugins.clone()
    };

    let mut installed = Vec::new();
    for base in &plugins {
        let _ = std::fs::remove_file(plugin_dest.join(format!("{}_{}.rb", base, other)));
        let src = embedded_plugin_src(base, lang_u)?;
        let dst = plugin_dest.join(format!("{}_{}.rb", base, lang_u));
        std::fs::write(&dst, src).map_err(|e| format!("write {}: {}", base, e))?;
        installed.push(format!("{}_{}.rb", base, lang_u));
    }

    let audio_dest = if entry.audio_dest.is_empty() {
        root.join("Audio").join("BGM")
    } else {
        join_rel(&root, &entry.audio_dest)
    };
    std::fs::create_dir_all(&audio_dest).map_err(|e| e.to_string())?;
    std::fs::write(audio_dest.join("FGL_Battle.mp3"), FGL_BATTLE_MP3)
        .map_err(|e| format!("write mp3: {}", e))?;
    installed.push("FGL_Battle.mp3".into());

    let (ok, missing) = check_plugins_on_disk(&root, &entry, lang_u);
    if !ok {
        return Err(format!("install incomplete: {:?}", missing));
    }
    Ok(format!(
        "Install OK ({}) -> {} | {}",
        lang_u,
        plugin_dest.display(),
        installed.join(", ")
    ))
}
