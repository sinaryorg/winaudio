# winaudio-cli (`winaudio`)

[![Crates.io](https://img.shields.io/crates/v/winaudio-cli.svg)](https://crates.io/crates/winaudio-cli)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/sinaryorg/winaudio)

Standalone command-line tool for per-process Windows audio capture, monitoring, session routing, and volume control by **SINARY™**.

Powered by native Windows WASAPI Process Loopback APIs (`AUDIOCLIENT_ACTIVATION_PARAMS` / `PROCESS_LOOPBACK_MODE`), requiring Windows 10 (Build 20348+) or Windows 11.

---

## Installation via Cargo

```bash
cargo install winaudio-cli
```

*(Installs the `winaudio` executable into your Cargo bin path)*

---

## Usage

```cmd
# Discover active audio sessions
winaudio list

# Record Spotify audio for 10 seconds to WAV
winaudio record --name spotify.exe --duration 10 -o spotify.wav

# Continuous live recording until Ctrl+C
winaudio record --name discord.exe -o live_meeting.wav

# Master loopback: Record ALL system audio
winaudio record --all --duration 15 -o all_system_sound.wav

# Multi-app mixing: Record Spotify and Chrome simultaneously
winaudio record --name "spotify.exe,chrome.exe" -o mixed.wav

# Exclude mode: Record everything except games and chat
winaudio record --exclude "steam.exe,discord.exe" -o clean_stream.wav

# Adjust application volume
winaudio volume set --name spotify.exe 0.8
winaudio volume mute --name zoom.exe true

# View complete runnable cheatsheet
winaudio commands
```

---

## License & Trademark

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/sinaryorg/winaudio/blob/main/LICENSE-APACHE))
- MIT License ([LICENSE-MIT](https://github.com/sinaryorg/winaudio/blob/main/LICENSE-MIT))

**Trademark Notice**: "SINARY" and the SINARY logo are trademarks of **SINARY™**.
