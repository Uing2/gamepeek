use std::sync::mpsc::Sender;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, SetForegroundWindow,
    TrackPopupMenu, IDI_APPLICATION, MF_SEPARATOR, MF_STRING, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_APP,
};

use crate::controller::Event;
use crate::util::to_wide;

pub const TRAY_ICON_ID: u32 = 1;
pub const WM_TRAYICON: u32 = WM_APP + 1;

const TIP: &str = "GamePeek";

fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        ..Default::default()
    };

    data.hIcon = unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default();

    let wide = to_wide(TIP);
    let len = wide.len().min(data.szTip.len());
    data.szTip[..len].copy_from_slice(&wide[..len]);
    data
}

pub fn add(hwnd: HWND) {
    let data = icon_data(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
    }
}

pub fn remove(hwnd: HWND) {
    let data = icon_data(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

pub fn show_menu(hwnd: HWND, sender: &Sender<Event>) {
    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(menu) => menu,
            Err(_) => return,
        };

        let _ = AppendMenuW(menu, MF_STRING, 1, w!("切换"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, 2, w!("打开配置"));
        let _ = AppendMenuW(menu, MF_STRING, 3, w!("打开日志目录"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, 4, w!("退出"));

        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);

        // 必须先激活自身窗口，否则点击菜单外部不会消失。
        let _ = SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);

        match command.0 {
            1 => {
                let _ = sender.send(Event::Hotkey);
            }
            2 => crate::app::open_path(&crate::config::config_path()),
            3 => crate::app::open_path(&crate::config::data_dir().join("logs")),
            4 => {
                let _ = sender.send(Event::Exit);
            }
            _ => {}
        }
    }
}