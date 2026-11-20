use std::ffi::c_void;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW,
    EndPaint, GetStockObject, HBRUSH, HFONT, HRGN, InvalidateRect, RoundRect, SelectClipRgn,
    SelectObject, SetBkColor, SetBkMode, SetTextColor, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
    DEFAULT_CHARSET, DEFAULT_PITCH, DT_LEFT, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FONT_WEIGHT,
    NULL_PEN, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetClientRect,
    GetForegroundWindow, GetWindowLongPtrW, GetWindowTextW, KillTimer, MessageBoxW,
    RegisterClassW, SendMessageW, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    SetWindowTextW, ShowWindow, GWLP_USERDATA, HMENU, ICON_BIG, ICON_SMALL, MB_ICONERROR, MB_OK,
    SW_SHOW, WM_CLOSE, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_PAINT, WM_SETFONT, WM_SETICON, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW,
    WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};

use crate::config::Config;
use crate::hotkey;
use crate::util::to_wide;
use crate::window;

const CLASS_NAME: PCWSTR = windows::core::w!("GamePeekSettingsClass");
const TIMER_PICK: usize = 1;

// Douyin 风格配色
const BG: u32 = 0x14_14_18; // #141418
const PANEL: u32 = 0x1D_1D_24;
const BORDER: u32 = 0x33_33_3E;
const TEXT: u32 = 0xEC_EFF6;
const MUTED: u32 = 0x9B_A1_B0;
const CYAN: u32 = 0xEE_F4_25; // #25F4EE
const PINK: u32 = 0x55_2C_FE; // #FE2C55

const ID_HOTKEY: i32 = 1001;
const ID_GAME: i32 = 1002;
const ID_TARGET: i32 = 1003;
const ID_TITLE: i32 = 1004;
const ID_WIDTH: i32 = 1005;
const ID_HEIGHT: i32 = 1006;
const ID_MARGIN: i32 = 1007;
const ID_POSITION: i32 = 1010;
const ID_HIDE: i32 = 1011;
const ID_PAUSE: i32 = 1012;
const ID_AUTO_RETURN: i32 = 1020;
const ID_AUTOSTART: i32 = 1021;
const ID_CAPTURE: i32 = 1100;
const ID_PICK: i32 = 1101;
const ID_SAVE: i32 = 1102;
const ID_CANCEL: i32 = 1103;
const ID_OPEN_CONFIG: i32 = 1104;
const ID_OPEN_LOGS: i32 = 1105;

const POSITIONS: [&str; 5] = ["bottom-right", "bottom-left", "top-right", "top-left", "remember"];
const POSITION_LABELS: [&str; 5] = ["右下", "左下", "右上", "左上", "记住"];
const HIDE_MODES: [&str; 2] = ["offscreen", "minimize"];
const HIDE_LABELS: [&str; 2] = ["移出屏幕", "最小化"];
const PAUSE_METHODS: [&str; 4] = ["auto", "space", "smtc", "none"];
const PAUSE_LABELS: [&str; 4] = ["自动", "仅空格", "仅媒体", "不暂停"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum BtnStyle {
    Primary,
    Ghost,
    Accent,
}

struct Segment {
    id: i32,
    selected: usize,
    options: &'static [&'static str],
    rects: Vec<RECT>,
}

struct Check {
    id: i32,
    label: &'static str,
    rect: RECT,
    checked: bool,
}

struct Btn {
    id: i32,
    label: &'static str,
    rect: RECT,
    style: BtnStyle,
}

struct SettingsState {
    config: Config,
    scale: f32,
    font_title: HFONT,
    font_body: HFONT,
    font_small: HFONT,
    brush_panel: HBRUSH,
    brush_bg: HBRUSH,
    edits: Vec<(i32, HWND)>,
    segments: Vec<Segment>,
    checks: Vec<Check>,
    buttons: Vec<Btn>,
    status: String,
    capturing: bool,
    countdown: i32,
}

