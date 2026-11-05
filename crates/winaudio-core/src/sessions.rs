use std::path::Path;
use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE, HWND, LPARAM, MAX_PATH, S_OK};
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, AudioSessionStateActive, AudioSessionStateExpired,
    AudioSessionStateInactive, IAudioSessionControl, IAudioSessionControl2,
    IAudioSessionEnumerator, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::com::ensure_mta;
use crate::errors::{Result, WinAudioError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Inactive,
    Expired,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct AudioSession {
    pub pid: u32,
    pub process_name: String,
    pub window_title: Option<String>,
    pub volume: f32,
    pub is_muted: bool,
    pub is_system_sound: bool,
    pub state: SessionState,
}

pub struct SessionManager;

impl SessionManager {
    /// Retrieves the default multimedia audio render endpoint.
    fn get_default_render_device() -> Result<IMMDevice> {
        ensure_mta()?;
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
            Ok(device)
        }
    }

    /// Retrieves the audio session manager for the default render device.
    fn get_session_manager() -> Result<IAudioSessionManager2> {
        let device = Self::get_default_render_device()?;
        unsafe {
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            Ok(manager)
        }
    }

    /// Lists all current audio sessions and their process info.
    pub fn list_sessions() -> Result<Vec<AudioSession>> {
        let manager = Self::get_session_manager()?;
        let mut sessions = Vec::new();

        unsafe {
            let enumerator: IAudioSessionEnumerator = manager.GetSessionEnumerator()?;
            let count = enumerator.GetCount()?;

            for i in 0..count {
                if let Ok(control) = enumerator.GetSession(i) {
                    if let Ok(session) = Self::parse_session(&control) {
                        sessions.push(session);
                    }
                }
            }
        }

        Ok(sessions)
    }

    /// Parses an `IAudioSessionControl` into a high-level `AudioSession`.
    fn parse_session(control: &IAudioSessionControl) -> Result<AudioSession> {
        unsafe {
            let control2: IAudioSessionControl2 = control.cast()?;
            let pid = control2.GetProcessId()?;
            let is_system_sound = control2.IsSystemSoundsSession() == S_OK;

            let raw_state = control2.GetState()?;
            let state = if raw_state == AudioSessionStateActive {
                SessionState::Active
            } else if raw_state == AudioSessionStateInactive {
                SessionState::Inactive
            } else if raw_state == AudioSessionStateExpired {
                SessionState::Expired
            } else {
                SessionState::Unknown
            };

            let simple_vol: ISimpleAudioVolume = control.cast()?;
            let volume = simple_vol.GetMasterVolume()?;
            let is_muted = simple_vol.GetMute()?.as_bool();

            let process_name = if is_system_sound || pid == 0 {
                "System Sounds".to_string()
            } else {
                get_process_name_by_pid(pid).unwrap_or_else(|| format!("PID:{}", pid))
            };

            let window_title = if pid > 0 {
                get_window_title_by_pid(pid)
            } else {
                None
            };

            Ok(AudioSession {
                pid,
                process_name,
                window_title,
                volume,
                is_muted,
                is_system_sound,
                state,
            })
        }
    }

    /// Sets the volume (0.0 to 1.0) for all sessions matching a target PID.
    pub fn set_volume(pid: u32, volume: f32) -> Result<()> {
        let clamped = volume.clamp(0.0, 1.0);
        let manager = Self::get_session_manager()?;
        let mut found = false;

        unsafe {
            let enumerator = manager.GetSessionEnumerator()?;
            let count = enumerator.GetCount()?;

            for i in 0..count {
                if let Ok(control) = enumerator.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        if control2.GetProcessId().unwrap_or(0) == pid {
                            if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                                simple_vol.SetMasterVolume(clamped, std::ptr::null())?;
                                found = true;
                            }
                        }
                    }
                }
            }
        }

        if found {
            Ok(())
        } else {
            Err(WinAudioError::SessionNotFound(pid))
        }
    }

    /// Sets the mute status for all sessions matching a target PID.
    pub fn set_mute(pid: u32, mute: bool) -> Result<()> {
        let manager = Self::get_session_manager()?;
        let mut found = false;

        unsafe {
            let enumerator = manager.GetSessionEnumerator()?;
            let count = enumerator.GetCount()?;

            for i in 0..count {
                if let Ok(control) = enumerator.GetSession(i) {
                    if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                        if control2.GetProcessId().unwrap_or(0) == pid {
                            if let Ok(simple_vol) = control.cast::<ISimpleAudioVolume>() {
                                simple_vol.SetMute(BOOL::from(mute), std::ptr::null())?;
                                found = true;
                            }
                        }
                    }
                }
            }
        }

        if found {
            Ok(())
        } else {
            Err(WinAudioError::SessionNotFound(pid))
        }
    }

    /// Finds sessions by executable name (e.g. "spotify.exe", case-insensitive).
    pub fn find_by_name(name: &str) -> Result<Vec<AudioSession>> {
        let sessions = Self::list_sessions()?;
        let lower = name.to_lowercase();
        let matched = sessions
            .into_iter()
            .filter(|s| s.process_name.to_lowercase().contains(&lower))
            .collect();
        Ok(matched)
    }

    /// Finds session by PID.
    pub fn find_by_pid(pid: u32) -> Result<Option<AudioSession>> {
        let sessions = Self::list_sessions()?;
        Ok(sessions.into_iter().find(|s| s.pid == pid))
    }
}

/// Helper function to retrieve the executable name for a given PID.
pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    unsafe {
        let handle: HANDLE =
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; MAX_PATH as usize];
        let mut size = buffer.len() as u32;

        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );

        let _ = CloseHandle(handle);

        if ok.is_ok() {
            let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
            let file_name = Path::new(&full_path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(&full_path)
                .to_string();
            Some(file_name)
        } else {
            None
        }
    }
}

struct WindowSearchData {
    target_pid: u32,
    found_title: Option<String>,
}

/// Helper function to retrieve the visible window title for a given PID.
pub fn get_window_title_by_pid(pid: u32) -> Option<String> {
    unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = unsafe { &mut *(lparam.0 as *mut WindowSearchData) };

        let mut window_pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        }

        if window_pid == data.target_pid && unsafe { IsWindowVisible(hwnd).as_bool() } {
            let length = unsafe { GetWindowTextLengthW(hwnd) };
            if length > 0 {
                let mut buffer = vec![0u16; (length + 1) as usize];
                let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
                if copied > 0 {
                    let title = String::from_utf16_lossy(&buffer[..copied as usize]);
                    let trimmed = title.trim();
                    if !trimmed.is_empty() {
                        data.found_title = Some(trimmed.to_string());
                        return BOOL(0); // Stop enumeration
                    }
                }
            }
        }

        BOOL(1) // Continue enumeration
    }

    let mut data = WindowSearchData {
        target_pid: pid,
        found_title: None,
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_window_callback),
            LPARAM(&mut data as *mut _ as isize),
        );
    }

    data.found_title
}
