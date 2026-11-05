use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Threading::{
    CreateEventW, OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};

use crate::com::{activate_process_loopback_client, ensure_mta, ProcessLoopbackMode};
use crate::dsp::{downmix_to_mono, downmix_to_stereo, f32_to_pcm16, pcm16_to_bytes, resample_linear};
use crate::errors::{Result, WinAudioError};
use crate::sessions::SessionManager;

#[link(name = "avrt")]
extern "system" {
    fn AvSetMmThreadCharacteristicsW(taskname: PCWSTR, taskindex: *mut u32) -> HANDLE;
    fn AvRevertMmThreadCharacteristics(avrt_handle: HANDLE) -> windows::Win32::Foundation::BOOL;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetProcess {
    All,
    ByPid(u32),
    ByName(String),
    ByWindowTitle(String),
    Multiple(Vec<TargetProcess>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Include,
    Exclude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Pcm16,
    Float32,
}

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub target: TargetProcess,
    pub mode: CaptureMode,
    pub sample_rate: u32,
    pub channels: u16,
    pub format: AudioFormat,
    pub silence_padding: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            target: TargetProcess::All,
            mode: CaptureMode::Include,
            sample_rate: 48000,
            channels: 2,
            format: AudioFormat::Pcm16,
            silence_padding: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub samples_f32: Vec<f32>,
    pub samples_i16: Vec<i16>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl AudioFrame {
    pub fn as_bytes(&self) -> Vec<u8> {
        if !self.samples_i16.is_empty() {
            pcm16_to_bytes(&self.samples_i16)
        } else {
            let pcm = f32_to_pcm16(&self.samples_f32);
            pcm16_to_bytes(&pcm)
        }
    }
}

pub struct CaptureBuilder {
    config: CaptureConfig,
}

impl CaptureBuilder {
    pub fn new(target: TargetProcess) -> Self {
        Self {
            config: CaptureConfig {
                target,
                ..Default::default()
            },
        }
    }

    pub fn all() -> Self {
        Self::new(TargetProcess::All)
    }

    pub fn mode(mut self, mode: CaptureMode) -> Self {
        self.config.mode = mode;
        self
    }

    pub fn exclude(mut self) -> Self {
        self.config.mode = CaptureMode::Exclude;
        self
    }

    pub fn sample_rate(mut self, rate: u32) -> Self {
        self.config.sample_rate = rate;
        self
    }

    pub fn channels(mut self, channels: u16) -> Self {
        self.config.channels = channels;
        self
    }

    pub fn format(mut self, format: AudioFormat) -> Self {
        self.config.format = format;
        self
    }

    pub fn silence_padding(mut self, padding: bool) -> Self {
        self.config.silence_padding = padding;
        self
    }

    pub fn start(self) -> Result<AudioCapture> {
        AudioCapture::start_with_config(self.config)
    }
}

pub struct AudioCapture {
    receiver: Receiver<AudioFrame>,
    stop_signal: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<Result<()>>>,
    target_pid: u32,
    resolved_pids: Vec<u32>,
    config: CaptureConfig,
}

impl AudioCapture {
    pub fn builder(target: TargetProcess) -> CaptureBuilder {
        CaptureBuilder::new(target)
    }

    pub fn all() -> CaptureBuilder {
        CaptureBuilder::all()
    }

    pub fn exclude(target: TargetProcess) -> CaptureBuilder {
        CaptureBuilder::new(target).mode(CaptureMode::Exclude)
    }

    pub fn receiver(&self) -> &Receiver<AudioFrame> {
        &self.receiver
    }

    pub fn target_pid(&self) -> u32 {
        self.target_pid
    }

    pub fn resolved_pids(&self) -> &[u32] {
        &self.resolved_pids
    }

    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }

    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }

    /// Records audio to a standard WAV file for a given duration with an optional progress callback.
    pub fn record_to_wav_with_progress<F>(&self, path: &str, duration: Duration, mut on_progress_secs: F) -> Result<()>
    where
        F: FnMut(u64),
    {
        let spec = hound::WavSpec {
            channels: self.config.channels,
            sample_rate: self.config.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(path, spec)?;
        let start = Instant::now();
        let mut last_sec = 0;

        while start.elapsed() < duration {
            if let Ok(frame) = self.receiver.recv_timeout(Duration::from_millis(50)) {
                for &sample in &frame.samples_i16 {
                    writer.write_sample(sample)?;
                }
            }
            let elapsed_secs = start.elapsed().as_secs();
            if elapsed_secs != last_sec {
                on_progress_secs(elapsed_secs);
                last_sec = elapsed_secs;
            }
        }

        writer.finalize()?;
        Ok(())
    }

    /// Records audio to a standard WAV file for a given duration.
    pub fn record_to_wav(&self, path: &str, duration: Duration) -> Result<()> {
        self.record_to_wav_with_progress(path, duration, |_| {})
    }

    fn resolve_target_pid(target: &TargetProcess) -> Result<Option<u32>> {
        match target {
            TargetProcess::All => Ok(None),
            TargetProcess::ByPid(pid) => Ok(Some(*pid)),
            TargetProcess::ByName(name) => {
                let sessions = SessionManager::find_by_name(name)?;
                if let Some(session) = sessions.into_iter().find(|s| s.pid > 0) {
                    Ok(Some(session.pid))
                } else {
                    Err(WinAudioError::ProcessNotFound(format!(
                        "No active audio session found for name: {}",
                        name
                    )))
                }
            }
            TargetProcess::ByWindowTitle(title) => {
                let sessions = SessionManager::list_sessions()?;
                let lower = title.to_lowercase();
                if let Some(session) = sessions.into_iter().find(|s| {
                    s.window_title
                        .as_ref()
                        .map(|t| t.to_lowercase().contains(&lower))
                        .unwrap_or(false)
                }) {
                    Ok(Some(session.pid))
                } else {
                    Err(WinAudioError::ProcessNotFound(format!(
                        "No active audio session found with window title containing: {}",
                        title
                    )))
                }
            }
            TargetProcess::Multiple(_) => Ok(None),
        }
    }

    pub fn start_with_config(config: CaptureConfig) -> Result<Self> {
        // Handle Multi-App Capture
        if let TargetProcess::Multiple(ref targets) = config.target {
            if targets.is_empty() {
                return Self::start_with_config(CaptureConfig {
                    target: TargetProcess::All,
                    ..config
                });
            }
            if targets.len() == 1 {
                return Self::start_with_config(CaptureConfig {
                    target: targets[0].clone(),
                    ..config
                });
            }

            // Spawn individual capture streams
            let mut captures = Vec::new();
            let mut resolved_pids = Vec::new();
            for t in targets {
                if let Ok(c) = AudioCapture::start_with_config(CaptureConfig {
                    target: t.clone(),
                    mode: config.mode,
                    sample_rate: config.sample_rate,
                    channels: config.channels,
                    format: config.format,
                    silence_padding: config.silence_padding,
                }) {
                    resolved_pids.push(c.target_pid());
                    captures.push(c);
                }
            }

            if captures.is_empty() {
                return Err(WinAudioError::Capture(
                    "None of the target applications could be captured for mixing".into(),
                ));
            }

            let (sender, receiver) = bounded(128);
            let stop_signal = Arc::new(AtomicBool::new(false));
            let stop_clone = Arc::clone(&stop_signal);

            let sample_rate = config.sample_rate;
            let channels = config.channels;

            let worker_handle = thread::spawn(move || {
                mixer_worker_loop(captures, sender, stop_clone, sample_rate, channels)
            });

            return Ok(Self {
                receiver,
                stop_signal,
                worker_handle: Some(worker_handle),
                target_pid: 0,
                resolved_pids,
                config,
            });
        }

        // Single target or All System Audio
        let target_pid_opt = Self::resolve_target_pid(&config.target)?;
        let target_pid = target_pid_opt.unwrap_or(0);

        let (sender, receiver) = bounded(128);
        let stop_signal = Arc::new(AtomicBool::new(false));

        let stop_clone = Arc::clone(&stop_signal);
        let config_clone = config.clone();

        let worker_handle = thread::spawn(move || {
            if let Err(e) = capture_worker_loop(target_pid_opt, config_clone, sender, stop_clone) {
                eprintln!("[winaudio error] capture worker error: {:?}", e);
                return Err(e);
            }
            Ok(())
        });

        Ok(Self {
            receiver,
            stop_signal,
            worker_handle: Some(worker_handle),
            target_pid,
            resolved_pids: if target_pid > 0 { vec![target_pid] } else { Vec::new() },
            config,
        })
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn get_system_mix_format() -> Result<WAVEFORMATEXTENSIBLE> {
    ensure_mta()?;
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let p_format = audio_client.GetMixFormat()?;
        let format = *p_format.cast::<WAVEFORMATEXTENSIBLE>();
        Ok(format)
    }
}

/// Activates master system audio endpoint loopback (captures all sound).
fn activate_master_loopback_client() -> Result<IAudioClient> {
    ensure_mta()?;
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        Ok(client)
    }
}

fn capture_worker_loop(
    target_pid_opt: Option<u32>,
    config: CaptureConfig,
    sender: Sender<AudioFrame>,
    stop_signal: Arc<AtomicBool>,
) -> Result<()> {
    ensure_mta()?;

    // Boost thread priority with MMCSS
    let mut task_index = 0u32;
    let avrt_handle = unsafe {
        AvSetMmThreadCharacteristicsW(windows::core::w!("Audio"), &mut task_index)
    };

    // Open target process handle to detect process exit (only for single PID include mode)
    let process_handle = if let Some(pid) = target_pid_opt {
        unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid).ok() }
    } else {
        None
    };

    let audio_client = match target_pid_opt {
        Some(pid) => {
            let loopback_mode = match config.mode {
                CaptureMode::Include => ProcessLoopbackMode::IncludeTargetProcessTree,
                CaptureMode::Exclude => ProcessLoopbackMode::ExcludeTargetProcessTree,
            };
            activate_process_loopback_client(pid, loopback_mode)?
        }
        None => activate_master_loopback_client()?,
    };

    // Get system mix format from default render device
    let sys_format = get_system_mix_format().unwrap_or_else(|_| {
        let mut wf = WAVEFORMATEXTENSIBLE::default();
        wf.Format.wFormatTag = 0xFFFE; // WAVE_FORMAT_EXTENSIBLE
        wf.Format.nChannels = 2;
        wf.Format.nSamplesPerSec = 48000;
        wf.Format.wBitsPerSample = 32;
        wf.Format.nBlockAlign = 8;
        wf.Format.nAvgBytesPerSec = 48000 * 8;
        wf.Format.cbSize = 22;
        wf.Samples.wValidBitsPerSample = 32;
        wf.dwChannelMask = 0x3;
        wf.SubFormat = windows::core::GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);
        wf
    });

    let format_ptr = &sys_format.Format as *const WAVEFORMATEX;
    let is_master_loopback = target_pid_opt.is_none();
    let stream_flags = if is_master_loopback {
        AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
    } else {
        AUDCLNT_STREAMFLAGS_LOOPBACK
            | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
    };

    // Buffer duration: 100 milliseconds in 100ns units = 1,000,000
    let buffer_duration = 1_000_000;

    unsafe {
        let init_res = audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            stream_flags,
            buffer_duration,
            0,
            format_ptr,
            None,
        );
        init_res?;

        let event = if !is_master_loopback {
            let ev = CreateEventW(None, false, false, None)?;
            audio_client.SetEventHandle(ev)?;
            Some(ev)
        } else {
            None
        };

        let capture_client: IAudioCaptureClient = audio_client.GetService()?;
        let start_res = audio_client.Start();
        start_res?;

        let src_sample_rate = sys_format.Format.nSamplesPerSec;
        let src_channels = sys_format.Format.nChannels as usize;

        while !stop_signal.load(Ordering::Relaxed) {
            // Check if target process has exited (in single app INCLUDE mode)
            if config.mode == CaptureMode::Include {
                if let Some(h_proc) = process_handle {
                    let wait_proc = WaitForSingleObject(h_proc, 0);
                    if wait_proc.0 == 0 {
                        break;
                    }
                }
            }

            if let Some(ev) = event {
                // Wait for audio event with 50ms timeout
                let wait_res = WaitForSingleObject(ev, 50);
                if wait_res.0 != 0 && wait_res.0 != 0x102 {
                    continue;
                }
            } else {
                // Master loopback polling mode (sleep 10ms per WASAPI recommendation)
                thread::sleep(Duration::from_millis(10));
            }

            let mut packet_length = capture_client.GetNextPacketSize()?;
            if packet_length == 0 && config.silence_padding {
                let silent_frames = (config.sample_rate as usize * 10) / 1000;
                let total_samples = silent_frames * (config.channels as usize);
                let (samples_f32, samples_i16) = match config.format {
                    AudioFormat::Float32 => (vec![0.0f32; total_samples], Vec::new()),
                    AudioFormat::Pcm16 => (Vec::new(), vec![0i16; total_samples]),
                };
                let frame = AudioFrame {
                    samples_f32,
                    samples_i16,
                    channels: config.channels,
                    sample_rate: config.sample_rate,
                };
                let _ = sender.try_send(frame);
            }
            while packet_length > 0 {
                let mut p_data: *mut u8 = std::ptr::null_mut();
                let mut num_frames_read = 0u32;
                let mut flags = 0u32;

                capture_client.GetBuffer(
                    &mut p_data,
                    &mut num_frames_read,
                    &mut flags,
                    None,
                    None,
                )?;

                if num_frames_read > 0 {
                    let total_samples = (num_frames_read as usize) * src_channels;
                    let is_silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;

                    let raw_floats: Vec<f32> = if is_silent {
                        vec![0.0f32; total_samples]
                    } else if !p_data.is_null() {
                        let float_slice = std::slice::from_raw_parts(
                            p_data as *const f32,
                            total_samples,
                        );
                        float_slice.to_vec()
                    } else {
                        vec![0.0f32; total_samples]
                    };

                    // 1. Channel conversion
                    let channel_floats = if config.channels == 1 {
                        downmix_to_mono(&raw_floats, src_channels)
                    } else {
                        downmix_to_stereo(&raw_floats, src_channels)
                    };

                    // 2. Resampling if rates differ
                    let final_floats = if src_sample_rate != config.sample_rate {
                        resample_linear(&channel_floats, src_sample_rate, config.sample_rate)
                    } else {
                        channel_floats
                    };

                    // 3. Format conversion
                    let pcm16 = f32_to_pcm16(&final_floats);

                    let frame = AudioFrame {
                        samples_f32: final_floats,
                        samples_i16: pcm16,
                        channels: config.channels,
                        sample_rate: config.sample_rate,
                    };

                    let _ = sender.try_send(frame);
                }

                capture_client.ReleaseBuffer(num_frames_read)?;
                packet_length = capture_client.GetNextPacketSize()?;
            }
        }

        let _ = audio_client.Stop();
        if let Some(ev) = event {
            let _ = CloseHandle(ev);
        }

        if let Some(h_proc) = process_handle {
            let _ = CloseHandle(h_proc);
        }

        if !avrt_handle.is_invalid() {
            let _ = AvRevertMmThreadCharacteristics(avrt_handle);
        }
    }

    Ok(())
}