impl SettingsState {
    fn edit(&self, id: i32) -> HWND {
        self.edits
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, hwnd)| *hwnd)
            .unwrap_or_default()
    }

    fn segment(&self, id: i32) -> &Segment {
        self.segments.iter().find(|s| s.id == id).expect("segment")
    }

}

fn rgb(value: u32) -> COLORREF {
    COLORREF(value & 0x00FF_FFFF)
}

fn scale_rect(scale: f32, x: i32, y: i32, width: i32, height: i32) -> RECT {
    RECT {
        left: (x as f32 * scale).round() as i32,
        top: (y as f32 * scale).round() as i32,
        right: ((x + width) as f32 * scale).round() as i32,
        bottom: ((y + height) as f32 * scale).round() as i32,
    }
}

fn text_of(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..len as usize])
    }
}

fn set_text(hwnd: HWND, value: &str) {
    let wide = to_wide(value);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
    }
}

fn show_error(hwnd: HWND, message: &str) {
    let text = to_wide(message);
    let caption = to_wide("GamePeek");
    unsafe {
        MessageBoxW(hwnd, PCWSTR(text.as_ptr()), PCWSTR(caption.as_ptr()), MB_OK | MB_ICONERROR);
    }
}

unsafe fn fill_round(hdc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT, radius: i32, color: u32) {
    let brush = CreateSolidBrush(rgb(color));
    let old_brush = SelectObject(hdc, brush);
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));

    let _ = RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, radius, radius);

    let _ = SelectObject(hdc, old_pen);
    let _ = SelectObject(hdc, old_brush);
    let _ = DeleteObject(brush);
}

/// 圆角矩形里的水平渐变。GDI 没有原生渐变画刷，用逐列描线实现。
unsafe fn fill_gradient(hdc: windows::Win32::Graphics::Gdi::HDC, rect: &RECT, radius: i32, from: u32, to: u32) {
    let region = CreateRoundRectRgn(
        rect.left,
        rect.top,
        rect.right + 1,
        rect.bottom + 1,
        radius,
        radius,
    );
    let _ = SelectClipRgn(hdc, region);

    let width = (rect.right - rect.left).max(1);
    for offset in 0..width {
        let t = offset as f32 / width as f32;
        let color = lerp_color(from, to, t);
        let line = RECT {
            left: rect.left + offset,
            top: rect.top,
            right: rect.left + offset + 1,
            bottom: rect.bottom,
        };
        let brush = CreateSolidBrush(rgb(color));
        let old = SelectObject(hdc, brush);
        let _ = windows::Win32::Graphics::Gdi::PatBlt(
            hdc,
            line.left,
            line.top,
            line.right - line.left,
            line.bottom - line.top,
            windows::Win32::Graphics::Gdi::PATCOPY,
        );
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(brush);
    }

    let _ = SelectClipRgn(hdc, HRGN::default());
    let _ = DeleteObject(region);
}

fn lerp_color(from: u32, to: u32, t: f32) -> u32 {
    let fr = (from & 0xFF) as f32;
    let fg = ((from >> 8) & 0xFF) as f32;
    let fb = ((from >> 16) & 0xFF) as f32;
    let tr = (to & 0xFF) as f32;
    let tg = ((to >> 8) & 0xFF) as f32;
    let tb = ((to >> 16) & 0xFF) as f32;

    let r = (fr + (tr - fr) * t).round() as u32;
    let g = (fg + (tg - fg) * t).round() as u32;
    let b = (fb + (tb - fb) * t).round() as u32;
    r | (g << 8) | (b << 16)
}

unsafe fn stroke_round(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    rect: &RECT,
    radius: i32,
    color: u32,
    width: i32,
) {
    let brush = CreateSolidBrush(rgb(BG));
    let pen = CreatePen(PS_SOLID, width.max(1), rgb(color));
    let old_brush = SelectObject(hdc, brush);
    let old_pen = SelectObject(hdc, pen);

    let _ = RoundRect(hdc, rect.left, rect.top, rect.right, rect.bottom, radius, radius);

    let _ = SelectObject(hdc, old_brush);
    let _ = SelectObject(hdc, old_pen);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);
}

