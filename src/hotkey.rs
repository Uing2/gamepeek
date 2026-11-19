use anyhow::{anyhow, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};

pub const HOTKEY_ID: i32 = 1;

pub fn parse(spec: &str) -> Result<(HOT_KEY_MODIFIERS, u32)> {
    let mut modifiers = MOD_NOREPEAT;
    let mut virtual_key: Option<u32> = None;

    for part in spec.split('+').map(str::trim).filter(|part| !part.is_empty()) {
        match part.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers = modifiers | MOD_CONTROL,
            "ALT" => modifiers = modifiers | MOD_ALT,
            "SHIFT" => modifiers = modifiers | MOD_SHIFT,
            "WIN" | "SUPER" | "META" => modifiers = modifiers | MOD_WIN,
            other => {
                virtual_key = Some(
                    vk_from_name(other)
                        .ok_or_else(|| anyhow!("无法识别的按键: {other}（完整配置: {spec}）"))?,
                );
            }
        }
    }

    let virtual_key = virtual_key.ok_or_else(|| anyhow!("热键缺少主键: {spec}"))?;
    Ok((modifiers, virtual_key))
}

fn vk_from_name(name: &str) -> Option<u32> {
    let upper = name.to_ascii_uppercase();

    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(number) = rest.parse::<u32>() {
            if (1..=24).contains(&number) {
                return Some(0x70 + number - 1);
            }
        }
    }

    if upper.len() == 1 {
        let ch = upper.chars().next()?;
        if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
            return Some(ch as u32);
        }
    }

    let code = match upper.as_str() {
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ENTER" | "RETURN" => 0x0D,
        "ESC" | "ESCAPE" => 0x1B,
        "BACKSPACE" => 0x08,
        "INSERT" => 0x2D,
        "DELETE" => 0x2E,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" | "PGUP" => 0x21,
        "PAGEDOWN" | "PGDN" => 0x22,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        "MOUSE4" | "XBUTTON1" => 0x05,
        "MOUSE5" | "XBUTTON2" => 0x06,
        "`" => 0xC0,
        "CAPSLOCK" => 0x14,
        _ => return None,
    };
    Some(code)
}

pub fn register(hwnd: HWND, spec: &str) -> Result<(HOT_KEY_MODIFIERS, u32)> {
    let (modifiers, virtual_key) = parse(spec)?;
    unsafe {
        RegisterHotKey(hwnd, HOTKEY_ID, modifiers, virtual_key)
            .map_err(|err| anyhow!("注册热键 {spec} 失败（可能已被占用）: {err}"))?;
    }
    Ok((modifiers, virtual_key))
}

pub fn unregister(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_function_key() {
        let (_, vk) = parse("F8").unwrap();
        assert_eq!(vk, 0x77);
    }

    #[test]
    fn parses_combination() {
        let (modifiers, vk) = parse("Ctrl+Shift+F9").unwrap();
        assert_eq!(vk, 0x78);
        assert_ne!((modifiers & MOD_CONTROL).0, 0);
        assert_ne!((modifiers & MOD_SHIFT).0, 0);
        assert_ne!((modifiers & MOD_NOREPEAT).0, 0);
    }

    #[test]
    fn parses_single_letter() {
        let (modifiers, vk) = parse("Alt+D").unwrap();
        assert_eq!(vk, 'D' as u32);
        assert_ne!((modifiers & MOD_ALT).0, 0);
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(parse("Ctrl+NotAKey").is_err());
        assert!(parse("Ctrl").is_err());
    }
}