/// Software Audio Mixer thread: pulls audio frames from multiple active streams,
/// sums and soft-clamps the samples in real-time, and yields unified frames.
fn mixer_worker_loop(
    mut captures: Vec<AudioCapture>,
    sender: Sender<AudioFrame>,
    stop_signal: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    while !stop_signal.load(Ordering::Relaxed) {
        let mut active_frames = Vec::new();

        for cap in &mut captures {
            if let Ok(frame) = cap.receiver().recv_timeout(Duration::from_millis(30)) {
                active_frames.push(frame);
            }
        }

        if active_frames.is_empty() {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        let max_samples = active_frames
            .iter()
            .map(|f| f.samples_f32.len())
            .max()
            .unwrap_or(0);

        let mut mixed = vec![0.0f32; max_samples];
        for f in &active_frames {
            for (i, &s) in f.samples_f32.iter().enumerate() {
                mixed[i] += s;
            }
        }

        // Soft-clamp with protection against audio clipping
        for s in &mut mixed {
            *s = s.clamp(-1.0, 1.0);
        }

        let pcm16 = f32_to_pcm16(&mixed);
        let out_frame = AudioFrame {
            samples_f32: mixed,
            samples_i16: pcm16,
            channels,
            sample_rate,
        };

        let _ = sender.try_send(out_frame);
    }

    // Stop all sub-captures
    for cap in &mut captures {
        cap.stop();
    }

    Ok(())
}
