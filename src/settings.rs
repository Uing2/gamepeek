use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Controls::BST_CHECKED;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetForegroundWindow,
    GetWindowLongPtrW, GetWindowTextW, KillTimer, MessageBoxW, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CB_ADDSTRING,
    CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST, GWLP_USERDATA, HMENU, ICON_BIG, ICON_SMALL,
    MB_ICONERROR, MB_OK, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_SETFONT,
    WM_SETICON, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_OVERLAPPED,
    WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use crate::config::Config;
use crate::hotkey;
use crate::util::to_wide;
use crate::window;

const CLASS_NAME: PCWSTR = windows::core::w!("GamePeekSettingsClass");

const ID_HOTKEY: i32 = 1001;
const ID_GAME: i32 = 1002;
const ID_TARGET: i32 = 1003;
const ID_TITLE: i32 = 1004;
const ID_POSITION: i32 = 1005;
const ID_WIDTH: i32 = 1006;
const ID_HEIGHT: i32 = 1007;
const ID_MARGIN: i32 = 1008;
const ID_HIDE: i32 = 1009;
const ID_PAUSE: i32 = 1010;
const ID_AUTO_RETURN: i32 = 1011;
const ID_AUTOSTART: i32 = 1012;
const ID_CAPTURE: i32 = 1100;
const ID_PICK: i32 = 1101;
const ID_SAVE: i32 = 1102;
const ID_CANCEL: i32 = 1103;
const ID_OPEN_CONFIG: i32 = 1104;
const ID_OPEN_LOGS: i32 = 1105;
const ID_STATUS: i32 = 1200;

const TIMER_PICK: usize = 1;

const POSITIONS: [&str; 5] = ["bottom-right", "bottom-left", "top-right", "top-left", "remember"];
const POSITION_LABELS: [&str; 5] = ["右下角", "左下角", "右上角", "左上角", "记住上次位置"];
const HIDE_MODES: [&str; 2] = ["offscreen", "minimize"];
const HIDE_LABELS: [&str; 2] = ["移出屏幕", "最小化"];
const PAUSE_METHODS: [&str; 4] = ["auto", "space", "smtc", "none"];
const PAUSE_LABELS: [&str; 4] = ["自动", "仅发送空格", "仅媒体会话", "不暂停"];

struct SettingsState {
    config: Config,
    controls: Vec<(i32, HWND)>,
    capturing: bool,
    countdown: i32,
}

impl SettingsState {
    fn control(&self, id: i32) -> HWND {
        self.controls
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, hwnd)| *hwnd)
            .unwrap_or_default()
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
        MessageBoxW(
            hwnd,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

struct Builder {
    parent: HWND,
    font: windows::Win32::Graphics::Gdi::HGDIOBJ,
    instance: HINSTANCE,
    scale: f32,
}

impl Builder {
    fn s(&self, value: i32) -> i32 {
        ((value as f32) * self.scale).round() as i32
    }

    fn create(
        &self,
        class: PCWSTR,
        text: &str,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        id: i32,
    ) -> HWND {
        let wide = to_wide(text);
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                class,
                PCWSTR(wide.as_ptr()),
                windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(style),
                self.s(x),
                self.s(y),
                self.s(width),
                self.s(height),
                self.parent,
                HMENU(id as isize as *mut core::ffi::c_void),
                self.instance,
                None,
            )
        }
        .unwrap_or_default();

        if !hwnd.0.is_null() {
            unsafe {
                SendMessageW(
                    hwnd,
                    WM_SETFONT,
                    WPARAM(self.font.0 as usize),
                    LPARAM(1),
                );
            }
        }
        hwnd
    }

    fn label(&self, text: &str, x: i32, y: i32, width: i32) -> HWND {
        self.create(
            windows::core::w!("STATIC"),
            text,
            WS_CHILD.0 | WS_VISIBLE.0,
            x,
            y,
            width,
            20,
            0,
        )
    }

    fn edit(&self, text: &str, x: i32, y: i32, width: i32, id: i32) -> HWND {
        const ES_AUTOHSCROLL: u32 = 0x0080;
        self.create(
            windows::core::w!("EDIT"),
            text,
            WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL,
            x,
            y,
            width,
            24,
            id,
        )
    }

    fn button(&self, text: &str, x: i32, y: i32, width: i32, id: i32) -> HWND {
        self.create(
            windows::core::w!("BUTTON"),
            text,
            WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32,
            x,
            y,
            width,
            26,
            id,
        )
    }

    fn checkbox(&self, text: &str, x: i32, y: i32, width: i32, id: i32) -> HWND {
        self.create(
            windows::core::w!("BUTTON"),
            text,
            WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32,
            x,
            y,
            width,
            22,
            id,
        )
    }

    fn combo(&self, items: &[&str], selected: usize, x: i32, y: i32, width: i32, id: i32) -> HWND {
        let hwnd = self.create(
            windows::core::w!("COMBOBOX"),
            "",
            WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32,
            x,
            y,
            width,
            180,
            id,
        );

        if hwnd.0.is_null() {
            return hwnd;
        }

        for item in items {
            let wide = to_wide(item);
            unsafe {
                SendMessageW(hwnd, CB_ADDSTRING, WPARAM(0), LPARAM(wide.as_ptr() as isize));
            }
        }
        unsafe {
            SendMessageW(hwnd, CB_SETCURSEL, WPARAM(selected), LPARAM(0));
        }
        hwnd
    }
}

