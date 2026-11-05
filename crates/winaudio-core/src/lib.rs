pub mod capture;
pub mod com;
pub mod dsp;
pub mod errors;
pub mod sessions;

pub use capture::{
    AudioCapture, AudioFormat, AudioFrame, CaptureBuilder, CaptureConfig, CaptureMode,
    TargetProcess,
};
pub use errors::{Result, WinAudioError};
pub use sessions::{AudioSession, SessionManager, SessionState};

/// Convenience function to quickly list all active Windows audio sessions.
pub fn list_sessions() -> Result<Vec<AudioSession>> {
    SessionManager::list_sessions()
}

/// Convenience function to adjust volume by process name.
pub fn set_volume_by_name(name: &str, volume: f32) -> Result<()> {
    let sessions = SessionManager::find_by_name(name)?;
    if sessions.is_empty() {
        return Err(WinAudioError::ProcessNotFound(format!(
            "No audio session found with name: {}",
            name
        )));
    }
    for session in sessions {
        if session.pid > 0 {
            let _ = SessionManager::set_volume(session.pid, volume);
        }
    }
    Ok(())
}

/// Convenience function to mute or unmute by process name.
pub fn set_mute_by_name(name: &str, mute: bool) -> Result<()> {
    let sessions = SessionManager::find_by_name(name)?;
    if sessions.is_empty() {
        return Err(WinAudioError::ProcessNotFound(format!(
            "No audio session found with name: {}",
            name
        )));
    }
    for session in sessions {
        if session.pid > 0 {
            let _ = SessionManager::set_mute(session.pid, mute);
        }
    }
    Ok(())
}
