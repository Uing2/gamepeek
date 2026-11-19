use std::time::Instant;

use windows::Win32::Foundation::HWND;

/// 转换为以 0 结尾的宽字符串，供 Win32 的 PCWSTR 参数使用。
pub fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// HWND 不是 Send，跨线程传递时统一转成整数。
pub fn hwnd_to_i64(hwnd: HWND) -> i64 {
    hwnd.0 as i64
}

pub fn hwnd_from_i64(value: i64) -> HWND {
    HWND(value as *mut core::ffi::c_void)
}

pub struct Stopwatch(Instant);

impl Stopwatch {
    pub fn new() -> Self {
        Self(Instant::now())
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.0.elapsed().as_secs_f64() * 1000.0
    }
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self::new()
    }
}