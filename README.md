# 🔊 WinAudio

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20(21H2%2B)%20%7C%20Windows%2011-0078D6.svg)](https://microsoft.com/windows)
[![Privacy: Local Only](https://img.shields.io/badge/privacy-local--only-success.svg)](PRIVACY.md)

**WinAudio** is a modern, high-performance Windows audio capture, monitoring, and routing toolkit by **SINARY™**, written in Rust, with native Python bindings (`pyo3`) and a standalone command-line executable.

It captures digital audio from **specific applications** (e.g., Discord, Spotify, Chrome, games) directly in user-mode using Windows WASAPI Process Loopback — **without virtual audio cables, audio repeaters, or kernel drivers**.

---

## ✨ Features

- 🎯 **Target Any Application**: Capture audio by Process ID (`--pid`), executable name (`--name spotify.exe` or `--name Riff`), or window title substring (`--window "Meeting"`).
- 🌲 **Dynamic Process Tree Resolution**: Automatically discovers child audio renderer processes (such as `msedgewebview2.exe` in WebView2 apps or `chrome.exe` audio service workers) and maps them to their host application.
- 🌐 **Master System Loopback (`--all`)**: Capture all audio playing across the entire PC with a single command.
- 🎛 **Multi-Application Mixing**: Record and mix multiple applications simultaneously into one WAV file (e.g. `--name "game.exe,discord.exe"`).
- 🚫 **Exclude Mode (`--exclude`)**: Capture all system audio *except* specific applications (ideal for streamers clipping gameplay without copyrighted music or voice chats).
- 🎚 **Audio Session & Volume Control**: Enumerate all active audio sessions, read/set per-application volume (`0.0` to `1.0`), and mute/unmute sessions.
- 🎙 **Built-in DSP Pipeline**:
  - Format conversion: IEEE 32-bit float (`f32`) to 16-bit signed integer (`i16` PCM) with sample clamping.
  - Channel downmixing: Multi-channel / stereo downmixed to mono (`-c 1`) or stereo (`-c 2`).
  - Resampling: High-speed linear resampler to convert native rates (e.g. 48 kHz $\to$ 16 kHz for Whisper / Speech-to-Text AI models).
- ⚡ **MMCSS Real-Time Scheduling**: Automatically opts capture worker threads into Windows Multimedia Class Scheduler Service (`Audio` profile) for glitch-free streaming.
- 📋 **Built-in Runnable Cheatsheet**: Run `winaudio commands` at any time to see every runnable command with copy-pasteable examples.

---

## 📁 Repository Structure

```
win_audio/
├── crates/
│   ├── winaudio-core/      # Core WASAPI engine, COM bindings, DSP, session management
│   ├── winaudio-cli/       # Standalone CLI binary (target/release/winaudio.exe)
│   └── winaudio-python/    # PyO3 C-extension module for Python
├── python/                 # Prebuilt Python extension (winaudio.pyd) & test scripts
├── winaudio.cmd            # Direct CMD launcher wrapper
├── winaudio.ps1            # Direct PowerShell launcher wrapper
├── Cargo.toml              # Root workspace manifest
└── README.md
```

---

## 💻 OS & System Requirements

- **Windows 10** Build 20348 (21H2 Server / Enterprise updates) or newer.
- **Windows 11** (all builds supported).
- **Rust Toolchain**: `stable` (1.75+ recommended).
- **Python**: 3.8+ (for Python bindings).

---

## 📦 Installation

### 1. Windows Package Manager (`winget`)
Install the standalone CLI executable directly into your PATH:
```powershell
winget install Sinary.WinAudio
```

### 2. Python Package (`pip`)
Install the official Python extension module:
```bash
pip install winaudio
```

### 3. Cargo (`crates.io`)
Add the core Rust engine to your `Cargo.toml`:
```toml
[dependencies]
winaudio-core = "0.1.0"
```

---

## 🛠 Building From Source

### 1. Build the Standalone CLI

```powershell
# Build optimized release binary
cargo build --release -p winaudio-cli

# The executable will be at:
# target\release\winaudio.exe
```

### 2. Build the Python Extension

```powershell
# Build the cdylib
cargo build --release -p winaudio-python

# Copy the DLL to python/winaudio.pyd
Copy-Item target\release\winaudio.dll -Destination python\winaudio.pyd -Force
```

### 3. Run Workspace Unit Tests

```powershell
cargo test --workspace
```

---

## 🚀 How to Use

### 1. Command-Line Interface (CLI)

You can run commands using `winaudio` (via the included `winaudio.cmd`) or directly using `.\target\release\winaudio.exe`.

#### View All Runnable Commands (Cheatsheet)
```cmd
winaudio commands
```

#### Discover Audio Sessions
```cmd
# List currently active sessions playing audio
winaudio list

# List all sessions (including inactive/closed ones)
winaudio list --all
```

#### Record Single Application
```cmd
# Record audio live continuously (press Ctrl+C anytime to stop and save)
winaudio record --name Riff

# Record for a specific duration (e.g. 10 seconds, 1 hour = 3600s, 2 hours = 7200s)
winaudio record --name chrome.exe --duration 10 --out chrome.wav
winaudio record --name Riff --duration 3600 --out 1hr_podcast.wav

# Record by Process ID
winaudio record --pid 15012 --out app.wav

# Record by window title
winaudio record --window "YouTube" --duration 15 --out video.wav
```

> **No Time Limits**: WinAudio is not restricted by time. When `--duration` is omitted, it records live continuously for 1 hour, 2 hours, or longer. Press **`Ctrl + C`** at any time to gracefully finalize and save your complete WAV recording.

#### Record ALL Audio Sources (Master System Loopback)
```cmd
# Record everything playing across the entire PC
winaudio record --all --duration 20 --out system_audio.wav
```

#### Multi-Application Mixing
```cmd
# Record and mix Riff + Chrome together
winaudio record --name "Riff,chrome" --duration 15 --out mix.wav

# Record and mix multiple apps by PIDs
winaudio record --pid 21632,15012 --duration 15 --out mix.wav
```

#### Exclude Mode (Streamer / Clipping)
```cmd
# Record all PC audio EXCEPT Spotify
winaudio record --exclude spotify.exe --duration 30 --out clean_game.wav

# Record all PC audio EXCEPT Riff and Steam
winaudio record --exclude "Riff,steam" --duration 30 --out clean_game.wav
```

#### AI / Speech-to-Text Preset (Whisper format: 16 kHz Mono)
```cmd
winaudio record --name Riff --sample-rate 16000 --channels 1 --out whisper_input.wav
```

#### Adjust Volume & Mute
```cmd
# Set Spotify volume to 50%
winaudio volume --name spotify.exe --set 0.50

# Set volume by PID to 80%
winaudio volume --pid 15012 --set 0.80

# Toggle mute state
winaudio mute --name zoom.exe

# Explicitly unmute
winaudio mute --name zoom.exe --unmute
```

---

### 2. Python API

Import `winaudio` from the `python/` folder:

```python
import sys
sys.path.insert(0, "python")
import winaudio

# 1. Enumerate active audio sessions
for session in winaudio.list_sessions():
    print(f"[{session.pid}] {session.name} (Host: {session.host_app_name}) - Volume: {session.volume * 100:.0f}% - Active: {session.state}")

# 2. Control application volume and mute
winaudio.set_volume_by_name("spotify.exe", 0.5)
winaudio.set_mute_by_name("zoom.exe", True)

# 3. Capture a single application (e.g. 10s, 1 hour = 3600s, 2 hours = 7200s)
with winaudio.Capture(name="Riff", sample_rate=48000, channels=2) as cap:
    print(f"Capturing from PID {cap.target_pid}...")
    cap.record_to_wav("riff_audio.wav", duration_seconds=10.0)

# 4. Continuous recording without time limits (runs continuously until Ctrl+C or cap.stop())
with winaudio.Capture(name="Discord") as cap:
    cap.record_to_wav("meeting_call.wav")  # Omit duration_seconds for unlimited live recording

# 5. Master Loopback: Capture ALL system audio
with winaudio.Capture(all=True) as cap:
    cap.record_to_wav("all_pc_sound.wav", duration_seconds=10.0)

# 6. Multi-App Mixing: Mix multiple apps together
with winaudio.Capture(name="Riff,chrome") as cap:
    cap.record_to_wav("mixed.wav", duration_seconds=10.0)

# 7. Exclude Mode: Capture all except specific apps
with winaudio.Capture(exclude="Riff,steam") as cap:
    cap.record_to_wav("clean_stream.wav", duration_seconds=10.0)

# 8. Real-time frame streaming (for AI / Whisper speech-to-text)
with winaudio.Capture(name="chrome.exe", sample_rate=16000, channels=1) as cap:
    for _ in range(50):
        # Reads raw 16-bit PCM bytes continuously
        chunk = cap.read_frame(timeout_ms=100)
        if chunk:
            print(f"Received audio frame: {len(chunk)} bytes")
```

---

### 3. Rust API (`winaudio-core`)

Add the local crate to your `Cargo.toml`:

```toml
[dependencies]
winaudio-core = { path = "crates/winaudio-core" }
```

```rust
use std::time::Duration;
use winaudio_core::{AudioCapture, SessionManager, TargetProcess};

fn main() -> anyhow::Result<()> {
    // 1. List active audio sessions
    let sessions = SessionManager::list_sessions()?;
    for s in sessions {
        println!("[{}] {} (Vol: {:.0}%)", s.pid, s.display_name, s.volume * 100.0);
    }

    // 2. Capture a specific application by name
    let capture = AudioCapture::builder(TargetProcess::ByName("Riff".into()))
        .sample_rate(48_000)
        .channels(2)
        .start()?;

    // 3. Record for custom duration (e.g. 10 seconds, 1 hour = 3600s, 2 hours = 7200s)
    capture.record_to_wav("output.wav", Duration::from_secs(10))?;

    // 4. Continuous recording without time limits (runs live until capture.stop() is called)
    // capture.record_to_wav_indefinite("long_session.wav")?;
    Ok(())
}
```

---

## 🏗 Architecture

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

## 📄 License & Trademark

Copyright © 2026 **SINARY™**. All rights reserved.

**SINARY™** and the SINARY logo are trademarks of SINARY.

Dual-licensed under either:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

For information on data safety, local-only processing, and privacy practices, see our [Privacy Policy](PRIVACY.md).
