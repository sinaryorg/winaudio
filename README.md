# 🔊 WinAudio

[![CI](https://github.com/win-audio/winaudio/actions/workflows/ci.yml/badge.svg)](https://github.com/win-audio/winaudio/actions)
[![crates.io](https://img.shields.io/crates/v/winaudio-core.svg)](https://crates.io/crates/winaudio-core)
[![PyPI](https://img.shields.io/pypi/v/winaudio.svg)](https://pypi.org/project/winaudio/)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20(21H2%2B)%20%7C%20Windows%2011-0078D6.svg)](https://microsoft.com/windows)

**WinAudio** is a modern, high-performance per-process Windows audio capture, monitoring, and routing library. It allows you to capture or control audio from a **specific application** (e.g., Discord, Spotify, Chrome, or a game) directly in user-mode **without virtual audio cables, audio repeaters, or kernel drivers**.

Built with a fast, memory-safe Rust core, official Python bindings (`pyo3`), and a standalone 1 MB command-line utility.

---

## ✨ Features

- 🎯 **Target Any Application:** Capture audio strictly by Process ID (`PID`), executable name (`spotify.exe`), or window title (`"Zoom Meeting"`).
- 🌲 **Full Process Tree Capture:** Automatically captures child processes and renderer sub-processes (essential for Chrome, Edge, and Electron apps).
- 🚫 **Zero Drivers or Virtual Cables:** Uses native Windows WASAPI Process Loopback (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`).
- 🎚 **Unified Control Surface:** Discover active audio sessions, adjust per-app master volume, and toggle mute state in one unified API.
- 🎙 **AI-Ready Audio Pipeline:** Built-in real-time channel downmixing (stereo/surround $\to$ mono) and automatic resampling (e.g. 48 kHz $\to$ 16 kHz) for direct feeding into Whisper, Gemini, or Speech-to-Text pipelines.
- ⚡ **Real-Time Thread Scheduling:** Automatically opts into Windows Multimedia Class Scheduler Service (MMCSS "Audio") for glitch-free streaming.
- 🐍 **First-Class Python Support:** Pre-compiled wheels for Python 3.8+ with native context manager support.
- 📦 **Zero-Dependency CLI:** A standalone ~1.1 MB executable (`winaudio.exe`) for scripts and terminal power users.

---

## 🚀 Quick Start

### 1. Command-Line Interface (CLI)

```bash
# List all active audio sessions with their PIDs, names, and volume
winaudio list

# Record audio from Spotify for 10 seconds into a WAV file
winaudio record --name spotify.exe --out music.wav --duration 10

# Capture audio from Discord downmixed to 16kHz mono (ideal for Whisper AI)
winaudio record --name discord.exe --sample-rate 16000 --channels 1 --out meeting.wav

# MASTER SYSTEM LOOPBACK: Record ALL audio sources played across the system
winaudio record --all --out system_audio.wav --duration 15

# MULTI-APP MIXING: Record and mix multiple specific apps together (e.g. game + Discord)
winaudio record --name "game.exe,discord.exe" --out stream_mix.wav --duration 20

# MULTI-APP EXCLUSION: Record all PC/game audio EXCEPT Spotify and Discord
winaudio record --exclude "spotify.exe,discord.exe" --out gameplay_clean.wav --duration 30

# Adjust application volume
winaudio volume --name spotify.exe --set 0.65

# Mute or unmute an application
winaudio mute --name zoom.exe
winaudio mute --name zoom.exe --unmute
```

### 2. Python (`pip install winaudio`)

```python
import winaudio

# 1. Discover all active audio sessions
for session in winaudio.list_sessions():
    print(f"[{session.pid}] {session.name} - Volume: {session.volume * 100:.0f}%")

# 2. Adjust volume programmatically
winaudio.set_volume_by_name("spotify.exe", 0.5)

# 3. Record application audio directly to WAV
with winaudio.Capture(name="discord.exe", sample_rate=16000, channels=1) as cap:
    print(f"Recording from PID {cap.target_pid}...")
    cap.record_to_wav("call.wav", duration_seconds=5.0)

# 4. Master Loopback: Record ALL system audio
with winaudio.Capture(all=True) as cap:
    cap.record_to_wav("all_sound.wav", duration_seconds=10.0)

# 5. Multi-App Mixing: Record multiple apps simultaneously
with winaudio.Capture(name="game.exe,discord.exe") as cap:
    cap.record_to_wav("multi_app_mix.wav", duration_seconds=10.0)

# 6. Exclude Mode: Record everything EXCEPT background music/voice
with winaudio.Capture(exclude="spotify.exe,discord.exe") as cap:
    cap.record_to_wav("clean_gameplay.wav", duration_seconds=10.0)

# 7. Stream real-time audio frames (e.g., for local AI / Whisper streaming)
with winaudio.Capture(name="chrome.exe", sample_rate=16000, channels=1) as cap:
    while True:
        frame_bytes = cap.read_frame(timeout_ms=100)
        if frame_bytes:
            # feed 16-bit PCM bytes into speech recognition engine
            process_audio_chunk(frame_bytes)
```

### 3. Rust (`Cargo.toml`)

```toml
[dependencies]
winaudio-core = "0.1"
```

```rust
use std::time::Duration;
use winaudio_core::{AudioCapture, TargetProcess, SessionManager};

fn main() -> anyhow::Result<()> {
    // List audio sessions
    let sessions = SessionManager::list_sessions()?;
    for s in sessions {
        println!("[{}] {} (Vol: {:.0}%)", s.pid, s.process_name, s.volume * 100.0);
    }

    // Capture Spotify audio
    let capture = AudioCapture::builder(TargetProcess::ByName("spotify.exe".into()))
        .sample_rate(48_000)
        .channels(2)
        .start()?;

    // Record for 5 seconds
    capture.record_to_wav("output.wav", Duration::from_secs(5))?;
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

## 💻 OS Requirements

- **Windows 10** Build 20348 (21H2 Server / Enterprise updates) or newer.
- **Windows 11** (all builds supported, including 21H2, 22H2, 23H2, 24H2).
- Supported architectures: `x86_64` (Intel/AMD) and `aarch64` (ARM64 Windows Copilot+ PCs).

---

## 🛠 Building From Source

```bash
# Clone the repository
git clone https://github.com/win-audio/winaudio.git
cd winaudio

# Build all workspace crates
cargo build --release

# Run the standalone CLI
./target/release/winaudio.exe list

# Run unit tests
cargo test --workspace
```

---

## 📄 License

Dual-licensed under either:
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
* MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
