# winaudio-core

[![Crates.io](https://img.shields.io/crates/v/winaudio-core.svg)](https://crates.io/crates/winaudio-core)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/sinaryorg/winaudio)
[![Documentation](https://docs.rs/winaudio-core/badge.svg)](https://docs.rs/winaudio-core)

Modern, high-performance per-process Windows audio capture, session routing, and volume management library in pure Rust by **SINARY™**.

Powered by native Windows WASAPI Process Loopback APIs (`AUDIOCLIENT_ACTIVATION_PARAMS` / `PROCESS_LOOPBACK_MODE`), requiring Windows 10 (Build 20348+) or Windows 11.

---

## Features

- 🎯 **Per-Process Loopback Capture**: Capture clean, unmixed audio from individual processes by PID or process name.
- 🎚️ **WASAPI Audio Session Management**: Enumerate active audio sessions, detect host processes, adjust volume, and set mute states.
- 🎛️ **Multi-Process Mixing & Exclusion**: Capture multiple processes simultaneously or exclude specific applications from loopback capture.
- 🔴 **Continuous & Timed Recording**: Stream directly into a WAV writer with graceful interruption (`Option<Duration>`) and proper header finalization.
- ⚡ **Low-Latency Streaming**: Channel-based audio frame extraction with SIMD-capable float-to-PCM conversion and high-quality linear resampling.

---

## Installation

Add this crate to your `Cargo.toml`:

```toml
[dependencies]
winaudio-core = "0.1.0"
```

---

## Quick Example

```rust
use std::time::Duration;
use winaudio_core::{AudioCapture, SessionManager, TargetProcess};

fn main() -> anyhow::Result<()> {
    // 1. List active audio sessions
    let sessions = SessionManager::list_sessions()?;
    for s in sessions {
        println!("[{}] {} (Vol: {:.0}%)", s.pid, s.display_name, s.volume * 100.0);
    }

    // 2. Adjust volume of an application
    SessionManager::set_volume_by_name("spotify.exe", 0.8)?;

    // 3. Capture specific application audio to a WAV file
    let capture = AudioCapture::builder(TargetProcess::ByName("Spotify".into()))
        .sample_rate(48_000)
        .channels(2)
        .start()?;

    // Record for 5 seconds
    capture.record_to_wav("spotify_clip.wav", Duration::from_secs(5))?;

    // Or record continuously until Ctrl+C / cancellation:
    // capture.record_to_wav_continuous("live.wav")?;

    Ok(())
}
```

---

## License & Trademark

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/sinaryorg/winaudio/blob/main/LICENSE-APACHE))
- MIT License ([LICENSE-MIT](https://github.com/sinaryorg/winaudio/blob/main/LICENSE-MIT))

**Trademark Notice**: "SINARY" and the SINARY logo are trademarks of **SINARY™**.