unsafe fn draw_text(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    rect: &RECT,
    font: HFONT,
    color: u32,
    flags: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    let old = SelectObject(hdc, font);
    let _ = SetBkMode(hdc, TRANSPARENT);
    let _ = SetTextColor(hdc, rgb(color));

    let mut wide = to_wide(text);
    let mut area = *rect;
    let _ = DrawTextW(hdc, &mut wide, &mut area, flags);

    let _ = SelectObject(hdc, old);
}

unsafe fn draw_text_center(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    rect: &RECT,
    font: HFONT,
    color: u32,
) {
    draw_text(
        hdc,
        text,
        rect,
        font,
        color,
        DT_SINGLELINE | DT_VCENTER | windows::Win32::Graphics::Gdi::DT_CENTER,
    );
}

pub fn open(_parent: HWND) {
    unsafe {
        if let Ok(existing) = FindWindowW(CLASS_NAME, PCWSTR::null()) {
            if !existing.0.is_null() {
                let _ = ShowWindow(existing, SW_SHOW);
                let _ = SetForegroundWindow(existing);
                return;
            }
        }
    }

    let instance = match unsafe { GetModuleHandleW(None) } {
        Ok(module) => windows::Win32::Foundation::HINSTANCE(module.0),
        Err(_) => return,
    };

    let window_class = WNDCLASSW {
        lpfnWndProc: Some(settings_proc),
        hInstance: instance,
        lpszClassName: CLASS_NAME,
        hbrBackground: HBRUSH::default(),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&window_class);
    }

    let area = window::work_area(None);
    let dpi = unsafe { GetDpiForSystem() };
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };

    let style = windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
        WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_VISIBLE.0 | WS_CLIPCHILDREN.0,
    );

    let mut frame = RECT {
        left: 0,
        top: 0,
        right: (640.0 * scale).round() as i32,
        bottom: (560.0 * scale).round() as i32,
    };
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::AdjustWindowRect(&mut frame, style, false);
    }
    let width = frame.right - frame.left;
    let height = frame.bottom - frame.top;
    let x = area.left + ((area.right - area.left) - width) / 2;
    let y = area.top + ((area.bottom - area.top) - height) / 2;

    let hwnd = unsafe {
        CreateWindowExW(
            Default::default(),
            CLASS_NAME,
            windows::core::w!("GamePeek 设置"),
            style,
            x,
            y,
            width,
            height,
            HWND::default(),
            HMENU::default(),
            instance,
            None,
        )
    };

    let hwnd = match hwnd {
        Ok(hwnd) => hwnd,
        Err(_) => return,
    };

    // 深色标题栏（Win10 1809+ / Win11 支持）
    unsafe {
        let dark = windows::Win32::Foundation::BOOL(1);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const c_void,
            std::mem::size_of::<windows::Win32::Foundation::BOOL>() as u32,
        );
    }

    build_ui(hwnd, instance);

    unsafe {
        if let Some(icon) = crate::icon::load(16) {
            SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(icon.0 as isize));
        }
        if let Some(icon) = crate::icon::load(32) {
            SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(icon.0 as isize));
        }
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

fn make_font(height: i32, weight: FONT_WEIGHT, scale: f32) -> HFONT {
    let face = to_wide("Microsoft YaHei");
    unsafe {
        CreateFontW(
            -((height as f32 * scale).round() as i32),
            0,
            0,
            0,
            weight.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            CLEARTYPE_QUALITY.0 as u32,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            PCWSTR(face.as_ptr()),
        )
    }
}

fn create_edit(
    parent: HWND,
    instance: windows::Win32::Foundation::HINSTANCE,
    scale: f32,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
    font: HFONT,
) -> HWND {
    const ES_AUTOHSCROLL: u32 = 0x0080;
    let rect = scale_rect(scale, x, y, width, height);
    let wide = to_wide(text);

    let hwnd = unsafe {
        CreateWindowExW(
            Default::default(),
            windows::core::w!("EDIT"),
            PCWSTR(wide.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
                WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL,
            ),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            parent,
            HMENU(id as isize as *mut c_void),
            instance,
            None,
        )
    }
    .unwrap_or_default();

    if !hwnd.0.is_null() {
        unsafe {
            SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        }
    }
    hwnd
}