fn index_of(list: &[&str], value: &str) -> usize {
    list.iter().position(|item| *item == value).unwrap_or(0)
}

fn selected_index(hwnd: HWND) -> usize {
    let value = unsafe { SendMessageW(hwnd, CB_GETCURSEL, WPARAM(0), LPARAM(0)) };
    if value.0 < 0 {
        0
    } else {
        value.0 as usize
    }
}

fn is_checked(hwnd: HWND) -> bool {
    let value = unsafe { SendMessageW(hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)) };
    value.0 == BST_CHECKED.0 as isize
}

fn set_checked(hwnd: HWND, checked: bool) {
    let value = if checked { BST_CHECKED.0 as usize } else { 0 };
    unsafe {
        SendMessageW(hwnd, BM_SETCHECK, WPARAM(value), LPARAM(0));
    }
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
        Ok(module) => HINSTANCE(module.0),
        Err(_) => return,
    };

    let window_class = WNDCLASSW {
        lpfnWndProc: Some(settings_proc),
        hInstance: instance,
        lpszClassName: CLASS_NAME,
        hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(16 as *mut core::ffi::c_void),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&window_class);
    }

    let area = window::work_area(None);
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForSystem() };
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };

    let style = windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(
        WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_VISIBLE.0,
    );

    // 客户区按 620x372 逻辑像素设计，窗口尺寸要把标题栏和边框算进去。
    let mut frame = windows::Win32::Foundation::RECT {
        left: 0,
        top: 0,
        right: (620.0 * scale).round() as i32,
        bottom: (372.0 * scale).round() as i32,
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

    build_ui(hwnd, instance);

    unsafe {
        if let Some(icon) = crate::icon::load(16) {
            SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_SMALL as usize), LPARAM(icon.0 as isize));
        }
        if let Some(icon) = crate::icon::load(32) {
            SendMessageW(hwnd, WM_SETICON, WPARAM(ICON_BIG as usize), LPARAM(icon.0 as isize));
        }
    }

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

