# WinAudio Documentation

Welcome to the official **WinAudio** documentation by **SINARY™**.

WinAudio is a modern, high-performance Windows audio capture, monitoring, and routing toolkit written in Rust, featuring a standalone CLI, official Python bindings (`sinary-winaudio`), and core Rust engine crates.

It captures digital audio from **specific running applications** (e.g. Discord, Spotify, Chrome, games) directly in user-mode using Windows WASAPI Process Loopback — **without virtual audio cables, audio repeaters, or kernel drivers**.

---

## 📚 Table of Contents

- [CLI Reference Guide](CLI-Reference)
  - Discovering audio sessions (`winaudio list`)
  - Target by name, PID, or window title
  - Timed and continuous recording (`Ctrl+C` safe finalization)
  - Master loopback (`--all`), mixing, and exclude mode
  - Volume and mute control
  - Runnable command cheatsheet
- [Python API Guide (`sinary-winaudio`)](Python-API)
  - Installation via `pip`
  - Enumerating active sessions
  - Controlling application volume and mute
  - Per-process, multi-app, and exclude recording
  - Real-time 16-bit PCM streaming for Whisper and AI models
- [Rust API Guide (`winaudio-core`)](Rust-API)
  - Adding the crate dependency
  - `AudioCapture` builder and target process selection
  - Managing sessions via `SessionManager`
  - Reading real-time audio frames via channels
- [Architecture & WASAPI Deep-Dive](Architecture-and-WASAPI)
  - Windows Process Loopback APIs (`AUDIOCLIENT_ACTIVATION_PARAMS`)
  - Dynamic process tree walking for child audio renderers
  - MMCSS real-time thread scheduling
  - Real-time DSP pipeline (downmixing and resampling)

---

## 💻 Operating System Requirements

- **Windows 10**: Build 20348 (21H2 Server / Enterprise updates) or newer.
- **Windows 11**: All builds supported.
- **Architecture**: 64-bit (`x86_64` / `x64`).
- **Permissions**: Standard user permissions (Administrator not required for standard desktop audio sessions).

---

## 📦 Official Distribution Channels

- **Python (`pip`)**: `pip install sinary-winaudio` ([PyPI Project](https://pypi.org/project/sinary-winaudio/))
- **Windows Package Manager (`winget`)**: `winget install Sinary.WinAudio`
- **Scoop**: `scoop install https://raw.githubusercontent.com/sinaryorg/winaudio/main/packaging/scoop/winaudio.json`
- **Rust (`crates.io`)**: `winaudio-core = "0.1.0"` ([crates.io/crates/winaudio-core](https://crates.io/crates/winaudio-core))
- **CLI via Cargo**: `cargo install winaudio-cli` ([crates.io/crates/winaudio-cli](https://crates.io/crates/winaudio-cli))
- **GitHub Releases**: Pre-built stand-alone binaries at [sinaryorg/winaudio/releases](https://github.com/sinaryorg/winaudio/releases)

---

## 📄 License & Trademark

Copyright © 2026 **SINARY™**. All rights reserved.

"SINARY" and the SINARY logo are trademarks of **SINARY™**.  
Dual-licensed under either **MIT License** or **Apache License, Version 2.0**.
