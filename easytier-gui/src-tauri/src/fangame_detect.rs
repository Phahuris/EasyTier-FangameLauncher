use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct FangameDetectResult {
    pub root: String,
    pub game_exe: Option<String>,
    pub has_data_dir: bool,
    pub data_dir: Option<String>,
    pub scripts_rxdata: Option<String>,
    pub game_rgssad: Option<String>,
    pub mode: String,
    pub ok: bool,
    pub message: String,
    pub cancelled: bool,
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

pub fn detect_fangame(path: &str) -> FangameDetectResult {
    let input = PathBuf::from(path.trim());
    if path.trim().is_empty() || !input.exists() {
        return FangameDetectResult {
            root: String::new(),
            game_exe: None,
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
            Some(input.display().to_string())
        } else {
            None
        };
        (parent, exe)
    } else {
        let exe = find_in_dir_ci(&input, "Game.exe").map(|p| p.display().to_string());
        (input, exe)
    };

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
        game_exe,
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

/// Ouvre le dialogue Windows pour choisir Game.exe, puis detecte.
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