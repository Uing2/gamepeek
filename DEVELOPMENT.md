# GamePeek 开发手册

> 配套文档：[项目文档](./PROJECT.md)。本文只讲工程实现，产品需求、边界情况和验收标准以项目文档为准。

| 项目 | 内容 |
|---|---|
| 文档版本 | v0.1（草案） |
| 日期 | 2026-11-19 |
| 目标平台 | Windows 10 1809+ / Windows 11，x64 |
| 首选技术栈 | Rust（MSVC toolchain）+ 原生 Win32 |
| 备选技术栈 | C# / .NET 8 + WinForms |
| 首版范围 | CS2 + 抖音 Windows 桌面客户端 |

---

## 1. 技术栈与依赖

### 1.1 环境

| 项 | 要求 |
|---|---|
| Rust | 已安装 1.100.0（`stable-x86_64-pc-windows-gnu`），见 15 节 |
| 编译器 | 本机使用 rustup 自带的 GNU 工具链；如装 VS Build Tools 可切换为 MSVC |
| 调试工具 | WinDbg 或 Visual Studio、Spy++、Process Explorer、PresentMon、DebugView |
| 目标系统 | Windows 10 1809+（SMTC 需要），建议 Windows 11 |

### 1.2 依赖清单

| 用途 | 选择 | 说明 |
|---|---|---|
| Win32 / COM / WinRT | `windows` crate | 统一绑定，避免多个 FFI 库 |
| 序列化 | `serde` + `toml` | 配置文件 |
| 错误 | `thiserror`（模块内）、`anyhow`（边界） | 不要用字符串错误 |
| 日志 | `tracing` + `tracing-subscriber` | 支持结构化字段与耗时打点 |
| 托盘 | `tauri-apps/tray-icon` 或原生 `Shell_NotifyIcon` | 前者省事，后者依赖更少 |
| 图标与清单 | `embed-resource` | 编译期嵌入 `.rc` 与 manifest |
| 单实例 | 命名 Mutex（`CreateMutexW`） | 不需要额外 crate |

Cargo 配置建议：

```toml
[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

`windows` crate 的 feature 至少需要：

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_Shell",
  "Win32_System_Com",
  "Win32_System_Threading",
  "Win32_System_LibraryLoader",
  "Win32_Media_Audio",
  "Win32_Media_Audio_Endpoints",
  "Foundation",
  "Media_Control",
] }
```

不同版本的 `windows` crate 模块路径和函数签名会变，升级时以 `cargo doc` 为准。

---

## 2. 可借鉴的项目

### 2.1 总表

