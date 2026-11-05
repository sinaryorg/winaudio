use std::collections::HashMap;
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
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
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
    pub host_app_name: Option<String>,
    pub display_name: String,
    pub window_title: Option<String>,
    pub volume: f32,
    pub is_muted: bool,
    pub is_system_sound: bool,
    pub state: SessionState,
}

#[derive(Debug, Clone)]
struct ProcessNode {
    parent_pid: u32,
    exe_name: String,
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

    /// Takes a rapid snapshot of the current OS process tree to resolve parent/child hierarchies.
    fn build_process_tree() -> HashMap<u32, ProcessNode> {
        let mut map = HashMap::new();
        unsafe {
            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(s) => s,
                Err(_) => return map,
            };

            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let null_pos = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile[..null_pos]);

                    map.insert(
                        entry.th32ProcessID,
                        ProcessNode {
                            parent_pid: entry.th32ParentProcessID,
                            exe_name,
                        },
                    );

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
        }
        map
    }

    /// Resolves the true host application by walking up the process hierarchy.
    /// This links background audio workers (like WebView2 or Chromium audio renderers)
    /// to their parent user-facing application without any hardcoded names.
    fn resolve_hierarchy(
        pid: u32,
        tree: &HashMap<u32, ProcessNode>,
    ) -> (String, Option<String>, String, Option<String>) {
        let base_exe = tree
            .get(&pid)
            .map(|p| p.exe_name.clone())
            .unwrap_or_else(|| {
                get_process_name_by_pid(pid).unwrap_or_else(|| format!("PID:{}", pid))
            });

        let direct_title = get_window_title_by_pid(pid);

        // Generic ancestor walk: stop at system processes or explorer.exe
        let system_roots = [
            "explorer.exe",
            "services.exe",
            "svchost.exe",
            "wininit.exe",
            "csrss.exe",
            "smss.exe",
            "lsass.exe",
            "winlogon.exe",
        ];

        let mut curr_pid = pid;
        let mut host_name: Option<String> = None;
        let mut ancestor_title: Option<String> = None;

        while let Some(node) = tree.get(&curr_pid) {
            let parent_pid = node.parent_pid;
            if parent_pid == 0 || parent_pid == curr_pid {
                break;
            }

            if let Some(parent) = tree.get(&parent_pid) {
                let parent_exe_lower = parent.exe_name.to_lowercase();
                if system_roots.iter().any(|&s| s == parent_exe_lower) {
                    break;
                }

                // If ancestor has a visible top-level window title
                if ancestor_title.is_none() {
                    if let Some(title) = get_window_title_by_pid(parent_pid) {
                        ancestor_title = Some(title);
                    }
                }

                // If parent executable name differs from child (e.g. worker -> main app)
                if parent.exe_name.to_lowercase() != base_exe.to_lowercase() {
                    host_name = Some(parent.exe_name.clone());
                }

                curr_pid = parent_pid;
            } else {
                break;
            }
        }

        let window_title = direct_title.or(ancestor_title);

        let display_name = if let Some(ref host) = host_name {
            let host_base = host.strip_suffix(".exe").unwrap_or(host);
            format!("{} [{}]", base_exe, host_base)
        } else {
            base_exe.clone()
        };

        (base_exe, host_name, display_name, window_title)
    }

    /// Lists all current audio sessions and their process info.
    pub fn list_sessions() -> Result<Vec<AudioSession>> {
        let manager = Self::get_session_manager()?;
        let process_tree = Self::build_process_tree();
        let mut sessions = Vec::new();

        unsafe {
            let enumerator: IAudioSessionEnumerator = manager.GetSessionEnumerator()?;
            let count = enumerator.GetCount()?;

            for i in 0..count {
                if let Ok(control) = enumerator.GetSession(i) {
                    if let Ok(session) = Self::parse_session(&control, &process_tree) {
                        sessions.push(session);
                    }
                }
            }
        }

        Ok(sessions)
    }

    /// Parses an `IAudioSessionControl` into a high-level `AudioSession`.
    fn parse_session(
        control: &IAudioSessionControl,
        tree: &HashMap<u32, ProcessNode>,
    ) -> Result<AudioSession> {
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

            if is_system_sound || pid == 0 {
                return Ok(AudioSession {
                    pid: 0,
                    process_name: "System Sounds".to_string(),
                    host_app_name: None,
                    display_name: "System Sounds".to_string(),
                    window_title: None,
                    volume,
                    is_muted,
                    is_system_sound: true,
                    state,
                });
            }

            let (process_name, host_app_name, display_name, window_title) =
                Self::resolve_hierarchy(pid, tree);

            Ok(AudioSession {
                pid,
                process_name,
                host_app_name,
                display_name,
                window_title,
                volume,
                is_muted,
                is_system_sound: false,
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

    /// Finds sessions by query string, matching process name, host app name, or window title.
    /// Automatically strips .exe suffix so 'app' and 'app.exe' behave identically.
    /// Results are prioritized so that currently Active audio sessions come first.
    pub fn find_by_name(query: &str) -> Result<Vec<AudioSession>> {
        let sessions = Self::list_sessions()?;
        let trimmed = query.trim();
        let clean_query = trimmed
            .strip_suffix(".exe")
            .unwrap_or(trimmed)
            .to_lowercase();

        let mut matched: Vec<AudioSession> = sessions
            .into_iter()
            .filter(|s| {
                let p_clean = s
                    .process_name
                    .strip_suffix(".exe")
                    .unwrap_or(&s.process_name)
                    .to_lowercase();
                let d_clean = s.display_name.to_lowercase();
                let h_clean = s
                    .host_app_name
                    .as_ref()
                    .map(|h| h.strip_suffix(".exe").unwrap_or(h).to_lowercase());
                let w_clean = s.window_title.as_ref().map(|w| w.to_lowercase());

                p_clean.contains(&clean_query)
                    || d_clean.contains(&clean_query)
                    || h_clean.map(|h| h.contains(&clean_query)).unwrap_or(false)
                    || w_clean.map(|w| w.contains(&clean_query)).unwrap_or(false)
            })
            .collect();

        // Sort so that Active audio sessions appear first!
        matched.sort_by_key(|s| match s.state {
            SessionState::Active => 0,
            SessionState::Inactive => 1,
            SessionState::Expired => 2,
            SessionState::Unknown => 3,
        });

        Ok(matched)
    }

    /// Finds session by PID.
    pub fn find_by_pid(pid: u32) -> Result<Option<AudioSession>> {
        let sessions = Self::list_sessions()?;
        Ok(sessions.into_iter().find(|s| s.pid == pid))
    }

    /// Resolves multi-target inputs (comma-separated or list of queries) into a TargetProcess.
    pub fn resolve_multi_include(names: &[String]) -> Result<crate::capture::TargetProcess> {
        if names.is_empty() {
            return Ok(crate::capture::TargetProcess::All);
        }
        if names.len() == 1 {
            return Ok(crate::capture::TargetProcess::ByName(names[0].clone()));
        }
        let targets = names
            .iter()
            .map(|n| crate::capture::TargetProcess::ByName(n.clone()))
            .collect();
        Ok(crate::capture::TargetProcess::Multiple(targets))
    }

    /// Resolves multiple exclusions.
    /// If 1 app is excluded: returns (TargetProcess::ByName(app), CaptureMode::Exclude)
    /// If multiple apps are excluded: computes Active Sessions - Excluded Apps and returns Multi-Include on remaining!
    pub fn resolve_multi_exclude(
        excluded_names: &[String],
    ) -> Result<(crate::capture::TargetProcess, crate::capture::CaptureMode)> {
        if excluded_names.is_empty() {
            return Ok((
                crate::capture::TargetProcess::All,
                crate::capture::CaptureMode::Include,
            ));
        }
        if excluded_names.len() == 1 {
            return Ok((
                crate::capture::TargetProcess::ByName(excluded_names[0].clone()),
                crate::capture::CaptureMode::Exclude,
            ));
        }

        // Multiple exclusions: Invert against currently active audio sessions
        let active_sessions = Self::list_sessions()?
            .into_iter()
            .filter(|s| s.state == SessionState::Active && s.pid > 0)
            .collect::<Vec<_>>();

        let mut remaining_pids = Vec::new();
        for s in active_sessions {
            let is_excluded = excluded_names.iter().any(|ex| {
                if let Ok(pid_val) = ex.trim().parse::<u32>() {
                    if s.pid == pid_val {
                        return true;
                    }
                }
                let clean_ex = ex
                    .trim()
                    .strip_suffix(".exe")
                    .unwrap_or(ex.trim())
                    .to_lowercase();
                s.process_name.to_lowercase().contains(&clean_ex)
                    || s.display_name.to_lowercase().contains(&clean_ex)
                    || s.host_app_name
                        .as_ref()
                        .map(|h| h.to_lowercase().contains(&clean_ex))
                        .unwrap_or(false)
                    || s.window_title
                        .as_ref()
                        .map(|w| w.to_lowercase().contains(&clean_ex))
                        .unwrap_or(false)
            });

            if !is_excluded {
                remaining_pids.push(s.pid);
            }
        }

        if remaining_pids.is_empty() {
            return Err(WinAudioError::ProcessNotFound(
                "No remaining active audio sessions found after applying multiple exclusions"
                    .into(),
            ));
        }

        if remaining_pids.len() == 1 {
            Ok((
                crate::capture::TargetProcess::ByPid(remaining_pids[0]),
                crate::capture::CaptureMode::Include,
            ))
        } else {
            Ok((
                crate::capture::TargetProcess::Multiple(
                    remaining_pids
                        .into_iter()
                        .map(crate::capture::TargetProcess::ByPid)
                        .collect(),
                ),
                crate::capture::CaptureMode::Include,
            ))
        }
    }
}

/// Helper function to retrieve the executable name for a given PID.
pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
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
