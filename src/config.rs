use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hotkey: String,
    pub game: GameConfig,
    pub target: TargetConfig,
    pub layout: LayoutConfig,
    pub pause: PauseConfig,
    pub auto_return: AutoReturnConfig,
    pub app: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GameConfig {
    /// 游戏进程名，例如 cs2.exe
    pub process: String,
    /// 进入 PEEK 时是否静音游戏
    pub mute_on_peek: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetConfig {
    /// 目标进程名，例如 douyin.exe
    pub process: String,
    /// 可选：精确匹配窗口类名
    pub window_class: String,
    /// 可选：窗口标题包含该字符串
    pub window_title: String,
    pub auto_launch: bool,
    pub launch_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    /// bottom-right / bottom-left / top-right / top-left / remember
    pub position: String,
    pub width: i32,
    pub height: i32,
    pub margin: i32,
    /// offscreen / minimize
    pub hide_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PauseConfig {
    /// auto / space / smtc / none
    pub method: String,
    pub verify_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoReturnConfig {
    /// 游戏重新获得前台时自动返回
    pub on_game_focus: bool,
    /// game：仅游戏；any：目标窗口失焦即返回
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub start_with_windows: bool,
    pub restore_audio_on_exit: bool,
    pub log_retention_days: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+C".into(),
            game: GameConfig::default(),
            target: TargetConfig::default(),
            layout: LayoutConfig::default(),
            pause: PauseConfig::default(),
            auto_return: AutoReturnConfig::default(),
            app: AppConfig::default(),
        }
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            process: "cs2.exe".into(),
            mute_on_peek: true,
        }
    }
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self {
            process: "douyin.exe".into(),
            window_class: String::new(),
            window_title: "抖音".into(),
            auto_launch: false,
            launch_command: String::new(),
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            position: "bottom-right".into(),
            width: 480,
            height: 270,
            margin: 24,
            hide_mode: "offscreen".into(),
        }
    }
}

impl Default for PauseConfig {
    fn default() -> Self {
        Self {
            method: "auto".into(),
            verify_timeout_ms: 100,
        }
    }
}

impl Default for AutoReturnConfig {
    fn default() -> Self {
        Self {
            on_game_focus: true,
            scope: "game".into(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            restore_audio_on_exit: true,
            log_retention_days: 7,
        }
    }
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("GamePeek")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.toml")
}

pub fn log_path() -> PathBuf {
    data_dir().join("logs").join("gamepeek.log")
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(&path, toml::to_string_pretty(config)?)
        .with_context(|| format!("写入配置失败: {}", path.display()))?;
    Ok(())
}

pub fn load_or_create() -> Result<Config> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    if path.exists() {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("读取配置失败: {}", path.display()))?;
        match toml::from_str::<Config>(&text) {
            Ok(config) => Ok(config),
            Err(err) => {
                let backup = path.with_extension("toml.bak");
                let _ = fs::copy(&path, &backup);
                crate::logline!(
                    "配置解析失败({err})，已备份到 {}，本次使用默认配置",
                    backup.display()
                );
                Ok(Config::default())
            }
        }
    } else {
        let config = Config::default();
        fs::write(&path, toml::to_string_pretty(&config)?)
            .with_context(|| format!("写入默认配置失败: {}", path.display()))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips() {
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed.hotkey, "F8");
        assert_eq!(parsed.game.process, "cs2.exe");
        assert_eq!(parsed.layout.width, 480);
    }

    #[test]
    fn partial_config_uses_defaults() {
        let parsed: Config = toml::from_str("hotkey = \"F7\"\n").unwrap();
        assert_eq!(parsed.hotkey, "F7");
        assert_eq!(parsed.game.process, "cs2.exe");
        assert!(parsed.auto_return.on_game_focus);
    }
}