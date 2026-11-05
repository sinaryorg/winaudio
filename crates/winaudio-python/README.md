# WinAudio Python API

[![PyPI version](https://img.shields.io/pypi/v/winaudio.svg)](https://pypi.org/project/winaudio/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/sinaryorg/winaudio)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2021H2%2B%20%2F%2011-0078D6.svg)](https://github.com/sinaryorg/winaudio)

Modern, ultra-low-latency per-process Windows audio capture, session routing, and volume control library by **SINARY™**.

Powered by native Windows WASAPI Process Loopback APIs (`AUDIOCLIENT_ACTIVATION_PARAMS` / `PROCESS_LOOPBACK_MODE`).

---

## Features

- 🎯 **Per-Process Capture**: Capture clean, isolated audio from any specific running application without capturing background noise or unwanted system sounds.
- 🎚️ **Session Management**: Query active audio sessions, display names, process IDs, and inspect or modify session volume and mute states.
- 🎛️ **Multi-App Mixing & Exclusion**: Mix audio from multiple applications simultaneously or capture the entire system while excluding specific programs.
- 🔴 **Continuous Recording**: Record indefinitely with clean WAV finalization upon interrupt (`Ctrl+C` or programmatic stop).
- ⚡ **Real-Time Streaming**: Stream raw 16-bit PCM frames in real-time directly into Python (ideal for speech-to-text, AI models, DSP pipelines).
- 🦀 **Rust Core Engine**: Built on a high-performance native Rust core with zero garbage-collection jitter.

---

## Installation

```bash
pip install winaudio
```

### System Requirements

- **Operating System**: Windows 10 (Build 20348 / 21H2 or later) or Windows 11
- **Python**: 3.8, 3.9, 3.10, 3.11, 3.12, or 3.13 (64-bit)

---

## Quick Start

### 1. List Sessions & Control Volume

```python
import winaudio

# List all active audio sessions
for session in winaudio.list_sessions():
    state_str = "Active" if session.state == 1 else "Inactive"
    print(f"[{session.pid}] {session.name} (Host: {session.host_app_name}) - Vol: {session.volume * 100:.0f}% ({state_str})")

# Set volume and mute by process name
winaudio.set_volume_by_name("spotify.exe", 0.75)  # 75% volume
winaudio.set_mute_by_name("zoom.exe", True)       # Mute
```

### 2. Capture Single Application Audio

```python
import winaudio

# Capture 5 seconds from Spotify
with winaudio.Capture(name="Spotify", sample_rate=48000, channels=2) as cap:
    print(f"Recording from PID {cap.target_pid}...")
    cap.record_to_wav("spotify_clip.wav", duration_seconds=5.0)
```

### 3. Continuous Recording with Graceful Stop

Record continuously until you press `Ctrl+C`. The WAV file header is safely flushed and finalized with zero audio loss or file corruption:

```python
import winaudio

with winaudio.Capture(name="Discord") as cap:
    print("Recording live stream... Press Ctrl+C to stop.")
    try:
        # Omitting duration_seconds enables continuous recording
        cap.record_to_wav("discord_meeting.wav")
    except KeyboardInterrupt:
        print("\nRecording stopped safely!")
```

### 4. Master Loopback (All System Audio)

```python
import winaudio

# Capture all desktop audio output
with winaudio.Capture(all=True) as cap:
    cap.record_to_wav("system_sound.wav", duration_seconds=10.0)
```

### 5. Multi-App Mixing & Exclude Mode

```python
import winaudio

# Multi-App Mixing: Mix audio from Spotify and Chrome simultaneously
with winaudio.Capture(name="Spotify,chrome") as cap:
    cap.record_to_wav("music_and_browser.wav", duration_seconds=15.0)

# Exclude Mode: Record everything except games and chat
with winaudio.Capture(exclude="steam,discord") as cap:
    cap.record_to_wav("clean_desktop.wav", duration_seconds=30.0)
```

### 6. Real-Time Streaming for AI & DSP

Read raw PCM chunks in real-time (16-bit signed, mono or stereo, configurable sample rate) for Whisper, live transcription, or real-time visualization:

```python
import winaudio

with winaudio.Capture(name="chrome.exe", sample_rate=16000, channels=1) as cap:
    print("Listening to Chrome audio in real-time...")
    for _ in range(100):
        # Reads raw 16-bit PCM bytes (100ms timeout)
        frame = cap.read_frame(timeout_ms=100)
        if frame:
            # Process frame bytes with NumPy, PyTorch, Whisper, etc.
            pass
```

---

## License & Trademark

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/sinaryorg/winaudio/blob/main/LICENSE-APACHE))
- MIT License ([LICENSE-MIT](https://github.com/sinaryorg/winaudio/blob/main/LICENSE-MIT))

**Trademark Notice**: "SINARY" and the SINARY logo are trademarks of **SINARY™**.
