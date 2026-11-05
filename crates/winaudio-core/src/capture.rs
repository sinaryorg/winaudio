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

use crate::com::{activate_process_loopback_client, ensure_mta};
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
    ByPid(u32),
    ByName(String),
    ByWindowTitle(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Pcm16,
    Float32,
}

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub target: TargetProcess,
    pub sample_rate: u32,
    pub channels: u16,
    pub format: AudioFormat,
    pub include_tree: bool,
    pub silence_padding: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            target: TargetProcess::ByPid(0),
            sample_rate: 48000,
            channels: 2,
            format: AudioFormat::Pcm16,
            include_tree: true,
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

    pub fn include_tree(mut self, include: bool) -> Self {
        self.config.include_tree = include;
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
    config: CaptureConfig,
}

impl AudioCapture {
    pub fn builder(target: TargetProcess) -> CaptureBuilder {
        CaptureBuilder::new(target)
    }

    pub fn receiver(&self) -> &Receiver<AudioFrame> {
        &self.receiver
    }

    pub fn target_pid(&self) -> u32 {
        self.target_pid
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

    /// Records audio to a standard WAV file for a given duration.
    pub fn record_to_wav(&self, path: &str, duration: Duration) -> Result<()> {
        let spec = hound::WavSpec {
            channels: self.config.channels,
            sample_rate: self.config.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(path, spec)?;
        let start = Instant::now();

        while start.elapsed() < duration {
            if let Ok(frame) = self.receiver.recv_timeout(Duration::from_millis(100)) {
                for &sample in &frame.samples_i16 {
                    writer.write_sample(sample)?;
                }
            }
        }

        writer.finalize()?;
        Ok(())
    }

    fn resolve_target_pid(target: &TargetProcess) -> Result<u32> {
        match target {
            TargetProcess::ByPid(pid) => Ok(*pid),
            TargetProcess::ByName(name) => {
                let sessions = SessionManager::find_by_name(name)?;
                if let Some(session) = sessions.into_iter().find(|s| s.pid > 0) {
                    Ok(session.pid)
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
                    Ok(session.pid)
                } else {
                    Err(WinAudioError::ProcessNotFound(format!(
                        "No active audio session found with window title containing: {}",
                        title
                    )))
                }
            }
        }
    }

    fn start_with_config(config: CaptureConfig) -> Result<Self> {
        let target_pid = Self::resolve_target_pid(&config.target)?;

        let (sender, receiver) = bounded(128);
        let stop_signal = Arc::new(AtomicBool::new(false));

        let stop_clone = Arc::clone(&stop_signal);
        let config_clone = config.clone();

        let worker_handle = thread::spawn(move || {
            capture_worker_loop(target_pid, config_clone, sender, stop_clone)
        });

        Ok(Self {
            receiver,
            stop_signal,
            worker_handle: Some(worker_handle),
            target_pid,
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

fn capture_worker_loop(
    target_pid: u32,
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

    // Open target process handle to detect process exit
    let process_handle = unsafe {
        OpenProcess(PROCESS_SYNCHRONIZE, false, target_pid).ok()
    };

    let audio_client = activate_process_loopback_client(target_pid, config.include_tree)?;

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
    let stream_flags = AUDCLNT_STREAMFLAGS_LOOPBACK
        | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM;

    // Buffer duration: 100 milliseconds in 100ns units = 1,000,000
    let buffer_duration = 1_000_000;

    unsafe {
        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            stream_flags,
            buffer_duration,
            0,
            format_ptr,
            None,
        )?;

        let event = CreateEventW(None, false, false, None)?;
        audio_client.SetEventHandle(event)?;

        let capture_client: IAudioCaptureClient = audio_client.GetService()?;
        audio_client.Start()?;

        let src_sample_rate = sys_format.Format.nSamplesPerSec;
        let src_channels = sys_format.Format.nChannels as usize;

        while !stop_signal.load(Ordering::Relaxed) {
            // Check if target process has exited
            if let Some(h_proc) = process_handle {
                let wait_proc = WaitForSingleObject(h_proc, 0);
                if wait_proc.0 == 0 {
                    // Process terminated: exit loop
                    break;
                }
            }

            // Wait for audio event with 50ms timeout
            let wait_res = WaitForSingleObject(event, 50);
            if wait_res.0 != 0 && wait_res.0 != 0x102 {
                // Not signaled and not timeout
                continue;
            }

            let mut packet_length = capture_client.GetNextPacketSize()?;
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
        let _ = CloseHandle(event);

        if let Some(h_proc) = process_handle {
            let _ = CloseHandle(h_proc);
        }

        if !avrt_handle.is_invalid() {
            let _ = AvRevertMmThreadCharacteristics(avrt_handle);
        }
    }

    Ok(())
}