fn build_ui(hwnd: HWND, instance: windows::Win32::Foundation::HINSTANCE) {
    let config = crate::app::current_config();
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };

    let font_title = make_font(19, FONT_WEIGHT(600), scale);
    let font_body = make_font(13, FONT_WEIGHT(400), scale);
    let font_small = make_font(11, FONT_WEIGHT(400), scale);

    let mut state = SettingsState {
        config: config.clone(),
        scale,
        font_title,
        font_body,
        font_small,
        brush_panel: unsafe { CreateSolidBrush(rgb(PANEL)) },
        brush_bg: unsafe { CreateSolidBrush(rgb(BG)) },
        edits: Vec::new(),
        segments: Vec::new(),
        checks: Vec::new(),
        buttons: Vec::new(),
        status: String::new(),
        capturing: false,
        countdown: 0,
    };

    let mut add_edit = |id: i32, text: &str, x: i32, y: i32, width: i32| {
        let hwnd_edit = create_edit(hwnd, instance, scale, text, x, y, width, 30, id, font_body);
        state.edits.push((id, hwnd_edit));
    };

    add_edit(ID_HOTKEY, &config.hotkey, 140, 92, 150);
    add_edit(ID_GAME, &config.game.process, 140, 132, 220);
    add_edit(ID_TARGET, &config.target.process, 140, 172, 220);
    add_edit(ID_TITLE, &config.target.window_title, 140, 212, 220);
    add_edit(ID_WIDTH, &config.layout.width.to_string(), 140, 294, 72);
    add_edit(ID_HEIGHT, &config.layout.height.to_string(), 224, 294, 72);
    add_edit(ID_MARGIN, &config.layout.margin.to_string(), 308, 294, 72);

    state.buttons.push(Btn {
        id: ID_CAPTURE,
        label: "录制",
        rect: scale_rect(scale, 300, 92, 76, 30),
        style: BtnStyle::Accent,
    });
    state.buttons.push(Btn {
        id: ID_PICK,
        label: "获取前台窗口",
        rect: scale_rect(scale, 372, 172, 170, 30),
        style: BtnStyle::Accent,
    });
    state.buttons.push(Btn {
        id: ID_OPEN_CONFIG,
        label: "打开配置",
        rect: scale_rect(scale, 24, 504, 120, 36),
        style: BtnStyle::Ghost,
    });
    state.buttons.push(Btn {
        id: ID_OPEN_LOGS,
        label: "打开日志",
        rect: scale_rect(scale, 152, 504, 120, 36),
        style: BtnStyle::Ghost,
    });
    state.buttons.push(Btn {
        id: ID_CANCEL,
        label: "取消",
        rect: scale_rect(scale, 410, 504, 88, 36),
        style: BtnStyle::Ghost,
    });
    state.buttons.push(Btn {
        id: ID_SAVE,
        label: "保存",
        rect: scale_rect(scale, 506, 504, 110, 36),
        style: BtnStyle::Primary,
    });

    let position_index = POSITIONS
        .iter()
        .position(|value| *value == config.layout.position)
        .unwrap_or(0);
    state.segments.push(Segment {
        id: ID_POSITION,
        selected: position_index,
        options: &POSITION_LABELS,
        rects: segment_rects(scale, 140, 254, 84, 8, POSITION_LABELS.len()),
    });

    let hide_index = HIDE_MODES
        .iter()
        .position(|value| *value == config.layout.hide_mode)
        .unwrap_or(0);
    state.segments.push(Segment {
        id: ID_HIDE,
        selected: hide_index,
        options: &HIDE_LABELS,
        rects: segment_rects(scale, 140, 334, 96, 8, HIDE_LABELS.len()),
    });

    let pause_index = PAUSE_METHODS
        .iter()
        .position(|value| *value == config.pause.method)
        .unwrap_or(0);
    state.segments.push(Segment {
        id: ID_PAUSE,
        selected: pause_index,
        options: &PAUSE_LABELS,
        rects: segment_rects(scale, 140, 374, 90, 8, PAUSE_LABELS.len()),
    });

    state.checks.push(Check {
        id: ID_AUTO_RETURN,
        label: "切回游戏自动返回",
        rect: scale_rect(scale, 140, 420, 200, 24),
        checked: config.auto_return.on_game_focus,
    });
    state.checks.push(Check {
        id: ID_AUTOSTART,
        label: "开机自启",
        rect: scale_rect(scale, 372, 420, 140, 24),
        checked: config.app.start_with_windows,
    });

    let state = Box::new(state);
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    }
}

