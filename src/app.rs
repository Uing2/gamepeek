use std::path::Path;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;

use anyhow::Result;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, TranslateMessage,
    WM_CLOSE, WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_QUERYENDSESSION, WM_RBUTTONUP,
    WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW, HMENU, HWND_MESSAGE, MSG, SW_SHOWNORMAL,
};

use crate::config;
use crate::controller::{Controller, Event};
use crate::util::to_wide;

static SENDER: OnceLock<Sender<Event>> = OnceLock::new();
static MAIN_WINDOW: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

pub fn request_exit() {
    let raw = MAIN_WINDOW.load(Ordering::Relaxed);
    if raw == 0 {
        return;
    }
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    unsafe {
        let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

pub fn open_path(path: &Path) {
    let wide = to_wide(&path.to_string_lossy());
    unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn send(event: Event) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(event);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_HOTKEY => {
            send(Event::Hotkey);
            LRESULT(0)
        }
        crate::tray::WM_TRAYICON => {
            let action = (lparam.0 as u32) & 0xFFFF;
            match action {
                WM_LBUTTONUP => send(Event::Hotkey),
                WM_RBUTTONUP => {
                    if let Some(sender) = SENDER.get() {
                        crate::tray::show_menu(hwnd, sender);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_QUERYENDSESSION => {
            crate::logline!("系统即将关机/注销，执行还原");
            send(Event::Exit);
            LRESULT(1)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        other if other != 0 && other == TASKBAR_CREATED_MESSAGE.load(Ordering::Relaxed) => {
            crate::logline!("资源管理器重启，重建托盘图标");
            crate::tray::add(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

pub fn run() -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    crate::diagnostics::disable_power_throttling();

    let config = config::load_or_create()?;
    if let Err(_err) = crate::diagnostics::init(&config::log_path()) {
        #[cfg(debug_assertions)]
        eprintln!("日志初始化失败: {_err}");
    }
    crate::logline!("GamePeek 启动，热键={}", config.hotkey);
    crate::autostart::apply(config.app.start_with_windows);

    let instance = unsafe { GetModuleHandleW(None)? };
    let hinstance = HINSTANCE(instance.0);
    let class_name = w!("GamePeekWindowClass");

    let window_class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&window_class);
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("GamePeek"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            HMENU::default(),
            hinstance,
            None,
        )?
    };

    MAIN_WINDOW.store(hwnd.0 as isize, Ordering::Relaxed);
    TASKBAR_CREATED_MESSAGE.store(
        unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        Ordering::Relaxed,
    );

    let (sender, receiver) = channel::<Event>();
    let _ = SENDER.set(sender.clone());

    match crate::hotkey::register(hwnd, &config.hotkey) {
        Ok((_, virtual_key)) => crate::logline!("热键注册成功 vk=0x{virtual_key:X}"),
        Err(err) => crate::logline!("{err:#}"),
    }

    crate::foreground::init(sender.clone());
    let hook = match crate::foreground::install() {
        Ok(hook) => Some(hook),
        Err(err) => {
            crate::logline!("前台监听安装失败: {err:#}");
            None
        }
    };

    crate::tray::add(hwnd);

    let worker_config = config.clone();
    let worker = std::thread::Builder::new()
        .name("gamepeek-worker".into())
        .spawn(move || match Controller::new(worker_config) {
            Ok(controller) => controller.run(receiver),
            Err(err) => crate::logline!("工作线程初始化失败: {err:#}"),
        })?;

    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0).as_bool() } {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    crate::logline!("消息循环结束，开始清理");
    let _ = sender.send(Event::Exit);
    let _ = crate::tray::remove(hwnd);

    if let Some(hook) = hook {
        crate::foreground::uninstall(hook);
    }
    crate::hotkey::unregister(hwnd);

    unsafe {
        let _ = DestroyWindow(hwnd);
    }

    let _ = worker.join();
    crate::logline!("GamePeek 已退出");
    Ok(())
}