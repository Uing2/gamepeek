use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, RECT};

use crate::audio::{AudioManager, MuteSnapshot};
use crate::config::Config;
use crate::media;
use crate::util::{hwnd_from_i64, Stopwatch};
use crate::window::{self, WindowQuery};

/// 主线程与工作线程之间传递的事件。HWND 用整数表示以满足 Send。
pub enum Event {
    Hotkey,
    Foreground(i64),
    /// 设置界面保存后热加载，不需要重启
    Reload(Box<Config>),
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Game,
    Peek,
}

/// 触发来源决定暂停方式：热键时抖音通常在前台，可以发空格；
/// 手动点击游戏时抖音已经失焦，发空格会打到游戏上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trigger {
    Hotkey,
    Manual,
}

pub struct Controller {
    config: Config,
    audio: AudioManager,
    game_window: Option<HWND>,
    game_pid: Option<u32>,
    target_window: Option<HWND>,
    target_pid: Option<u32>,
    /// 抖音被挪到屏幕外之前的可见矩形
    target_restore: Option<RECT>,
    /// 上一次 PEEK 时的窗口矩形，供 position = "remember" 使用
    last_peek_rect: Option<RECT>,
    snapshot: Vec<MuteSnapshot>,
    expected_foreground: Option<HWND>,
    suppress_until: Instant,
    phase: Phase,
}