| 项目 | 语言 | 可借鉴内容 | 许可证与注意 |
|---|---|---|---|
| [microsoft/PowerToys](https://github.com/microsoft/PowerToys) | C++ / C# | `src/modules/AlwaysOnTop` 的置顶实现、热键注册、Game Mode 规避、窗口失效处理 | MIT，可放心参考 |
| [brucevon/bosskey](https://github.com/brucevon/bosskey) | Rust | 托盘应用结构、窗口隐藏/恢复、快捷键与配置组织 | GPL-3.0，只参考思路，不要拷代码进闭源项目 |
| [LTSlw/BossKey](https://github.com/LTSlw/BossKey) | C++ | 进程绑定、窗口隐藏/显示、开机自启 | GPL-2.0，仓库已归档 |
| [File-New-Project/EarTrumpet](https://github.com/File-New-Project/EarTrumpet) | C# | 按应用枚举 WASAPI 会话、音量与静音、设备变化处理 | 自定义许可证，使用前确认 |
| [naudio/NAudio](https://github.com/naudio/NAudio) | C# | `AudioSessionManager` / `SimpleAudioVolume` 的封装方式，适合对照 API 调用顺序 | MIT |
| [thqby/ahk2_lib Audio.ahk](https://github.com/thqby/ahk2_lib/blob/master/Audio.ahk) | AHK | 按进程静音的最小实现，适合 M0 快速验证 | 许可证以仓库为准 |
| [tauri-apps/global-hotkey](https://github.com/tauri-apps/global-hotkey) | Rust | 热键解析与 `RegisterHotKey` 封装 | Apache-2.0，可直接依赖 |
| [tauri-apps/tray-icon](https://github.com/tauri-apps/tray-icon) | Rust | 托盘图标与菜单 | Apache-2.0，可直接依赖 |
| [microsoft/windows-rs](https://github.com/microsoft/windows-rs) | Rust | COM/WinRT 调用范例、类型定义 | MIT / Apache-2.0 |
| [AMM TriggerKit](https://apps.microsoft.com/detail/9mvtj4r71z97) | 闭源 | "按指定音源暂停/压低"的产品交互与音源选择思路 | 只看行为，不看代码 |
| [OneQuick](https://onequick.org/reference/operation) | AHK / 闭源 | 多步热键、窗口操作组合的产品设计 | 旧版源码已不可用，只参考交互 |

### 2.2 按功能找参考

| 功能 | 首选参考 | 说明 |
|---|---|---|
| 全局热键 | PowerToys、global-hotkey | 统一用 `RegisterHotKey`，不要低级键盘钩子 |
| 窗口置顶/取消 | PowerToys AlwaysOnTop | 注意 Game Mode、窗口重建、置顶清理 |
| 窗口隐藏/恢复 | brucevon/bosskey、LTSlw/BossKey | 重点看窗口句柄失效后的重找逻辑 |
| 按应用静音 | EarTrumpet、NAudio、Audio.ahk | 先看 C# 封装，再用 `windows` crate 翻译 |
| 暂停媒体 | AMM TriggerKit（行为）、SMTC 文档 | 按 AUMID 选择音源，避免误伤其他播放器 |
| 托盘与自启 | brucevon/bosskey、tray-icon | 注意 Explorer 重启后重建图标 |
| 焦点切换 | PowerToys、MS 文档 | 前台锁、AttachThreadInput、超时校验 |

### 2.3 明确不要借鉴的做法

- 不要向游戏进程注入 DLL、Hook D3D/OpenGL、读写游戏内存。
- 不要装全局低级键盘钩子来拦截按键，游戏输入延迟和兼容性都可能受影响。
- 不要把 GPL 项目的代码直接复制进闭源项目。
- 不要用模拟"Alt+Tab"或反复 `ShowWindow` 的方式抢焦点，容易闪烁和丢焦点。

---

## 3. 代码结构

```text
GamePeek/
├─ Cargo.toml
├─ build.rs
├─ app.manifest
├─ app.rc
├─ assets/
│  └─ icon.ico
├─ src/
│  ├─ main.rs            入口、单实例、崩溃钩子
│  ├─ app.rs             消息循环、生命周期、退出清理
│  ├─ config.rs          配置读写与默认值
│  ├─ controller.rs      状态机与流程编排
│  ├─ hotkey.rs          RegisterHotKey
│  ├─ foreground.rs      WinEvent 前台监听与事件抑制
│  ├─ window.rs          窗口查找、置顶、显示、移动、焦点
│  ├─ audio.rs           WASAPI 会话静音与还原
│  ├─ media.rs           SMTC 暂停、空格发送
│  ├─ tray.rs            托盘图标与菜单
│  ├─ diagnostics.rs     日志、耗时打点、状态快照
│  └─ util.rs            RAII、句柄包装、时间测量
└─ tests/
   └─ config_test.rs
```

原则：

- 所有 Win32/COM 调用集中在上面的模块里，`controller` 不直接碰 FFI。
- 每个模块对外暴露安全接口，`unsafe` 只出现在模块内部。
- 热路径上的函数不分配内存、不打同步日志、不做阻塞调用。

---

## 4. 线程与生命周期

```text
主线程
  ├─ 消息循环（GetMessage）
  ├─ 热键消息 WM_HOTKEY
  ├─ WinEvent 回调（WINEVENT_OUTOFCONTEXT 投递到本线程）
  └─ 托盘消息

工作线程（MTA）
  ├─ WASAPI（CoInitializeEx COINIT_MULTITHREADED）
  ├─ SMTC（WinRT 异步 .get()）
  └─ 状态机切换（串行处理，避免并发切换）

日志线程（可选）
  └─ 无锁队列落盘
```

约定：

1. 只允许一个线程执行状态切换，其他线程通过 channel 投递事件。
2. `WinEvent` 回调只做判断和投递，不执行置顶、静音等操作。
3. SMTC 的 `IAsyncOperation::get()` 会阻塞，必须在工作线程调用。
4. 退出时先停止接收事件，再还原音频和窗口状态，最后销毁托盘图标。

---

## 5. 核心流程时序

### 5.1 进入 PEEK

```text
热键消息
  -> 读取当前前台窗口
  -> 前台不是抖音：执行进入流程
  -> 解析/校验抖音 HWND
  -> 快照 CS2 音频会话静音状态
  -> 静音 CS2 会话
  -> 移动并置顶抖音（SWP_NOACTIVATE）
  -> 设置前台窗口并校验
  -> 记录 expected_foreground = 抖音
  -> 完成
```

### 5.2 返回 GAME（热键触发）

```text
热键消息
  -> 前台是抖音：执行返回流程
  -> 校验前台仍是抖音
  -> 发送空格
  -> 校验播放状态
  -> 取消置顶、移出屏幕
  -> 还原 CS2 静音状态
  -> 聚焦 CS2 并校验
  -> 完成
```

### 5.3 返回 GAME（手动点击 CS 触发）

```text
WinEvent(EVENT_SYSTEM_FOREGROUND) 收到 CS2 HWND
  -> 判断不是自身切换（expected_foreground 检查 + 去抖）
  -> 抖音已失焦，跳过空格
  -> SMTC 暂停指定音源
  -> 取消置顶、移出屏幕
  -> 还原 CS2 静音状态
  -> CS2 已在前台，不再抢焦点
  -> 完成
```

---

## 6. 模块实现指南

### 6.1 配置（config.rs）

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub hotkey: String,
    pub game: GameConfig,
    pub target: TargetConfig,
    pub layout: LayoutConfig,
    pub pause: PauseConfig,
    pub auto_return: AutoReturnConfig,
    pub app: AppConfig,
}
```

实现要点：

- 首次启动生成默认配置，路径固定在 `%LOCALAPPDATA%\GamePeek\config.toml`。
- 读取失败时保留原文件并生成 `.bak`，不要覆盖用户配置。
- 热键字符串解析独立成函数，便于测试。
- 配置变更后只需要重新注册热键和刷新布局，不必重启进程。

参考：brucevon/bosskey 的配置结构；PowerToys 的设置持久化方式。

### 6.2 热键（hotkey.rs）

使用独立的消息专用窗口接收 `WM_HOTKEY`，不要占用托盘窗口的消息处理逻辑。

```rust
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT, VK_F8,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
};

const HOTKEY_ID: i32 = 1;

pub unsafe fn run_loop(hwnd: windows::Win32::Foundation::HWND) -> anyhow::Result<()> {
    RegisterHotKey(hwnd, HOTKEY_ID, MOD_NOREPEAT, VK_F8.0 as u32)?;
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        if msg.message == WM_HOTKEY && msg.wParam.0 as i32 == HOTKEY_ID {
            // 只投递事件，不在消息循环里做重活
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
    Ok(())
}
```

要点：

- `MOD_NOREPEAT` 防止长按连续触发。
- 注册失败说明键位被占用，托盘提示并允许改键。
- 热键回调里只发送 channel 消息，实际切换在工作线程完成。
- 如果改用 `global-hotkey` crate，注意它需要配合事件循环使用，不能和裸 `GetMessage` 随意混用。

参考：PowerToys 的热键注册与冲突处理；global-hotkey 的按键解析。

### 6.3 前台监听（foreground.rs）

```rust
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT,
};

unsafe extern "system" fn on_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: windows::Win32::Foundation::HWND,
    _id_object: i32,
    _id_child: i32,
    _id_thread: u32,
    _time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        // 只投递 hwnd，交给 controller 判断
    }
}

pub unsafe fn install() -> anyhow::Result<HWINEVENTHOOK> {
    let hook = SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND,
        EVENT_SYSTEM_FOREGROUND,
        None,
        Some(on_win_event),
        0,
        0,
        WINEVENT_OUTOFCONTEXT,
    );
    if hook.is_invalid() {
        anyhow::bail!("SetWinEventHook failed");
    }
    Ok(hook)
}
```

事件抑制：

```rust
struct ForegroundGuard {
    expected: Option<HWND>,
    suppress_until: Instant,
}

fn should_handle(&mut self, hwnd: HWND) -> bool {
    if Some(hwnd) == self.expected {
        self.expected = None;
        return false;
    }
    if Instant::now() < self.suppress_until {
        return false;
    }
    true
}
```

要点：

- 进入 PEEK 前设置 `expected = Some(douyin_hwnd)`，避免自己的 `SetForegroundWindow` 触发返回。
- 事件可能连续到达多次，加 100–200 ms 去抖。
- 只处理顶层窗口，忽略子窗口和工具窗口。

参考：PowerToys 对窗口事件的处理；MS 文档 `SetWinEventHook`。

### 6.4 窗口（window.rs）

**查找窗口**：先缓存 HWND，失效时按优先级重找：

1. 进程可执行文件路径包含 `douyin`（用 `QueryFullProcessImageNameW`）。
2. 窗口类名匹配（用 Spy++ 查）。
3. 窗口标题包含"抖音"。
4. 排除不可见窗口、工具窗口、0 尺寸窗口。

**置顶与显示**：

```rust
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SW_SHOWNOACTIVATE,
};

unsafe fn show_topmost(hwnd: HWND, rect: RECT) -> anyhow::Result<()> {
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    )?;
    Ok(())
}
```

**聚焦**：

```rust
use windows::Win32::UI::WindowsAndMessaging::{
    AttachThreadInput, GetForegroundWindow, GetWindowThreadProcessId,
    SetForegroundWindow,
};

unsafe fn force_foreground(hwnd: HWND) {
    let fg = GetForegroundWindow();
    if fg == hwnd {
        return;
    }
    let fg_thread = GetWindowThreadProcessId(fg, None);
    let target_thread = GetWindowThreadProcessId(hwnd, None);
    let _ = AttachThreadInput(fg_thread, target_thread, true.into());
    let _ = SetForegroundWindow(hwnd);
    let _ = AttachThreadInput(fg_thread, target_thread, false.into());
}
```

要点：

- `AttachThreadInput` 有死锁风险，调用前后都要校验线程是否响应；失败时退回 `SwitchToThisWindow` 或提示用户。
- 聚焦后必须用 `GetForegroundWindow()` 校验，不能假设成功。
- 窗口移出屏幕时保留一个像素可见或使用 `SWP_NOACTIVATE`，避免某些应用判定为最小化而暂停。
- 不要每次切换都改尺寸，尺寸变化会重建交换链，造成黑闪。

参考：PowerToys AlwaysOnTop 的窗口重建处理；LTSlw/BossKey 的隐藏与恢复。

### 6.5 音频（audio.rs）

**初始化**：工作线程 `CoInitializeEx(COINIT_MULTITHREADED)`，只创建一次 `IMMDeviceEnumerator`。

**枚举会话并静音**：

```rust
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

unsafe fn mute_process(pid: u32, mute: bool) -> anyhow::Result<()> {
    let enumerator: IMMDeviceEnumerator =
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
    let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
    let sessions = manager.GetSessionEnumerator()?;

    for i in 0..sessions.GetCount()? {
        let control = sessions.GetSession(i)?;
        let control2: IAudioSessionControl2 = control.cast()?;
        if control2.GetProcessId()? != pid {
            continue;
        }
        let volume: ISimpleAudioVolume = control.cast()?;
        volume.SetMute(mute.into(), std::ptr::null())?;
    }
    Ok(())
}
```

模块路径以 `windows` crate 版本为准，例如 `IMMDeviceEnumerator` 可能在 `Win32::Media::Audio::Endpoints` 下，`SetMute` 的参数类型也可能不同，实现时以 `cargo doc` 为准。

**快照与还原**：

```rust
#[derive(Clone, Copy)]
struct MuteSnapshot {
    pid: u32,
    session_id: String,
    was_muted: bool,
}
```

要点：

- 进入 PEEK 前逐个记录 `GetMute()`，返回时只还原本工具改过的会话。
- 会话可能在切换期间重建，`ISimpleAudioVolume` 指针会失效；捕获失败时重新枚举。
- 注册 `IMMNotificationClient` 监听默认设备变化，换耳机后重新解析。
- 如果 CS2 有多个音频会话，全部处理。
- 系统独占模式或会话不可控时，记录日志并降级为提示，不要假装成功。

参考：EarTrumpet 的会话管理；NAudio 的 `AudioSessionManager`；Audio.ahk 的最小实现。

### 6.6 暂停（media.rs）

**优先级**：

1. 抖音在前台：发空格（`SendInput`），发送前校验前台。
2. 抖音已失焦：SMTC 按音源暂停。
3. SMTC 不可用：向抖音窗口 `PostMessage(WM_KEYDOWN/WM_KEYUP)`。
4. 全部失败：收起窗口并在托盘提示，不让抖音继续出声。

**空格发送**：

```rust
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_SPACE,
};

unsafe fn send_space() {
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
    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}
```

调用前必须：

```rust
if GetForegroundWindow() != douyin_hwnd {
    // 绝不发送，否则空格会打到 CS2
}
```

**SMTC 暂停**：

```rust
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

fn pause_by_aumid(keyword: &str) -> anyhow::Result<bool> {
    let manager =
        GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.get()?;
    for session in manager.GetSessions()? {
        let aumid = session.SourceAppUserModelId()?.to_string();
        if aumid.to_lowercase().contains(keyword) {
            session.TryPauseAsync()?.get()?;
            return Ok(true);
        }
    }
    Ok(false)
}
```

要点：

- `RequestAsync().get()` 会阻塞，放在工作线程。
- 用 AUMID 或显示名精确匹配抖音，避免暂停浏览器里的其他视频。
- 暂停后读取 `GetPlaybackInfo().PlaybackStatus()` 校验结果。
- 抖音是否注册 SMTC 必须先验证；如果不注册，这条路径直接不可用。

参考：AMM TriggerKit 的音源选择交互；MS 文档 SMTC。

### 6.7 状态机（controller.rs）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Game,
    Peek,
    Switching,
}

enum Event {
    Hotkey,
    ForegroundChanged(HWND),
    TargetLost,
    Exit,
}
```

规则：

- 任何时刻只有一个 `Phase`，`Switching` 期间拒绝新事件，避免连按导致状态错乱。
- 每次切换生成一个 `Transition` 对象，记录每一步的耗时与结果，失败时按逆序回滚。
- 状态判定以 `GetForegroundWindow()` 为准，`Phase` 只用于防重入。
- 用户手动 Alt-Tab 后，下一次热键根据实际前台决定方向。
- 抖音被关闭时立即回滚音频并回到 `Game`。

参考：PowerToys 模块的窗口状态管理；LTSlw/BossKey 的显示状态保存。

### 6.8 托盘与单实例（tray.rs）

- 托盘菜单至少包含：切换、暂停/恢复抖音、打开配置、查看日志、退出。
- 监听 `TaskbarCreated` 消息，Explorer 重启后重新添加图标。
- 单实例用命名 Mutex：`Global\GamePeek_SingleInstance`，已存在则激活现有窗口并退出。
- 退出路径统一走 `shutdown()`：注销热键、卸载 hook、还原音频、取消置顶、销毁托盘。

参考：brucevon/bosskey 的托盘结构；tray-icon 的菜单 API。

### 6.9 诊断（diagnostics.rs）

每次切换记录一条结构化事件：

```text
ts=... event=switch_start trigger=hotkey
ts=... event=resolve_target ok=true hwnd=0x... cost_us=120
ts=... event=mute_game ok=true sessions=1 cost_us=340
ts=... event=show_target ok=true cost_us=810
ts=... event=focus_target ok=true cost_us=1500
ts=... event=switch_end total_us=3200 result=ok
```

要求：

- 热路径只写内存环形缓冲，落盘在后台线程。
- 日志默认保留最近 7 天，避免无限增长。
- 诊断面板显示：当前 Phase、前台 HWND、抖音 HWND、CS2 PID、音频会话数、最近一次切换耗时。
- 发布版保留最小日志，方便用户反馈"偶尔不灵"。

---

## 7. 性能实现规范

1. **热路径零查找**：热键回调里不枚举进程、不枚举音频会话、不查注册表。
2. **预热**：启动时解析一次；进程出现时再刷新。可用低频定时器（2–5 s）兜底，但只在未命中时启用。
3. **缓存失效**：窗口销毁、会话断开、设备切换时置空缓存，下一次使用前重建。
4. **不做同步 IO**：配置只在启动和修改时读写，日志异步落盘。
5. **减少窗口操作**：进入 PEEK 只做一次 `SetWindowPos`；返回只做一次取消置顶和移动。
6. **避免尺寸变化**：预设尺寸固定，用户改尺寸时在窗口不可见时完成。
7. **不要用轮询抢焦点**：焦点切换失败时最多重试一次，不要循环 `SetForegroundWindow`。
8. **测量**：用 `QueryPerformanceCounter` 包裹每一步，P95 超过预算就优化。

---

## 8. 稳定性与异常处理

| 场景 | 处理 |
|---|---|
| 抖音未启动 | 托盘提示；如果配置了自动启动，则启动后等待窗口出现（带超时） |
| 抖音窗口重建 | 监听窗口销毁事件，下一次切换前重新解析 |
| 抖音被关闭 | 立即还原 CS2 音频，回到 Game |
| 窗口句柄失效 | 所有操作前校验 `IsWindow`，失败则重新解析 |
| 音频会话失效 | 捕获 `E_NOTFOUND` 等错误后重新枚举一次 |
| 默认设备切换 | `IMMNotificationClient` 回调中标记缓存失效 |
| 屏幕锁定/用户切换 | 监听会话事件，退出前还原 |
| 系统关机 | 处理 `WM_QUERYENDSESSION`，还原后允许退出 |
| 工具崩溃 | `panic` hook 里同步还原音频、取消置顶，写崩溃日志 |
| 进程被强杀 | 启动时读取 `state.json` 标记；若上次留下静音标记，按配置还原 |
| 权限不匹配 | 操作返回 `ACCESS_DENIED` 时提示以管理员重启，不静默失败 |
| Explorer 重启 | 监听 `TaskbarCreated`，重建托盘图标 |
| 虚拟桌面 | 用 `IVirtualDesktopManager` 判断，必要时移动窗口或提示 |
| 多显示器/DPI | 保存 RECT 与显示器标识，按 `PerMonitorV2` 换算 |

`state.json` 示例：

```json
{
  "muted_by_us": true,
  "game_pid": 12345,
  "sessions": ["{0.0.0.00000000}.{...}|12345"],
  "updated_at": "2026-11-19T12:00:00+08:00"
}
```

原则：**任何退出路径都要可重复还原**，还原操作必须幂等。

---

## 9. 测试方案

### 9.1 M0 可行性验证

在写正式代码前，用最小脚本或临时程序验证：

| 编号 | 验证项 | 步骤 | 通过标准 |
|---|---|---|---|
| V1 | 焦点切换 | CS2 无边框，按热键 20 次切到抖音再切回 | 成功率 100%，无明显卡顿 |
| V2 | 前台空格 | 抖音前台发空格 | 视频暂停，无误触其他控件 |
| V3 | 失焦空格 | 抖音失焦后 `PostMessage` 空格 | 能暂停，或确认不可用 |
| V4 | SMTC 暂停 | 用探针列出媒体会话并暂停抖音 | 能找到抖音会话且暂停成功 |
| V5 | 恢复显示 | 从屏幕外/最小化恢复各 20 次 | 无黑帧、无尺寸跳变 |
| V6 | 按进程静音 | 静音 CS2 后确认游戏无声、抖音有声 | 即时生效，可还原 |
| V7 | 独占全屏 | CS2 独占全屏时尝试显示抖音 | 确认无法置顶，记录提示文案 |
| V8 | 反作弊 | 在离线/休闲模式运行完整流程 | 无异常，确认无注入行为 |

SMTC 探针（独立小程序，不影响主项目）：

```csharp
var mgr = await Windows.Media.Control
    .GlobalSystemMediaTransportControlsSessionManager.RequestAsync();
foreach (var s in mgr.GetSessions())
{
    var info = s.GetPlaybackInfo();
    Console.WriteLine($"{s.SourceAppUserModelId} | {info.PlaybackStatus}");
}
```

### 9.2 回归矩阵

| 场景 | 操作 | 预期 |
|---|---|---|
| 正常往返 | 热键进入、热键返回，重复 100 次 | 无状态残留、无位置漂移 |
| 手动点击 | PEEK 态点击 CS 可见区域 | 抖音暂停、收起，游戏声音恢复 |
| 手动 Alt-Tab | PEEK 态 Alt-Tab 到浏览器 | 默认不暂停，抖音仍置顶 |
| 连按热键 | 200 ms 内连按 5 次 | 不出现多次切换或半完成状态 |
| 抖音关闭 | PEEK 态结束抖音进程 | 游戏声音立即恢复 |
| 设备切换 | PEEK 态换耳机 | 音频控制仍然有效 |
| 显示器变化 | 切换主屏/拔掉副屏 | 抖音位置正确，不越界 |
| 权限变化 | 用管理员启动 CS2 | 工具提示权限不匹配 |
| 崩溃恢复 | 强杀工具后重启 | 能识别并还原遗留静音 |
| 资源管理器重启 | 重启 explorer.exe | 托盘图标恢复 |

### 9.3 性能测试

- 用 `PresentMon` 或 CapFrameX 记录 CS2 在切换前后的帧时间，观察是否有长帧。
- 连续切换 100 次，统计 P50/P95 总耗时与各步骤耗时。
- 空闲 10 分钟，记录 CPU 占用与内存曲线，确认没有定时器抖动或日志堆积。

---

## 10. 调试工具箱

| 工具 | 用途 |
|---|---|
| Spy++ | 查看窗口类名、层级、样式，定位抖音主窗口 |
| Window Spy（AHK 自带） | 快速查看窗口标题、类名、进程 |
| Process Explorer | 查看进程路径、完整性级别、句柄 |
| Windows 音量合成器 | 确认 CS2/抖音的音频会话与静音状态 |
| PresentMon / CapFrameX | 观察切换时的帧时间与掉帧 |
| DebugView | 捕获 `OutputDebugStringW` 输出 |
| WinDbg / Visual Studio | 崩溃转储分析 |
| Windows 设置 > 系统 > 声音 | 验证默认设备与音量 |
| Windows 11 媒体浮层 | 确认抖音是否注册 SMTC |

常用检查命令：

```powershell
# 查看进程路径与命令行
Get-CimInstance Win32_Process -Filter "Name='cs2.exe'" |
  Select-Object ProcessId, ExecutablePath, CommandLine

# 查看抖音进程
Get-Process | Where-Object { $_.ProcessName -like '*douyin*' } |
  Select-Object Id, ProcessName, Path
```

---

## 11. 打包与发布

### 11.1 清单文件

`app.manifest` 至少包含：

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
```

### 11.2 构建产物

- 单文件 exe，静态 CRT，不依赖 VC++ 运行库。
- 图标、版本信息、产品名通过 `.rc` 嵌入。
- 默认 `asInvoker`，不要无条件要求管理员权限。
- 需要提权时，由用户在托盘菜单主动选择"以管理员重启"，而不是启动就弹 UAC。

### 11.3 自启动

- 普通权限：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`。
- 需要提权：任务计划程序"登录时"触发，勾选"使用最高权限运行"。
- 提供关闭自启动的开关，卸载时清理注册表项和计划任务。

### 11.4 发布检查

- 新系统首次运行无缺失 DLL。
- SmartScreen 提示可接受，必要时做代码签名。
- 配置文件、日志目录可写，路径不包含中文兼容问题。
- 卸载后不残留自启动项。

---

## 12. 开发规范与常见坑

### 12.1 规范

- `unsafe` 只在 FFI 模块内出现，并写清楚前置条件。
- 所有 Win32 返回值都检查，不忽略 `Result`。
- 热路径禁止 `unwrap()`、`println!`、堆分配。
- 状态切换失败必须回滚，不能留下"游戏静音但抖音没显示"的中间态。
- 新增边界情况时，同时补回归矩阵和验收标准。

### 12.2 常见坑

| 现象 | 原因 | 处理 |
|---|---|---|
| 热键按了没反应 | 键位被占用、工具被节流、权限不匹配 | 检查注册返回值、关闭 EcoQoS、提示提权 |
| 抖音显示不出来 | CS2 独占全屏、虚拟桌面、窗口被最小化 | 检测并提示，切换窗口显示方式 |
| 按一次切两次 | 自身焦点事件未抑制、事件重复 | 增加期望窗口校验与去抖 |
| 抖音不暂停 | 已失焦仍发空格、SMTC 无会话 | 改为按音源暂停，记录失败原因 |
| CS 角色突然跳 | 空格误发到游戏 | 发送前强制校验前台窗口 |
| 切回游戏后没声音 | 音频会话重建、还原失败 | 重新枚举会话并重试还原 |
| 抖音黑闪 | 从最小化恢复、尺寸变化 | 改为移出屏幕、固定尺寸 |
| 切换卡顿 | 每次都枚举窗口/会话 | 缓存句柄、异步刷新 |
| 托盘图标消失 | Explorer 重启 | 监听 `TaskbarCreated` 重建 |
| 日志里全是超时 | 前台锁未处理 | 使用 AttachThreadInput 或 SwitchToThisWindow |

---

## 13. 实施顺序

1. 骨架：单实例、托盘、配置、日志、消息循环。
2. 窗口：查找抖音/CS2、置顶、移动、聚焦、校验。
3. 音频：WASAPI 会话枚举、静音、快照与还原。
4. 热键闭环：进入 PEEK、热键返回。
5. 手动返回：前台监听、事件抑制、SMTC 暂停。
6. 稳定性：异常还原、设备变化、Explorer 重启、权限提示。
7. 体验：设置界面、位置记忆、多显示器/DPI、自启动、打包。
8. 验证：跑完 M0 清单和回归矩阵，再考虑通用化。

每一步都以"可回滚、可观测"为前提，不要在没有日志和还原逻辑的情况下继续加功能。

---

## 14. 构建与运行

### 14.1 工具链

本项目不需要 Visual Studio Build Tools：rustup 自带的 GNU 工具链含链接器，普通用户权限即可编译。

```powershell
rustup default stable-x86_64-pc-windows-gnu
```

如果已经装了 VS Build Tools，也可以切到官方主推的 MSVC 目标：

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc
```

功能与 API 调用不受影响，两者编译产物都是 Windows 可执行文件。

### 14.2 构建

MinGW 的链接器无法处理包含中文的路径，因此项目路径含中文时，必须把输出目录指向纯 ASCII 路径：

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\gamepeek-target"

cargo check          # 类型检查
cargo test           # 单元测试
cargo build --release
```

产物：`%LOCALAPPDATA%\gamepeek-target\release\gamepeek.exe`（约 750 KB）。

### 14.3 运行

直接双击 `gamepeek.exe`，托盘会出现图标：

- 左键或 `F8`：切换游戏 / 抖音
- 右键：切换、打开配置、打开日志目录、退出

运行时的文件都在 `%LOCALAPPDATA%\GamePeek`：

| 文件 | 说明 |
|---|---|
| `config.toml` | 配置，首次运行自动生成 |
| `logs\gamepeek.log` | 运行日志，每次切换都会记录耗时 |
| `state.toml` | 静音状态标记，异常退出后用于还原 |

首版默认**不写开机自启**（`app.start_with_windows = false`），需要时在配置里改为 `true` 并重启程序。

### 14.4 当前实现状态

已完成（M1 + M2 主体）：

- 单实例、托盘、配置读写、结构化日志
- 全局热键注册、前台事件监听
- 抖音窗口定位、置顶、移动、焦点切换与校验
- 按进程静音（WASAPI）与快照还原，还原失败自动重试
- 暂停三层降级：媒体会话优先 → 前台空格 → 投递按键消息，并校验播放状态
- 点击游戏自动返回
- 目标窗口被关闭后自动结束 PEEK 并恢复游戏音频
- 崩溃后遗留静音状态的启动还原
- 位置记忆（`layout.position = "remember"` 会落盘到 `layout.toml`）
- 设置界面：热键录制、进程与标题、布局与尺寸、隐藏与暂停方式、自动返回、开机自启，保存后热键重注册并热加载配置
- 关闭 Win11 后台节能节流、DPI 感知、资源管理器重启后重建托盘

尚未实现：

- `IMMNotificationClient` 显式设备回调（当前每次切换重新解析设备与会话，功能已覆盖，只是少了事件驱动）
- 抖音多窗口选择（主窗 / 直播 / 小窗）
- 安装包与代码签名

## 15. CI 与代码签名（SignPath）

### 15.1 工作流

| 文件 | 触发 | 作用 |
|---|---|---|
| `.github/workflows/ci.yml` | push / PR 到 main | `cargo check` + `cargo test` + 构建，上传未签名产物 |
| `.github/workflows/release.yml` | 打 `v*` tag | 构建 → 提交 SignPath 签名 → 生成 SHA256 → 发布 Release |

未配置签名密钥时，release 工作流会自动跳过签名步骤，仍然产出未签名 exe 和 `checksums.txt`，不会让流水线失败。

### 15.2 为什么必须用 CI 签名

签名私钥托管在 SignPath 的 HSM 中，不落到本地和仓库里。SignPath 只接受由 CI 构建出的产物，从而保证被签名的二进制确实来自仓库源码。工作流里先 `upload-artifact` 拿到 artifact id，再把它提交给签名服务，签好的文件写回 `signed/` 目录。

### 15.3 启用步骤

1. 仓库转为公开，确认根目录有 OSI 认可的许可证（本项目为 MIT）。
2. 把仓库镜像到 GitHub（SignPath 不接受 Gitee 作为构建源）：
   - Gitee 仓库 → 管理 → 仓库镜像管理 → 推送镜像，填 GitHub 仓库地址和 token；或本地 `git remote add github ... && git push github main --tags`。
3. 在 SignPath 申请开源计划（Foundation），提交仓库地址、许可证和构建说明，等待人工审核。
4. 审核通过后，在 SignPath 控制台创建 project 与 signing policy，记下：
   - organization id
   - project slug（默认 `gamepeek`）
   - signing policy slug（例如 `release-signing`）
   - 一个 API token
5. 在 GitHub 仓库配置：
   - Secrets：`SIGNPATH_API_TOKEN`、`SIGNPATH_ORGANIZATION_ID`
   - Variables：`SIGNPATH_PROJECT_SLUG`、`SIGNPATH_POLICY_SLUG`
6. 推送一个 tag 触发发布，检查 Release 里的 exe 是否带签名（`signtool verify /pa gamepeek.exe`）。

### 15.4 注意

- CI 使用 MSVC 工具链（GitHub 的 windows-latest 自带），与本地 GNU 工具链的产物功能一致，体积略有差异。
- SignPath 的免费计划要求项目持续开源且构建可复现；如果后续改成闭源，需要改用付费方案或自购证书。
- 自签名证书对普通用户无效，SmartScreen 仍会拦截，不要用它冒充正式签名。

## 16. 参考资料

- [PowerToys Always on Top](https://learn.microsoft.com/zh-cn/windows/powertoys/always-on-top)
- [PowerToys 源码](https://github.com/microsoft/PowerToys)
- [brucevon/bosskey](https://github.com/brucevon/bosskey)
- [LTSlw/BossKey](https://github.com/LTSlw/BossKey)
- [EarTrumpet](https://github.com/File-New-Project/EarTrumpet)
- [NAudio](https://github.com/naudio/NAudio)
- [Audio.ahk](https://github.com/thqby/ahk2_lib/blob/master/Audio.ahk)
- [global-hotkey](https://github.com/tauri-apps/global-hotkey)
- [tray-icon](https://github.com/tauri-apps/tray-icon)
- [windows-rs](https://github.com/microsoft/windows-rs)
- [RegisterHotKey](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-registerhotkey)
- [SetWinEventHook](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwineventhook)
- [SetWindowPos](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowpos)
- [SetForegroundWindow](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setforegroundwindow)
- [AttachThreadInput](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-attachthreadinput)
- [IAudioSessionManager2](https://learn.microsoft.com/windows/win32/api/audiosessiontypes/nn-audiosessiontypes-iaudiosessionmanager2)
- [ISimpleAudioVolume](https://learn.microsoft.com/windows/win32/api/endpointvolume/nn-endpointvolume-isimpleaudiovolume)
- [IMMNotificationClient](https://learn.microsoft.com/windows/win32/api/mmdeviceapi/nn-mmdeviceapi-immnotificationclient)
- [SetProcessInformation](https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation)
- [GlobalSystemMediaTransportControlsSessionManager](https://learn.microsoft.com/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager)
- [IVirtualDesktopManager](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nn-shobjidl_core-ivirtualdesktopmanager)
