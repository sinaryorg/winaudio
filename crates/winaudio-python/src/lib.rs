use std::time::Duration;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use winaudio_core::{
    AudioCapture as CoreCapture, AudioFormat, AudioSession as CoreSession, SessionManager,
    SessionState, TargetProcess,
};

#[pyclass(name = "AudioSession")]
#[derive(Clone)]
pub struct PyAudioSession {
    #[pyo3(get)]
    pub pid: u32,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub display_name: String,
    #[pyo3(get)]
    pub host_name: Option<String>,
    #[pyo3(get)]
    pub window_title: Option<String>,
    #[pyo3(get)]
    pub volume: f32,
    #[pyo3(get)]
    pub is_muted: bool,
    #[pyo3(get)]
    pub is_system_sound: bool,
    #[pyo3(get)]
    pub state: String,
}

#[pymethods]
impl PyAudioSession {
    #[getter]
    pub fn host_app_name(&self) -> Option<String> {
        self.host_name.clone()
    }

    pub fn set_volume(&self, volume: f32) -> PyResult<()> {
        if self.pid == 0 {
            return Err(PyValueError::new_err("Cannot adjust volume of system session directly"));
        }
        SessionManager::set_volume(self.pid, volume)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    pub fn set_mute(&self, mute: bool) -> PyResult<()> {
        if self.pid == 0 {
            return Err(PyValueError::new_err("Cannot mute system session directly"));
        }
        SessionManager::set_mute(self.pid, mute)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "<AudioSession pid={} app='{}' window='{}' volume={:.2} muted={} state={}>",
            self.pid,
            self.display_name,
            self.window_title.as_deref().unwrap_or("-"),
            self.volume,
            self.is_muted,
            self.state
        )
    }
}

impl From<CoreSession> for PyAudioSession {
    fn from(s: CoreSession) -> Self {
        let state_str = match s.state {
            SessionState::Active => "Active",
            SessionState::Inactive => "Inactive",
            SessionState::Expired => "Expired",
            SessionState::Unknown => "Unknown",
        }
        .to_string();

        Self {
            pid: s.pid,
            name: s.process_name,
            display_name: s.display_name,
            host_name: s.host_app_name,
            window_title: s.window_title,
            volume: s.volume,
            is_muted: s.is_muted,
            is_system_sound: s.is_system_sound,
            state: state_str,
        }
    }
}

#[pyfunction]
pub fn list_sessions() -> PyResult<Vec<PyAudioSession>> {
    let sessions = SessionManager::list_sessions()
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    Ok(sessions.into_iter().map(PyAudioSession::from).collect())
}

#[pyfunction]
pub fn set_volume(pid: u32, volume: f32) -> PyResult<()> {
    SessionManager::set_volume(pid, volume)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
pub fn set_mute(pid: u32, mute: bool) -> PyResult<()> {
    SessionManager::set_mute(pid, mute)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
pub fn set_volume_by_name(name: &str, volume: f32) -> PyResult<()> {
    winaudio_core::set_volume_by_name(name, volume)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
pub fn set_mute_by_name(name: &str, mute: bool) -> PyResult<()> {
    winaudio_core::set_mute_by_name(name, mute)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyclass(name = "Capture")]
pub struct PyCapture {
    capture: Option<CoreCapture>,
    pid: u32,
}

#[pymethods]
impl PyCapture {
    #[new]
    #[pyo3(signature = (all=false, pid=None, name=None, window=None, exclude=None, sample_rate=48000, channels=2))]
    pub fn new(
        all: bool,
        pid: Option<u32>,
        name: Option<String>,
        window: Option<String>,
        exclude: Option<String>,
        sample_rate: u32,
        channels: u16,
    ) -> PyResult<Self> {
        let (target, mode) = if all || (pid.is_none() && name.is_none() && window.is_none() && exclude.is_none()) {
            (TargetProcess::All, winaudio_core::CaptureMode::Include)
        } else if let Some(ex) = exclude {
            let ex_list: Vec<String> = ex
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            SessionManager::resolve_multi_exclude(&ex_list)
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        } else if let Some(n) = name {
            let name_list: Vec<String> = n
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (
                SessionManager::resolve_multi_include(&name_list)
                    .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
                winaudio_core::CaptureMode::Include,
            )
        } else if let Some(p) = pid {
            (TargetProcess::ByPid(p), winaudio_core::CaptureMode::Include)
        } else if let Some(w) = window {
            (TargetProcess::ByWindowTitle(w), winaudio_core::CaptureMode::Include)
        } else {
            (TargetProcess::All, winaudio_core::CaptureMode::Include)
        };

        let cap = CoreCapture::builder(target)
            .mode(mode)
            .sample_rate(sample_rate)
            .channels(channels)
            .format(AudioFormat::Pcm16)
            .start()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let resolved_pid = cap.target_pid();

        Ok(Self {
            capture: Some(cap),
            pid: resolved_pid,
        })
    }

    #[getter]
    pub fn target_pid(&self) -> u32 {
        self.pid
    }

    pub fn record_to_wav(&self, path: &str, duration_seconds: f32) -> PyResult<()> {
        let cap = self
            .capture
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Capture already closed"))?;

        let dur = Duration::from_secs_f32(duration_seconds);
        cap.record_to_wav(path, dur)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    pub fn read_frame<'py>(&self, py: Python<'py>, timeout_ms: u64) -> PyResult<Option<Bound<'py, PyBytes>>> {
        let cap = self
            .capture
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("Capture already closed"))?;

        match cap.receiver().recv_timeout(Duration::from_millis(timeout_ms)) {
            Ok(frame) => {
                let bytes = frame.as_bytes();
                Ok(Some(PyBytes::new_bound(py, &bytes)))
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => Ok(None),
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut cap) = self.capture.take() {
            cap.stop();
        }
    }

    pub fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (_exc_type=None, _exc_val=None, _exc_tb=None))]
    pub fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) {
        self.stop();
    }
}

/// A Python module implemented in Rust for Windows per-process audio capture and control.
#[pymodule]
fn winaudio(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAudioSession>()?;
    m.add_class::<PyCapture>()?;
    m.add_function(wrap_pyfunction!(list_sessions, m)?)?;
    m.add_function(wrap_pyfunction!(set_volume, m)?)?;
    m.add_function(wrap_pyfunction!(set_mute, m)?)?;
    m.add_function(wrap_pyfunction!(set_volume_by_name, m)?)?;
    m.add_function(wrap_pyfunction!(set_mute_by_name, m)?)?;
    Ok(())
}