fn segment_rects(scale: f32, x: i32, y: i32, width: i32, gap: i32, count: usize) -> Vec<RECT> {
    let mut rects = Vec::new();
    for index in 0..count {
        let left = x + index as i32 * (width + gap);
        rects.push(scale_rect(scale, left, y, width, 30));
    }
    rects
}

fn inside(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

unsafe fn paint(hwnd: HWND, state: &SettingsState) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);

    // 背景
    let bg = CreateSolidBrush(rgb(BG));
    windows::Win32::Graphics::Gdi::FillRect(hdc, &client, bg);
    let _ = DeleteObject(bg);

    let title_rect = scale_rect(state.scale, 24, 18, 400, 30);
    draw_text(hdc, "GamePeek", &title_rect, state.font_title, TEXT, DT_SINGLELINE | DT_VCENTER | DT_LEFT);

    let badge = scale_rect(state.scale, 24, 50, 96, 18);
    draw_text(hdc, "GAME × DOUYIN", &badge, state.font_small, CYAN, DT_SINGLELINE | DT_VCENTER | DT_LEFT);

    let accent = scale_rect(state.scale, 24, 72, 592, 2);
    fill_gradient(hdc, &accent, 2, CYAN, PINK);

    for (label, x, y) in [
        ("切换热键", 24, 100),
        ("游戏进程", 24, 140),
        ("目标进程", 24, 180),
        ("窗口标题含", 24, 220),
        ("显示位置", 24, 262),
        ("宽 / 高 / 边距", 24, 302),
        ("返回时", 24, 342),
        ("暂停方式", 24, 382),
    ] {
        let rect = scale_rect(state.scale, x, y, 260, 20);
        draw_text(hdc, label, &rect, state.font_body, MUTED, DT_SINGLELINE | DT_VCENTER | DT_LEFT);
    }

    // 输入框底衬
    for (x, y, width) in [
        (140, 92, 150),
        (140, 132, 220),
        (140, 172, 220),
        (140, 212, 220),
        (140, 294, 72),
        (224, 294, 72),
        (308, 294, 72),
    ] {
        let rect = scale_rect(state.scale, x - 1, y - 1, width + 2, 32);
        fill_round(hdc, &rect, (8.0 * state.scale).round() as i32, PANEL);
        stroke_round(hdc, &rect, (8.0 * state.scale).round() as i32, BORDER, (1.5 * state.scale).round() as i32);
    }

    for segment in &state.segments {
        for (index, rect) in segment.rects.iter().enumerate() {
            if index == segment.selected {
                fill_gradient(hdc, rect, 15, CYAN, PINK);
                draw_text_center(hdc, segment.options[index], rect, state.font_body, 0x000000);
            } else {
                fill_round(hdc, rect, (15.0 * state.scale).round() as i32, PANEL);
                stroke_round(hdc, rect, (15.0 * state.scale).round() as i32, BORDER, (1.5 * state.scale).round() as i32);
                draw_text_center(hdc, segment.options[index], rect, state.font_body, MUTED);
            }
        }
    }

    for check in &state.checks {
        let box_size = (18.0 * state.scale).round() as i32;
        let box_rect = RECT {
            left: check.rect.left,
            top: check.rect.top + ((check.rect.bottom - check.rect.top) - box_size) / 2,
            right: check.rect.left + box_size,
            bottom: check.rect.top + ((check.rect.bottom - check.rect.top) - box_size) / 2 + box_size,
        };

        if check.checked {
            fill_gradient(hdc, &box_rect, 6, CYAN, PINK);

            // 勾选标记
            let pen = CreatePen(PS_SOLID, (2.4 * state.scale).round().max(1.0) as i32, rgb(0x0C0C10));
            let old = SelectObject(hdc, pen);
            let w = box_rect.right - box_rect.left;
            let h = box_rect.bottom - box_rect.top;
            let _ = windows::Win32::Graphics::Gdi::MoveToEx(
                hdc,
                box_rect.left + (w as f32 * 0.24) as i32,
                box_rect.top + (h as f32 * 0.52) as i32,
                None,
            );
            let _ = windows::Win32::Graphics::Gdi::LineTo(
                hdc,
                box_rect.left + (w as f32 * 0.43) as i32,
                box_rect.top + (h as f32 * 0.72) as i32,
            );
            let _ = windows::Win32::Graphics::Gdi::MoveToEx(
                hdc,
                box_rect.left + (w as f32 * 0.43) as i32,
                box_rect.top + (h as f32 * 0.72) as i32,
                None,
            );
            let _ = windows::Win32::Graphics::Gdi::LineTo(
                hdc,
                box_rect.left + (w as f32 * 0.78) as i32,
                box_rect.top + (h as f32 * 0.30) as i32,
            );
            let _ = SelectObject(hdc, old);
            let _ = DeleteObject(pen);
        } else {
            fill_round(hdc, &box_rect, 6, PANEL);
            stroke_round(hdc, &box_rect, 6, BORDER, (1.5 * state.scale).round() as i32);
        }

        let label_rect = RECT {
            left: check.rect.left + box_size + (8.0 * state.scale).round() as i32,
            top: check.rect.top,
            right: check.rect.right,
            bottom: check.rect.bottom,
        };
        draw_text(
            hdc,
            check.label,
            &label_rect,
            state.font_body,
            if check.checked { TEXT } else { MUTED },
            DT_SINGLELINE | DT_VCENTER | DT_LEFT,
        );
    }

    for button in &state.buttons {
        match button.style {
            BtnStyle::Primary => {
                fill_gradient(hdc, &button.rect, 18, CYAN, PINK);
                draw_text_center(hdc, button.label, &button.rect, state.font_body, 0x000000);
            }
            BtnStyle::Ghost => {
                fill_round(hdc, &button.rect, (18.0 * state.scale).round() as i32, PANEL);
                stroke_round(hdc, &button.rect, (18.0 * state.scale).round() as i32, BORDER, (1.5 * state.scale).round() as i32);
                draw_text_center(hdc, button.label, &button.rect, state.font_body, TEXT);
            }
            BtnStyle::Accent => {
                fill_round(hdc, &button.rect, (15.0 * state.scale).round() as i32, 0x1A_1A_22);
                stroke_round(hdc, &button.rect, (15.0 * state.scale).round() as i32, CYAN, (2.0 * state.scale).round() as i32);
                draw_text_center(hdc, button.label, &button.rect, state.font_body, CYAN);
            }
        }
    }

    if !state.status.is_empty() {
        let rect = scale_rect(state.scale, 24, 464, 592, 20);
        draw_text(hdc, &state.status, &rect, state.font_small, MUTED, DT_SINGLELINE | DT_VCENTER | DT_LEFT);
    }

    let _ = EndPaint(hwnd, &ps);
}

