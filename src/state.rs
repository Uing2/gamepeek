use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::audio::{AudioManager, MuteSnapshot};

#[derive(Debug, Default, Serialize, Deserialize)]
struct SavedState {
    #[serde(default)]
    snapshots: Vec<SavedSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SavedSnapshot {
    pid: u32,
    session_id: String,
    was_muted: bool,
}

fn path() -> std::path::PathBuf {
    crate::config::data_dir().join("state.toml")
}

/// 静音游戏后立即落盘，进程被强杀时下次启动还能还原。
pub fn save(snapshots: &[MuteSnapshot]) {
    if snapshots.is_empty() {
        clear();
        return;
    }

    let state = SavedState {
        snapshots: snapshots
            .iter()
            .map(|snapshot| SavedSnapshot {
                pid: snapshot.pid,
                session_id: snapshot.session_id.clone(),
                was_muted: snapshot.was_muted,
            })
            .collect(),
    };

    if let Ok(text) = toml::to_string_pretty(&state) {
        let _ = std::fs::write(path(), text);
    }
}

pub fn clear() {
    let _ = std::fs::remove_file(path());
}

/// 启动时尝试还原上次遗留的静音状态。返回是否真的还原过。
pub fn restore_pending(audio: &AudioManager) -> Result<bool> {
    let file = path();
    if !file.exists() {
        return Ok(false);
    }

    let text = std::fs::read_to_string(&file)?;
    let state: SavedState = toml::from_str(&text)?;

    if !state.snapshots.is_empty() {
        let snapshots: Vec<MuteSnapshot> = state
            .snapshots
            .iter()
            .map(|saved| MuteSnapshot {
                pid: saved.pid,
                session_id: saved.session_id.clone(),
                was_muted: saved.was_muted,
            })
            .collect();
        audio.restore(&snapshots);
        crate::logline!("已还原上次遗留的音频状态，共 {} 个会话", snapshots.len());
    }

    clear();
    Ok(true)
}