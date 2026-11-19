use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("GamePeek");

/// 按配置写入/移除当前用户的登录自启动项。失败不影响主流程。
pub fn apply(enabled: bool) {
    let command = match current_command() {
        Some(command) => command,
        None => return,
    };

    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, KEY_SET_VALUE, &mut key);
        if status != ERROR_SUCCESS {
            crate::logline!("打开自启动注册表项失败: {status:?}");
            return;
        }

        if enabled {
            let wide = crate::util::to_wide(&command);
            let bytes: Vec<u8> = wide.iter().flat_map(|unit| unit.to_ne_bytes()).collect();
            let status = RegSetValueExW(key, VALUE_NAME, 0, REG_SZ, Some(&bytes));
            if status == ERROR_SUCCESS {
                crate::logline!("已设置开机自启");
            } else {
                crate::logline!("设置开机自启失败: {status:?}");
            }
        } else {
            let _ = RegDeleteValueW(key, VALUE_NAME);
        }

        let _ = RegCloseKey(key);
    }
}

fn current_command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.to_string_lossy().to_string();
    Some(format!("\"{path}\""))
}