fn build_ui(hwnd: HWND, instance: HINSTANCE) {
    let config = crate::app::current_config();
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };

    let builder = Builder {
        parent: hwnd,
        font: unsafe { GetStockObject(DEFAULT_GUI_FONT) },
        instance,
        scale,
    };

    let mut controls: Vec<(i32, HWND)> = Vec::new();
    let label_x = 16;
    let field_x = 118;
    let mut y = 16;
    let row = 34;

    builder.label("切换热键", label_x, y + 4, 96);
    let hotkey_edit = builder.edit(&config.hotkey, field_x, y, 140, ID_HOTKEY);
    let capture = builder.button("录制", 268, y - 1, 70, ID_CAPTURE);
    controls.push((ID_HOTKEY, hotkey_edit));
    controls.push((ID_CAPTURE, capture));
    y += row;

    builder.label("游戏进程", label_x, y + 4, 96);
    let game_edit = builder.edit(&config.game.process, field_x, y, 220, ID_GAME);
    controls.push((ID_GAME, game_edit));
    y += row;

    builder.label("目标进程", label_x, y + 4, 96);
    let target_edit = builder.edit(&config.target.process, field_x, y, 220, ID_TARGET);
    let pick = builder.button("获取前台窗口（3 秒）", 350, y - 1, 170, ID_PICK);
    controls.push((ID_TARGET, target_edit));
    controls.push((ID_PICK, pick));
    y += row;

    builder.label("窗口标题含", label_x, y + 4, 96);
    let title_edit = builder.edit(&config.target.window_title, field_x, y, 220, ID_TITLE);
    controls.push((ID_TITLE, title_edit));
    y += row;

    builder.label("显示位置", label_x, y + 4, 96);
    let position = builder.combo(
        &POSITION_LABELS,
        index_of(&POSITIONS, &config.layout.position),
        field_x,
        y,
        150,
        ID_POSITION,
    );
    controls.push((ID_POSITION, position));

    builder.label("宽", 286, y + 4, 24);
    let width_edit = builder.edit(
        &config.layout.width.to_string(),
        312,
        y,
        60,
        ID_WIDTH,
    );
    controls.push((ID_WIDTH, width_edit));

    builder.label("高", 384, y + 4, 24);
    let height_edit = builder.edit(
        &config.layout.height.to_string(),
        410,
        y,
        60,
        ID_HEIGHT,
    );
    controls.push((ID_HEIGHT, height_edit));

    builder.label("边距", 480, y + 4, 34);
    let margin_edit = builder.edit(
        &config.layout.margin.to_string(),
        516,
        y,
        50,
        ID_MARGIN,
    );
    controls.push((ID_MARGIN, margin_edit));
    y += row;

    builder.label("返回时", label_x, y + 4, 96);
    let hide = builder.combo(
        &HIDE_LABELS,
        index_of(&HIDE_MODES, &config.layout.hide_mode),
        field_x,
        y,
        120,
        ID_HIDE,
    );
    controls.push((ID_HIDE, hide));

    builder.label("暂停方式", 260, y + 4, 68);
    let pause = builder.combo(
        &PAUSE_LABELS,
        index_of(&PAUSE_METHODS, &config.pause.method),
        332,
        y,
        120,
        ID_PAUSE,
    );
    controls.push((ID_PAUSE, pause));
    y += row + 6;

    let auto_return = builder.checkbox(
        "切回游戏窗口时自动返回",
        field_x,
        y,
        240,
        ID_AUTO_RETURN,
    );
    set_checked(auto_return, config.auto_return.on_game_focus);
    controls.push((ID_AUTO_RETURN, auto_return));
    y += 30;

    let autostart = builder.checkbox("开机自启", field_x, y, 240, ID_AUTOSTART);
    set_checked(autostart, config.app.start_with_windows);
    controls.push((ID_AUTOSTART, autostart));
    y += 36;

    let status = builder.label("", label_x, y, 560);
    controls.push((ID_STATUS, status));
    y += 34;

    let open_config = builder.button("打开配置文件", label_x, y, 120, ID_OPEN_CONFIG);
    let open_logs = builder.button("打开日志目录", 148, y, 120, ID_OPEN_LOGS);
    let cancel = builder.button("取消", 380, y, 90, ID_CANCEL);
    let save = builder.button("保存", 480, y, 90, ID_SAVE);
    controls.push((ID_OPEN_CONFIG, open_config));
    controls.push((ID_OPEN_LOGS, open_logs));
    controls.push((ID_CANCEL, cancel));
    controls.push((ID_SAVE, save));

    let state = Box::new(SettingsState {
        config,
        controls,
        capturing: false,
        countdown: 0,
    });

    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
    }
}

