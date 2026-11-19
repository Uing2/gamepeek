use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_KEYDOWN, WM_KEYUP};

/// 只在已确认目标窗口在前台时调用，否则空格会打到游戏上。
pub fn send_space() -> bool {
    unsafe {
        let mut inputs = [INPUT::default(); 2];
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous = INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_SPACE,
                ..Default::default()
            },
        };
        inputs[1] = inputs[0];
        inputs[1].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) == 2
    }
}

/// 通过 Windows 媒体会话按音源暂停，不抢焦点。
pub fn try_pause_smtc(keyword: &str) -> bool {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

    let keyword = keyword.trim_end_matches(".exe").to_lowercase();
    if keyword.is_empty() {
        return false;
    }

    let attempt = || -> anyhow::Result<bool> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
        let sessions = manager.GetSessions()?;

        for session in sessions {
            let aumid = session
                .SourceAppUserModelId()?
                .to_string()
                .to_lowercase();
            if !aumid.contains(&keyword) {
                continue;
            }

            let info = session.GetPlaybackInfo()?;
            if info.PlaybackStatus()? == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                let _ = session.TryPauseAsync()?.get()?;
            }
            return Ok(true);
        }
        Ok(false)
    };

    attempt().unwrap_or(false)
}

/// SMTC 不可用时，直接向目标窗口投递按键消息。
pub fn post_space(hwnd: HWND) -> bool {
    unsafe {
        let down = PostMessageW(
            hwnd,
            WM_KEYDOWN,
            WPARAM(VK_SPACE.0 as usize),
            LPARAM(0),
        )
        .is_ok();
        let up = PostMessageW(
            hwnd,
            WM_KEYUP,
            WPARAM(VK_SPACE.0 as usize),
            LPARAM(0),
        )
        .is_ok();
        down && up
    }
}