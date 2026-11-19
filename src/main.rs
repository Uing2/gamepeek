#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_export]
macro_rules! logline {
    ($($arg:tt)*) => {
        $crate::diagnostics::write(format!($($arg)*))
    };
}

mod app;
mod audio;
mod autostart;
mod config;
mod controller;
mod diagnostics;
mod foreground;
mod hotkey;
mod media;
mod state;
mod tray;
mod util;
mod window;

use windows::core::w;
use windows::Win32::Foundation::{GetLastError, BOOL, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;

fn main() {
    // 命名互斥体保证单实例；句柄在整个进程生命周期内保持存活。
    let mutex = unsafe { CreateMutexW(None, BOOL(1), w!("Global\\GamePeek_SingleInstance")) };

    match mutex {
        Ok(_handle) => {
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                return;
            }
            if let Err(err) = app::run() {
                crate::logline!("运行失败: {err:#}");
                #[cfg(debug_assertions)]
                eprintln!("运行失败: {err:#}");
            }
        }
        Err(err) => {
            crate::logline!("创建单实例互斥体失败: {err}");
            if let Err(err) = app::run() {
                crate::logline!("运行失败: {err:#}");
            }
        }
    }
}