fn on_click(state: &mut SettingsState, hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let _ = SetFocus(hwnd);
    }

    for segment in state.segments.iter_mut() {
        for (index, rect) in segment.rects.iter().enumerate() {
            if inside(rect, x, y) {
                segment.selected = index;
                unsafe {
                    let _ = InvalidateRect(hwnd, None, false);
                }
                return;
            }
        }
    }

    for check in state.checks.iter_mut() {
        if inside(&check.rect, x, y) {
            check.checked = !check.checked;
            unsafe {
                let _ = InvalidateRect(hwnd, None, false);
            }
            return;
        }
    }

    let button_id = state
        .buttons
        .iter()
        .find(|button| inside(&button.rect, x, y))
        .map(|button| button.id);

    match button_id {
        Some(ID_SAVE) => save(state, hwnd),
        Some(ID_CANCEL) => unsafe {
            let _ = DestroyWindow(hwnd);
        },
        Some(ID_CAPTURE) => begin_capture(state, hwnd),
        Some(ID_PICK) => start_pick(state, hwnd),
        Some(ID_OPEN_CONFIG) => crate::app::open_path(&crate::config::config_path()),
        Some(ID_OPEN_LOGS) => crate::app::open_path(&crate::config::data_dir().join("logs")),
        _ => {}
    }
}