impl Controller {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let audio = AudioManager::new()?;
        Ok(Self {
            config,
            audio,
            game_window: None,
            game_pid: None,
            target_window: None,
            target_pid: None,
            target_restore: None,
            last_peek_rect: crate::state::load_layout(),
            snapshot: Vec::new(),
            expected_foreground: None,
            suppress_until: Instant::now(),
            phase: Phase::Game,
        })
    }

    pub fn run(mut self, receiver: Receiver<Event>) {
        crate::logline!("工作线程就绪");

        if self.config.app.restore_audio_on_exit {
            if let Err(err) = crate::state::restore_pending(&self.audio) {
                crate::logline!("还原上次遗留音频状态失败: {err:#}");
            }
        } else {
            crate::state::clear();
        }

        loop {
            match receiver.recv_timeout(Duration::from_millis(1000)) {
                Ok(Event::Exit) => {
                    self.shutdown();
                    break;
                }
                Ok(other) => self.handle(other),
                Err(RecvTimeoutError::Timeout) => self.tick(),
                Err(RecvTimeoutError::Disconnected) => {
                    self.shutdown();
                    break;
                }
            }
        }
        crate::app::request_exit();
    }

    /// 空闲时兜底检查：目标窗口被关掉时不能一直让游戏保持静音。
    fn tick(&mut self) {
        if self.phase != Phase::Peek {
            return;
        }

        let target_alive = self
            .target_window
            .map(window::is_valid)
            .unwrap_or(false);

        if !target_alive {
            crate::logline!("目标窗口已关闭或失效，自动结束 PEEK");
            self.target_window = None;
            self.target_pid = None;
            self.leave_peek(Trigger::Manual);
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Hotkey => self.on_hotkey(),
            Event::Foreground(raw) => self.on_foreground(hwnd_from_i64(raw)),
            Event::Reload(config) => self.on_reload(*config),
            Event::Exit => {}
        }
    }

    fn on_reload(&mut self, config: Config) {
        let target_changed = config.target.process != self.config.target.process
            || config.target.window_class != self.config.target.window_class
            || config.target.window_title != self.config.target.window_title;
        let game_changed = config.game.process != self.config.game.process;

        self.config = config;

        if target_changed {
            self.target_window = None;
            self.target_pid = None;
        }
        if game_changed {
            self.game_window = None;
            self.game_pid = None;
        }

        crate::logline!("配置已热加载");
    }

    fn on_hotkey(&mut self) {
        let foreground = window::foreground();
        let branch = if self.phase == Phase::Peek || self.is_target(foreground) {
            "leave"
        } else {
            "enter"
        };
        crate::logline!(
            "热键触发 分支={} phase={:?} 前台pid={}",
            branch,
            self.phase,
            window::process_id(foreground)
        );

        if branch == "leave" {
            self.leave_peek(Trigger::Hotkey);
        } else {
            self.enter_peek();
        }
    }

    fn on_foreground(&mut self, hwnd: HWND) {
        if self.phase != Phase::Peek {
            return;
        }

        // 工具自身切换焦点时产生的第一个事件，直接吞掉。
        if let Some(expected) = self.expected_foreground {
            self.expected_foreground = None;
            if hwnd == expected {
                return;
            }
        }

        if Instant::now() < self.suppress_until {
            return;
        }

        if !self.config.auto_return.on_game_focus {
            return;
        }

        let is_game = self.is_game(hwnd);
        let is_target = self.is_target(hwnd);
        if is_target {
            return;
        }

        let should_return = if self.config.auto_return.scope == "any" {
            true
        } else {
            is_game
        };

        if should_return {
            crate::logline!("检测到前台切换，自动返回游戏");
            self.leave_peek(Trigger::Manual);
        }
    }

    fn enter_peek(&mut self) {
        let stopwatch = Stopwatch::new();

        let target = match self.resolve_target() {
            Some(hwnd) => hwnd,
            None => {
                crate::logline!(
                    "未找到目标窗口（process={}，title={}），取消切换",
                    self.config.target.process,
                    self.config.target.window_title
                );
                return;
            }
        };

        self.resolve_game();
        window::restore_if_needed(target);

        let area = window::work_area(self.game_window);
        let current = window::window_rect(target);
        // 只有可见的矩形才当作"原始位置"，否则会把上次移出屏幕的位置记成正常位置。
        let visible_rect = current.filter(|rect| rect.right > area.left && rect.left < area.right);
        if let Some(rect) = visible_rect {
            self.target_restore = Some(rect);
        }
        let peek_rect = self.compute_peek_rect(visible_rect.or(self.target_restore));
        self.last_peek_rect = Some(peek_rect);
        if self.config.layout.position == "remember" {
            crate::state::save_layout(&peek_rect);
        }

        if self.config.game.mute_on_peek && self.snapshot.is_empty() {
            if let Some(pid) = self.game_pid {
                match self.audio.snapshot_and_mute(pid) {
                    Ok(snapshots) => {
                        crate::logline!("已静音游戏 pid={} 会话数={}", pid, snapshots.len());
                        crate::state::save(&snapshots);
                        self.snapshot = snapshots;
                    }
                    Err(err) => crate::logline!("静音游戏失败: {err:#}"),
                }
            } else {
                crate::logline!("未找到游戏进程，跳过静音");
            }
        }

        if let Err(err) = window::place_topmost(target, &peek_rect) {
            crate::logline!("置顶目标窗口失败: {err:#}");
        }

        self.expected_foreground = Some(target);
        self.suppress_until = Instant::now() + Duration::from_millis(400);
        let focused = window::force_foreground(target);
        if !focused {
            crate::logline!("目标窗口未能获得前台，下次按热键可能需按两次");
        }

        self.phase = Phase::Peek;
        crate::logline!("进入 PEEK 完成，耗时 {:.1}ms", stopwatch.elapsed_ms());
    }

    fn leave_peek(&mut self, trigger: Trigger) {
        let stopwatch = Stopwatch::new();
        crate::logline!("开始返回 GAME（触发={:?}）", trigger);

        // 清除上一次进入时可能残留的抑制状态，避免吞掉真实的前台事件。
        self.expected_foreground = None;
        self.suppress_until = Instant::now();

        if let Some(target) = self.resolve_target() {
            let paused = self.pause_target(target, trigger);
            if !paused {
                crate::logline!("暂停目标窗口失败，已记录以便排查");
            }
            self.hide_target(target);
        }

        self.restore_audio();

        if let Some(game) = self.game_window {
            if window::is_valid(game) && window::foreground() != game {
                if !window::force_foreground(game) {
                    crate::logline!("未能把焦点切回游戏");
                }
            }
        }

        self.phase = Phase::Game;
        crate::logline!(
            "进入 GAME 完成（触发={:?}），耗时 {:.1}ms",
            trigger,
            stopwatch.elapsed_ms()
        );
    }

    fn pause_target(&self, target: HWND, trigger: Trigger) -> bool {
        let method = self.config.pause.method.as_str();
        if method == "none" {
            return false;
        }

        let foreground = window::foreground();

        // 热键返回时目标仍在前台，空格是最快的路径，先做它，避免 COM 调用拖慢返回。
        if method != "smtc" && trigger == Trigger::Hotkey && foreground == target {
            if media::send_space() {
                return true;
            }
        }

        // 失焦或空格失败时再走媒体会话，必须带超时。
        if method != "space" {
            let timeout = Duration::from_millis(self.config.pause.verify_timeout_ms.max(500));
            if let Some(paused) = media::try_pause_smtc_with_timeout(&self.config.target.process, timeout) {
                return paused;
            }
        }

        crate::logline!("媒体会话不可用，向目标窗口投递空格");
        media::post_space(target)
    }

    fn hide_target(&mut self, target: HWND) {
        let _ = window::drop_topmost(target);

        // 最大化窗口无法用 SetWindowPos 移动，先还原。
        window::restore_if_needed(target);

        if self.config.layout.hide_mode == "minimize" {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                    target,
                    windows::Win32::UI::WindowsAndMessaging::SW_MINIMIZE,
                );
            }
            return;
        }

        let restore = self
            .target_restore
            .or_else(|| window::window_rect(target))
            .unwrap_or(RECT {
                left: 0,
                top: 0,
                right: 480,
                bottom: 270,
            });

        let area = window::work_area(self.game_window);
        let width = restore.right - restore.left;
        let hidden = RECT {
            left: area.left - width - 16,
            top: restore.top,
            right: area.left - 16,
            bottom: restore.bottom,
        };

        if let Err(err) = window::place_passive(target, &hidden) {
            crate::logline!("移出屏幕失败: {err:#}");
        }
        // 部分应用（例如抖音）会在激活变化后自行重新置顶，移动完成后再清一次。
        let _ = window::drop_topmost(target);
        let rect = window::window_rect(target);
        crate::logline!("目标窗口已隐藏 rect={:?}", rect);
        self.target_restore = Some(restore);
    }

    fn restore_audio(&mut self) {
        if self.snapshot.is_empty() {
            crate::state::clear();
            return;
        }
        self.audio.restore(&self.snapshot);
        crate::logline!("已还原游戏音频（{} 个会话）", self.snapshot.len());
        self.snapshot.clear();
        crate::state::clear();
    }

    fn resolve_target(&mut self) -> Option<HWND> {
        if let Some(hwnd) = self.target_window {
            if window::is_valid(hwnd) {
                return Some(hwnd);
            }
        }

        let processes = vec![self.config.target.process.to_lowercase()];
        let query = WindowQuery {
            processes: &processes,
            class: optional(&self.config.target.window_class),
            title_contains: optional(&self.config.target.window_title),
        };

        match window::find_window(&query) {
            Some(hwnd) => {
                self.target_window = Some(hwnd);
                self.target_pid = Some(window::process_id(hwnd));
                crate::logline!(
                    "已定位目标窗口 hwnd={:?} pid={:?} title={}",
                    hwnd.0,
                    self.target_pid,
                    window::title(hwnd)
                );
                Some(hwnd)
            }
            None => {
                self.target_window = None;
                self.target_pid = None;
                None
            }
        }
    }

    fn resolve_game(&mut self) {
        if let Some(hwnd) = self.game_window {
            if window::is_valid(hwnd) {
                self.game_pid = Some(window::process_id(hwnd));
                return;
            }
        }

        let processes = vec![self.config.game.process.to_lowercase()];
        let query = WindowQuery {
            processes: &processes,
            class: None,
            title_contains: None,
        };

        match window::find_window(&query) {
            Some(hwnd) => {
                self.game_window = Some(hwnd);
                self.game_pid = Some(window::process_id(hwnd));
                crate::logline!("已定位游戏窗口 hwnd={:?} pid={:?}", hwnd.0, self.game_pid);
            }
            None => {
                self.game_window = None;
                self.game_pid = None;
            }
        }
    }

    fn is_target(&self, hwnd: HWND) -> bool {
        if hwnd.0.is_null() {
            return false;
        }
        match self.target_pid {
            Some(pid) => window::process_id(hwnd) == pid,
            None => Some(hwnd) == self.target_window,
        }
    }

    fn is_game(&self, hwnd: HWND) -> bool {
        if hwnd.0.is_null() {
            return false;
        }
        match self.game_pid {
            Some(pid) => window::process_id(hwnd) == pid,
            None => false,
        }
    }

    fn compute_peek_rect(&self, original: Option<RECT>) -> RECT {
        let area = window::work_area(self.game_window);
        let width = self.config.layout.width.max(160);
        let height = self.config.layout.height.max(90);
        let margin = self.config.layout.margin.max(0);

        let (left, top) = match self.config.layout.position.as_str() {
            "top-right" => (area.right - width - margin, area.top + margin),
            "top-left" => (area.left + margin, area.top + margin),
            "bottom-left" => (area.left + margin, area.bottom - height - margin),
            "remember" => self
                .last_peek_rect
                .or(original)
                .map(|rect| (rect.left, rect.top))
                .unwrap_or((area.right - width - margin, area.bottom - height - margin)),
            _ => (area.right - width - margin, area.bottom - height - margin),
        };

        RECT {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    fn shutdown(&mut self) {
        crate::logline!("开始退出流程");

        if self.phase == Phase::Peek {
            if let Some(target) = self.target_window {
                if window::is_valid(target) {
                    let _ = window::drop_topmost(target);
                }
            }
        }

        self.restore_audio();
    }
}

fn optional(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}