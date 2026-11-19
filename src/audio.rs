use anyhow::{Context, Result};
use windows::core::Interface;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

#[derive(Debug, Clone)]
pub struct MuteSnapshot {
    pub pid: u32,
    pub session_id: String,
    pub was_muted: bool,
}

/// 按进程静音的实现。所有方法都应在同一个 MTA 线程上调用。
pub struct AudioManager {
    enumerator: IMMDeviceEnumerator,
}

impl AudioManager {
    pub fn new() -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .context("创建音频设备枚举器失败")?;
            Ok(Self { enumerator })
        }
    }

    fn sessions(&self) -> Result<windows::Win32::Media::Audio::IAudioSessionEnumerator> {
        unsafe {
            let device = self.enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            Ok(manager.GetSessionEnumerator()?)
        }
    }

    /// 记录指定进程全部会话的静音状态，然后静音。
    pub fn snapshot_and_mute(&self, pid: u32) -> Result<Vec<MuteSnapshot>> {
        let mut snapshots = Vec::new();
        let sessions = self.sessions()?;
        let count = unsafe { sessions.GetCount()? };

        for index in 0..count {
            let control = unsafe { sessions.GetSession(index)? };
            let control2: IAudioSessionControl2 = control.cast()?;
            if unsafe { control2.GetProcessId()? } != pid {
                continue;
            }
            let volume: ISimpleAudioVolume = control.cast()?;
            let was_muted = unsafe { volume.GetMute()?.as_bool() };
            let session_id = unsafe {
                control2
                    .GetSessionIdentifier()
                    .map(|value| value.to_string().unwrap_or_default())
                    .unwrap_or_default()
            };
            snapshots.push(MuteSnapshot {
                pid,
                session_id,
                was_muted,
            });
            unsafe { volume.SetMute(BOOL(1), std::ptr::null())? };
        }

        Ok(snapshots)
    }

    pub fn restore(&self, snapshots: &[MuteSnapshot]) {
        for snapshot in snapshots {
            if let Err(err) = self.restore_one(snapshot) {
                // 会话可能刚好在重建，稍等后重试一次。
                std::thread::sleep(std::time::Duration::from_millis(80));
                if let Err(retry_err) = self.restore_one(snapshot) {
                    crate::logline!(
                        "还原音频失败 pid={} err={err:#} retry={retry_err:#}",
                        snapshot.pid
                    );
                }
            }
        }
    }

    fn restore_one(&self, snapshot: &MuteSnapshot) -> Result<()> {
        let sessions = self.sessions()?;
        let count = unsafe { sessions.GetCount()? };

        for index in 0..count {
            let control = unsafe { sessions.GetSession(index)? };
            let control2: IAudioSessionControl2 = control.cast()?;
            if unsafe { control2.GetProcessId()? } != snapshot.pid {
                continue;
            }
            let session_id = unsafe {
                control2
                    .GetSessionIdentifier()
                    .map(|value| value.to_string().unwrap_or_default())
                    .unwrap_or_default()
            };
            if !snapshot.session_id.is_empty() && session_id != snapshot.session_id {
                continue;
            }
            let volume: ISimpleAudioVolume = control.cast()?;
            unsafe { volume.SetMute(BOOL(snapshot.was_muted as i32), std::ptr::null())? };
        }

        Ok(())
    }
}