fn save(state: &SettingsState, hwnd: HWND) {
    let hotkey_text = text_of(state.edit(ID_HOTKEY)).trim().to_string();
    if let Err(err) = hotkey::parse(&hotkey_text) {
        show_error(hwnd, &format!("热键无效：{err}"));
        return;
    }

    let mut config = state.config.clone();
    config.hotkey = hotkey_text;
    config.game.process = text_of(state.edit(ID_GAME)).trim().to_string();
    config.target.process = text_of(state.edit(ID_TARGET)).trim().to_string();
    config.target.window_title = text_of(state.edit(ID_TITLE)).trim().to_string();
    config.layout.position = POSITIONS[state.segment(ID_POSITION).selected].to_string();
    config.layout.hide_mode = HIDE_MODES[state.segment(ID_HIDE).selected].to_string();
    config.pause.method = PAUSE_METHODS[state.segment(ID_PAUSE).selected].to_string();
    config.layout.width = text_of(state.edit(ID_WIDTH)).trim().parse().unwrap_or(config.layout.width);
    config.layout.height = text_of(state.edit(ID_HEIGHT)).trim().parse().unwrap_or(config.layout.height);
    config.layout.margin = text_of(state.edit(ID_MARGIN)).trim().parse().unwrap_or(config.layout.margin);
    config.auto_return.on_game_focus = state
        .checks
        .iter()
        .find(|check| check.id == ID_AUTO_RETURN)
        .map(|check| check.checked)
        .unwrap_or(true);
    config.app.start_with_windows = state
        .checks
        .iter()
        .find(|check| check.id == ID_AUTOSTART)
        .map(|check| check.checked)
        .unwrap_or(false);

    if config.game.process.is_empty() || config.target.process.is_empty() {
        show_error(hwnd, "游戏进程和目标进程不能为空");
        return;
    }

    match crate::app::apply_config(config) {
        Ok(()) => unsafe {
            let _ = DestroyWindow(hwnd);
        },
        Err(err) => show_error(hwnd, &format!("保存失败：{err:#}")),
    }
}

fn begin_capture(state: &mut SettingsState, hwnd: HWND) {
    state.capturing = true;
    state.status = "按下组合键，Esc 取消".into();
    set_text(state.edit(ID_HOTKEY), "按下组合键…");
    unsafe {
        let _ = SetFocus(hwnd);
        let _ = InvalidateRect(hwnd, None, false);
    }
}

