use std::sync::mpsc::Sender;
use std::sync::OnceLock;

use anyhow::Result;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT};

use crate::controller::Event;
use crate::util::hwnd_to_i64;

static SENDER: OnceLock<Sender<Event>> = OnceLock::new();

pub fn init(sender: Sender<Event>) {
    let _ = SENDER.set(sender);
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_thread: u32,
    _time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        if let Some(sender) = SENDER.get() {
            let _ = sender.send(Event::Foreground(hwnd_to_i64(hwnd)));
        }
    }
}

/// WINEVENT_OUTOFCONTEXT 的回调会投递到安装它的线程，因此必须在消息循环线程上调用。
pub fn install() -> Result<HWINEVENTHOOK> {
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };

    if hook.is_invalid() {
        anyhow::bail!("SetWinEventHook 注册失败");
    }
    Ok(hook)
}

pub fn uninstall(hook: HWINEVENTHOOK) {
    unsafe {
        let _ = UnhookWinEvent(hook);
    }
}