use thiserror::Error;

#[derive(Error, Debug)]
pub enum WinAudioError {
    #[error("Windows COM error: {0}")]
    Com(#[from] windows::core::Error),

    #[error("Process not found: {0}")]
    ProcessNotFound(String),

    #[error("Audio session not found for PID: {0}")]
    SessionNotFound(u32),

    #[error("Default audio render device not found")]
    DeviceNotFound,

    #[error("Activation failed: {0}")]
    ActivationFailed(String),

    #[error("Capture stream error: {0}")]
    Capture(String),

    #[error("DSP/Resampling error: {0}")]
    Dsp(String),

    #[error("WAV encoder error: {0}")]
    WavError(#[from] hound::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, WinAudioError>;
