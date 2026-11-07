# Architecture & WASAPI Deep-Dive

WinAudio implements user-mode audio capture and session management using native Windows Core Audio APIs. It requires **no virtual audio cables, audio repeaters, or kernel drivers**.

---

## 1. High-Level Architecture

```
                    ┌──────────────────────────────────────────────┐
                    │      winaudio CLI / Python / Rust API        │
                    └──────────────────────┬───────────────────────┘
                                           │
                    ┌──────────────────────▼───────────────────────┐
                    │                winaudio-core                 │
                    ├──────────────────────┬───────────────────────┤
                    │   Session Manager    │ Process Loopback Engine│
                    │ (IAudioSessionMgr2)  │ (AUDIOCLIENT_ACTIVATION)
                    ├──────────────────────┼───────────────────────┤
                    │ Process-Tree Walker  │ Multi-Stream Mixer    │
                    │(CreateToolhelp32Snap)│   (Bounded Channels)  │
                    ├──────────────────────┼───────────────────────┤
                    │ Volume & Mute Control│ Real-Time Ring Buffer │
                    │ (ISimpleAudioVolume) │   + MMCSS Scheduler   │
                    └──────────────────────┴───────────┬───────────┘
                                                       │
                                      ┌─────────────────▼───────────┐
                                      │  DSP Pipeline: Resampling   │
                                      │  & Mono/Stereo Downmixing   │
                                      └─────────────────┬───────────┘
                                                        │
                                      ┌─────────────────▼───────────┐
                                      │ Windows WASAPI Audio Engine │
                                      └─────────────────────────────┘
```

---

## 2. Windows WASAPI Process Loopback

Traditional Windows audio capture (WASAPI Loopback) captures all audio rendered on a specific playback device (master output). 

Starting with Windows 10 Build 20348 (and Windows 11), Microsoft introduced **Per-Process Loopback Activation**:
- Interface: `IActivateAudioInterfaceAsync`
- Parameter structure: `AUDIOCLIENT_ACTIVATION_PARAMS`
- Activation type: `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`
- Loopback modes:
  - `PROCESS_LOOPBACK_MODE_INCLUDE_PROCESS_TREE`: Captures the specified process ID and all its child processes.
  - `PROCESS_LOOPBACK_MODE_EXCLUDE_PROCESS_TREE`: Captures the system audio while excluding the target process and its children.

---

## 3. Dynamic Process Tree Resolution

Many modern Windows applications run multi-process architectures where the UI process is not the process outputting audio:
- **Electron applications** (Discord, Slack, Spotify): The main executable spawns child renderer processes.
- **WebView2 applications**: The host application hosts `msedgewebview2.exe` child processes that render web content and audio.
- **Chromium / Edge**: Dedicated `utility --type=audio` processes handle audio mixing.

WinAudio resolves these relationships dynamically using native Windows toolhelp snapshots (`CreateToolhelp32Snapshot`). When targeting an application by name (e.g. `Riff` or `spotify`), WinAudio inspects the process hierarchy and automatically routes capture to the appropriate audio-generating process.

---

## 4. MMCSS Real-Time Scheduling

Audio buffer underruns cause audible clicks, pops, or dropped frames. To prevent this, WinAudio's worker threads register with the Windows Multimedia Class Scheduler Service (MMCSS):
```rust
AvSetMmThreadCharacteristicsW(w!("Audio"), &mut task_index);
```
This informs the Windows NT kernel scheduler that the capture thread processes time-critical multimedia data, granting it elevated priority over background tasks.

---

## 5. DSP Pipeline

WASAPI natively delivers audio in IEEE 32-bit floating point format (`f32`) at the audio hardware's native sample rate (typically 48,000 Hz or 44,100 Hz, 2 channels).

WinAudio's DSP pipeline converts this stream on the fly:
1. **Downmixing**:
   - Stereo to Mono: $M = \frac{L + R}{2}$
   - Multi-channel downmixing with gain normalization.
2. **Resampling**:
   - High-speed linear resampler to convert between sample rates (e.g. 48 kHz $\to$ 16 kHz for Whisper / Speech-to-Text).
3. **Format Quantization**:
   - IEEE 32-bit float (`[-1.0, 1.0]`) converted to 16-bit signed integer (`i16` PCM, `[-32768, 32767]`) with hard sample clamping to avoid distortion.
4. **WAV File Writing**:
   - Uses `hound` to write standard RIFF/WAVE headers. On interruption (`Ctrl+C` or duration completion), the header byte count is finalized cleanly, producing a playable WAV file without corruption.
