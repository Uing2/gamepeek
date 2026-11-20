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
/// 返回 None 表示没有找到该应用的媒体会话；Some(true) 表示本次执行了暂停；
/// Some(false) 表示会话存在但当前不在播放。
pub fn try_pause_smtc(keyword: &str) -> Option<bool> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus;

    let keyword = keyword.trim_end_matches(".exe").to_lowercase();
    if keyword.is_empty() {
        return None;
    }

    let attempt = || -> anyhow::Result<Option<bool>> {
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
            let status = info.PlaybackStatus()?;
            if status != GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
                return Ok(Some(false));
            }

            session.TryPauseAsync()?.get()?;
            let after = session.GetPlaybackInfo()?.PlaybackStatus()?;
            crate::logline!(
                "媒体会话暂停完成 aumid={} 状态={:?}",
                aumid,
                after
            );
            return Ok(Some(
                after != GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing,
            ));
        }
        Ok(None)
    };

    attempt().unwrap_or(None)
}

/// 带超时的媒体会话暂停。SMTC 首次调用可能阻塞较久，不能让它卡住切换流程。
pub fn try_pause_smtc_with_timeout(keyword: &str, timeout: std::time::Duration) -> Option<bool> {
    let key = keyword.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let result = try_pause_smtc(&key);
        let _ = sender.send(result);
    });

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(_) => {
            crate::logline!(
                "媒体会话响应超时（{}ms），改用键盘兜底",
                timeout.as_millis()
            );
            None
        }
    }
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