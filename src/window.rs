use anyhow::Result;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, RECT};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed,
    SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOWNA, SW_RESTORE, WS_EX_TOOLWINDOW,
};

pub fn is_valid(hwnd: HWND) -> bool {
    unsafe { IsWindow(hwnd).as_bool() }
}

pub fn foreground() -> HWND {
    unsafe { GetForegroundWindow() }
}

pub fn process_id(hwnd: HWND) -> u32 {
    if hwnd.0.is_null() {
        return 0;
    }
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
    }
    pid
}

pub fn window_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect).ok()?;
    }
    Some(rect)
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..len as usize])
    }
}

pub fn title(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..len as usize])
    }
}

fn process_exe_name(hwnd: HWND) -> Option<String> {
    let pid = process_id(hwnd);
    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, BOOL(0), pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        let path = String::from_utf16_lossy(&buffer[..size as usize]);
        path.rsplit('\\').next().map(|name| name.to_lowercase())
    }
}

pub struct WindowQuery<'a> {
    pub processes: &'a [String],
    pub class: Option<&'a str>,
    pub title_contains: Option<&'a str>,
}

struct SearchState<'a> {
    query: &'a WindowQuery<'a>,
    found: Option<HWND>,
}

unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut SearchState);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    if GetWindowLongW(hwnd, GWL_EXSTYLE) & (WS_EX_TOOLWINDOW.0 as i32) != 0 {
        return BOOL(1);
    }
    // 至少要有可见尺寸
    if let Some(rect) = window_rect(hwnd) {
        if rect.right - rect.left <= 0 || rect.bottom - rect.top <= 0 {
            return BOOL(1);
        }
    } else {
        return BOOL(1);
    }

    let query = state.query;
    let process_matched = process_exe_name(hwnd)
        .map(|name| query.processes.iter().any(|want| want.eq_ignore_ascii_case(&name)))
        .unwrap_or(false);

    if !process_matched {
        let class_matched = query
            .class
            .filter(|value| !value.is_empty())
            .map(|value| class_name(hwnd).eq_ignore_ascii_case(value))
            .unwrap_or(false);

        let title_matched = query
            .title_contains
            .filter(|value| !value.is_empty())
            .map(|value| title(hwnd).contains(value))
            .unwrap_or(false);

        if !class_matched && !title_matched {
            return BOOL(1);
        }
    }

    state.found = Some(hwnd);
    BOOL(0)
}

pub fn find_window(query: &WindowQuery<'_>) -> Option<HWND> {
    let mut state = SearchState {
        query,
        found: None,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut state as *mut _ as isize),
        );
    }
    state.found
}

pub fn restore_if_needed(hwnd: HWND) {
    unsafe {
        if IsIconic(hwnd).as_bool() || IsZoomed(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
    }
}

pub fn place_topmost(hwnd: HWND, rect: &RECT) -> Result<()> {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNA);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
    }
    Ok(())
}

/// 只移动、不改变层级、不激活。尺寸保持调用方给出的矩形。
pub fn place_passive(hwnd: HWND, rect: &RECT) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND::default(),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )?;
    }
    Ok(())
}

pub fn drop_topmost(hwnd: HWND) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_NOTOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
    }
    Ok(())
}

/// 抢前台。绕过前台锁后再校验，失败返回 false。
pub fn force_foreground(hwnd: HWND) -> bool {
    use windows::Win32::System::Threading::AttachThreadInput;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, SetForegroundWindow};

    unsafe {
        if GetForegroundWindow() == hwnd {
            return true;
        }

        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);

        let mut attached = false;
        if foreground_thread != 0 && target_thread != 0 && foreground_thread != target_thread {
            attached = AttachThreadInput(foreground_thread, target_thread, BOOL(1)).as_bool();
        }

        let _ = SetForegroundWindow(hwnd);

        if attached {
            let _ = AttachThreadInput(foreground_thread, target_thread, BOOL(0));
        }

        for _ in 0..20 {
            if GetForegroundWindow() == hwnd {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        false
    }
}

/// 取指定窗口（或主屏）所在显示器的工作区。
pub fn work_area(anchor: Option<HWND>) -> RECT {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };

    let hwnd = anchor.unwrap_or_default();
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() } {
        info.rcWork
    } else {
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        }
    }
}