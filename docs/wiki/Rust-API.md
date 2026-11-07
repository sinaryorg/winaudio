# Rust API Guide (`winaudio-core`)

The `winaudio-core` crate provides the native Rust engine for per-process Windows audio capture, session routing, and volume management.

---

## 1. Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
winaudio-core = "0.1.0"
```

---

## 2. Audio Session Management

```rust
use winaudio_core::SessionManager;

fn main() -> anyhow::Result<()> {
    // 1. Enumerate active audio sessions
    let sessions = SessionManager::list_sessions()?;
    for s in sessions {
        println!(
            "[{}] {} (Host: {}) - Volume: {:.0}% - Muted: {}",
            s.pid,
            s.display_name,
            s.host_app_name,
            s.volume * 100.0,
            s.is_muted
        );
    }

    // 2. Adjust volume and mute states
    SessionManager::set_volume_by_name("spotify.exe", 0.75)?;
    SessionManager::set_mute_by_name("zoom.exe", true)?;

    Ok(())
}
```

---

## 3. Audio Capture (`AudioCapture`)

Capture audio using the fluent builder pattern.

### Builder Configuration
```rust
use std::time::Duration;
use winaudio_core::{AudioCapture, TargetProcess};

fn main() -> anyhow::Result<()> {
    // Target a process by executable name
    let capture = AudioCapture::builder(TargetProcess::ByName("spotify.exe".into()))
        .sample_rate(48_000)   // 48 kHz
        .channels(2)          // Stereo
        .include_tree(true)   // Include child audio renderers
        .start()?;

    // Record for 10 seconds to a WAV file
    capture.record_to_wav("spotify_audio.wav", Duration::from_secs(10))?;

    Ok(())
}
```

---

## 4. Target Selection Variants (`TargetProcess`)

```rust
use winaudio_core::TargetProcess;

// 1. Target by process name
let target = TargetProcess::ByName("discord.exe".into());

// 2. Target by PID
let target = TargetProcess::ByPid(10488);

// 3. Target by window title substring
let target = TargetProcess::ByWindow("Google Chrome".into());

// 4. Capture all system audio (Master Loopback)
let target = TargetProcess::All;

// 5. Exclude specific applications
let target = TargetProcess::Exclude(vec![
    TargetProcess::ByName("steam.exe".into()),
    TargetProcess::ByName("discord.exe".into()),
]);

// 6. Mix multiple applications
let target = TargetProcess::Mix(vec![
    TargetProcess::ByName("game.exe".into()),
    TargetProcess::ByName("spotify.exe".into()),
]);
```

---

## 5. Continuous Recording & Graceful Shutdown

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use winaudio_core::{AudioCapture, TargetProcess};

fn main() -> anyhow::Result<()> {
    let capture = AudioCapture::builder(TargetProcess::ByName("discord.exe".into()))
        .sample_rate(48_000)
        .channels(2)
        .start()?;

    // Record continuously until capture.stop() is called
    // (WAV header is finalized upon termination)
    capture.record_to_wav_continuous("continuous_meeting.wav")?;

    Ok(())
}
```

---

## 6. Real-Time Audio Chunk Streaming

Read real-time float/PCM audio buffers directly through a crossbeam receiver channel:

```rust
use std::time::Duration;
use winaudio_core::{AudioCapture, TargetProcess};

fn main() -> anyhow::Result<()> {
    let capture = AudioCapture::builder(TargetProcess::ByName("chrome.exe".into()))
        .sample_rate(16_000)
        .channels(1)
        .start()?;

    let rx = capture.receiver();

    for _ in 0..100 {
        if let Ok(frame) = rx.recv_timeout(Duration::from_millis(100)) {
            // frame: AudioFrame { data: Vec<f32>, channels: u16, sample_rate: u32 }
            println!("Received {} audio samples", frame.data.len());
        }
    }

    capture.stop();
    Ok(())
}
```