fn finish_capture(state: &mut SettingsState, hwnd: HWND, vk: u32) {
    if vk == VK_ESCAPE.0 as u32 {
        state.capturing = false;
        set_text(state.edit(ID_HOTKEY), &state.config.hotkey);
        state.status = String::new();
        unsafe {
            let _ = InvalidateRect(hwnd, None, false);
        }
        return;
    }

    let modifier_keys = [
        VK_SHIFT.0 as u32,
        VK_CONTROL.0 as u32,
        VK_MENU.0 as u32,
        VK_LWIN.0 as u32,
        VK_RWIN.0 as u32,
    ];
    if modifier_keys.contains(&vk) {
        return;
    }

    let pressed = |key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe {
        GetKeyState(key.0 as i32) < 0
    };

    let mut parts: Vec<String> = Vec::new();
    if pressed(VK_CONTROL) {
        parts.push("Ctrl".into());
    }
    if pressed(VK_MENU) {
        parts.push("Alt".into());
    }
    if pressed(VK_SHIFT) {
        parts.push("Shift".into());
    }
    if pressed(VK_LWIN) || pressed(VK_RWIN) {
        parts.push("Win".into());
    }

    match vk_name(vk) {
        Some(name) => {
            parts.push(name);
            state.capturing = false;
            let spec = parts.join("+");
            set_text(state.edit(ID_HOTKEY), &spec);
            state.status = "已录制，保存后生效".into();
        }
        None => state.status = "这个键不支持，换一个试试".into(),
    }

    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

fn vk_name(vk: u32) -> Option<String> {
    if (0x70..=0x87).contains(&vk) {
        return Some(format!("F{}", vk - 0x70 + 1));
    }
    if (0x41..=0x5A).contains(&vk) {
        return Some(((vk as u8) as char).to_string());
    }
    if (0x30..=0x39).contains(&vk) {
        return Some(((vk as u8) as char).to_string());
    }
    let name = match vk {
        0x20 => "Space",
        0x09 => "Tab",
        0x0D => "Enter",
        0x08 => "Backspace",
        0x2D => "Insert",
        0x2E => "Delete",
        0x24 => "Home",
        0x23 => "End",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x05 => "Mouse4",
        0x06 => "Mouse5",
        0xC0 => "`",
        _ => return None,
    };
    Some(name.to_string())
}

fn start_pick(state: &mut SettingsState, hwnd: HWND) {
    state.countdown = 3;
    state.status = "请切换到抖音窗口，3 秒后读取…".into();
    unsafe {
        SetTimer(hwnd, TIMER_PICK, 1000, None);
        let _ = InvalidateRect(hwnd, None, false);
    }
}

fn tick_pick(state: &mut SettingsState, hwnd: HWND) {
    state.countdown -= 1;

    if state.countdown > 0 {
        state.status = format!("请切换到抖音窗口，{} 秒后读取…", state.countdown);
        unsafe {
            let _ = InvalidateRect(hwnd, None, false);
        }
        return;
    }

    unsafe {
        let _ = KillTimer(hwnd, TIMER_PICK);
    }

    let foreground = unsafe { GetForegroundWindow() };
    if foreground == hwnd || foreground.0.is_null() {
        state.status = "没检测到其他窗口，请重试".into();
    } else {
        let exe = window::exe_name(foreground).unwrap_or_default();
        let title = window::title(foreground);
        if !exe.is_empty() {
            set_text(state.edit(ID_TARGET), &exe);
        }
        if !title.is_empty() {
            set_text(state.edit(ID_TITLE), &title);
        }
        state.status = format!("已获取：{exe} / {title}");
    }

    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw != 0 {
                let state = &*(raw as *mut SettingsState);
                paint(hwnd, state);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw != 0 {
                let state = &*(raw as *mut SettingsState);
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut c_void);
                let _ = SetTextColor(hdc, rgb(TEXT));
                let _ = SetBkColor(hdc, rgb(PANEL));
                return LRESULT(state.brush_panel.0 as isize);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_LBUTTONDOWN => {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw != 0 {
                let state = &mut *(raw as *mut SettingsState);
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                on_click(state, hwnd, x, y);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_PICK {
                let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if raw != 0 {
                    let state = &mut *(raw as *mut SettingsState);
                    tick_pick(state, hwnd);
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw != 0 {
                let state = &mut *(raw as *mut SettingsState);
                if state.capturing {
                    finish_capture(state, hwnd, wparam.0 as u32);
                    return LRESULT(0);
                }
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw != 0 {
                let state = Box::from_raw(raw as *mut SettingsState);
                unsafe {
                    let _ = DeleteObject(state.font_title);
                    let _ = DeleteObject(state.font_body);
                    let _ = DeleteObject(state.font_small);
                    let _ = DeleteObject(state.brush_panel);
                    let _ = DeleteObject(state.brush_bg);
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}