fn save(state: &SettingsState, hwnd: HWND) {
    let hotkey_text = text_of(state.control(ID_HOTKEY)).trim().to_string();
    if let Err(err) = hotkey::parse(&hotkey_text) {
        show_error(hwnd, &format!("热键无效：{err}"));
        return;
    }

    let mut config = state.config.clone();
    config.hotkey = hotkey_text;
    config.game.process = text_of(state.control(ID_GAME)).trim().to_string();
    config.target.process = text_of(state.control(ID_TARGET)).trim().to_string();
    config.target.window_title = text_of(state.control(ID_TITLE)).trim().to_string();
    config.layout.position = POSITIONS[selected_index(state.control(ID_POSITION))]
        .to_string();
    config.layout.width = text_of(state.control(ID_WIDTH))
        .trim()
        .parse()
        .unwrap_or(config.layout.width);
    config.layout.height = text_of(state.control(ID_HEIGHT))
        .trim()
        .parse()
        .unwrap_or(config.layout.height);
    config.layout.margin = text_of(state.control(ID_MARGIN))
        .trim()
        .parse()
        .unwrap_or(config.layout.margin);
    config.layout.hide_mode = HIDE_MODES[selected_index(state.control(ID_HIDE))].to_string();
    config.pause.method = PAUSE_METHODS[selected_index(state.control(ID_PAUSE))].to_string();
    config.auto_return.on_game_focus = is_checked(state.control(ID_AUTO_RETURN));
    config.app.start_with_windows = is_checked(state.control(ID_AUTOSTART));

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
    set_text(state.control(ID_HOTKEY), "按下组合键…");
    set_text(state.control(ID_STATUS), "按 Esc 取消录制");
    unsafe {
        let _ = SetFocus(hwnd);
    }
}

fn finish_capture(state: &mut SettingsState, vk: u32) {
    if vk == VK_ESCAPE.0 as u32 {
        state.capturing = false;
        set_text(state.control(ID_HOTKEY), &state.config.hotkey);
        set_text(state.control(ID_STATUS), "已取消");
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
            let spec = parts.join("+");
            state.capturing = false;
            set_text(state.control(ID_HOTKEY), &spec);
            set_text(state.control(ID_STATUS), "已录制，保存后生效");
        }
        None => set_text(state.control(ID_STATUS), "不支持的按键，请换一个"),
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
    set_text(
        state.control(ID_STATUS),
        "请切换到抖音窗口，3 秒后自动读取…",
    );
    unsafe {
        SetTimer(hwnd, TIMER_PICK, 1000, None);
    }
}

fn tick_pick(state: &mut SettingsState, hwnd: HWND) {
    state.countdown -= 1;

    if state.countdown > 0 {
        set_text(
            state.control(ID_STATUS),
            &format!("请切换到抖音窗口，{} 秒后自动读取…", state.countdown),
        );
        return;
    }

    unsafe {
        let _ = KillTimer(hwnd, TIMER_PICK);
    }

    let foreground = unsafe { GetForegroundWindow() };
    if foreground == hwnd || foreground.0.is_null() {
        set_text(state.control(ID_STATUS), "没有检测到其他窗口，请重试");
        return;
    }

    let exe = window::exe_name(foreground).unwrap_or_default();
    let title = window::title(foreground);

    if !exe.is_empty() {
        set_text(state.control(ID_TARGET), &exe);
    }
    if !title.is_empty() {
        set_text(state.control(ID_TITLE), &title);
    }

    set_text(
        state.control(ID_STATUS),
        &format!("已获取：{exe} / {title}"),
    );
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u32;

            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw == 0 {
                return LRESULT(0);
            }
            let state = &mut *(raw as *mut SettingsState);

            if code == BN_CLICKED {
                match id {
                    ID_SAVE => save(state, hwnd),
                    ID_CANCEL => {
                        let _ = DestroyWindow(hwnd);
                    }
                    ID_CAPTURE => begin_capture(state, hwnd),
                    ID_PICK => start_pick(state, hwnd),
                    ID_OPEN_CONFIG => crate::app::open_path(&crate::config::config_path()),
                    ID_OPEN_LOGS => crate::app::open_path(&crate::config::data_dir().join("logs")),
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let id = wparam.0;
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw != 0 && id == TIMER_PICK {
                let state = &mut *(raw as *mut SettingsState);
                tick_pick(state, hwnd);
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if raw != 0 {
                let state = &mut *(raw as *mut SettingsState);
                if state.capturing {
                    finish_capture(state, wparam.0 as u32);
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
                drop(Box::from_raw(raw as *mut SettingsState));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}