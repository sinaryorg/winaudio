# CLI Reference Guide

The standalone `winaudio` CLI executable provides per-process audio capture, session routing, monitoring, and volume control.

---

## 1. Discover Active Audio Sessions

List all running applications currently registered with Windows WASAPI audio sessions:

```cmd
winaudio list
```

**Output Details:**
- **PID**: Process ID of the audio-playing process.
- **Process Name**: Executable name (e.g. `spotify.exe`, `chrome.exe`).
- **Host App**: Host executable (e.g. `msedgewebview2.exe` mapped to host application).
- **Display Name / Window Title**: Window title or application name.
- **Volume & Mute**: Current volume percentage and mute status.
- **State**: `Active`, `Inactive`, or `Expired`.

---

## 2. Audio Capture & Recording (`winaudio record`)

Capture audio from specific processes, multiple applications, or the entire desktop into a standard 16-bit PCM WAV file.

### Basic Syntax
```cmd
winaudio record [TARGET_OPTIONS] [DSP_OPTIONS] -o <OUTPUT.WAV>
```

### Target Selection Options (Choose one)
- `--name <NAME>`: Target by executable name (e.g. `--name spotify.exe` or `--name Riff`).
- `--pid <PID>`: Target by numerical process ID (e.g. `--pid 12344`).
- `--window <TITLE>`: Target by window title substring (e.g. `--window "Meeting"`).
- `--all`: Capture all audio playing across the entire PC (Master Loopback).
- `--exclude <NAMES>`: Capture all system audio *except* specific applications (e.g. `--exclude "steam.exe,discord.exe"`).

### Multi-Application Mixing
Provide comma-separated names to capture and mix multiple apps simultaneously:
```cmd
winaudio record --name "spotify.exe,chrome.exe" -o mixed_audio.wav
```

### Recording Duration & Continuous Mode
- `--duration <SECONDS>`: Record for a specified number of seconds:
  ```cmd
  winaudio record --name spotify.exe --duration 10 -o spotify_10s.wav
  ```
- **Continuous Recording (Omit `--duration`)**:
  If `--duration` is omitted, `winaudio` records indefinitely with a live timer. When you press `Ctrl+C`, the capture thread stops cleanly and **finalizes the WAV header**:
  ```cmd
  winaudio record --name discord.exe -o live_meeting.wav
  # Press Ctrl+C at any time to safely save and exit
  ```

### DSP & Audio Format Flags
- `-o, --output <PATH>`: Destination WAV file path (defaults to `output.wav`).
- `-s, --sample-rate <HZ>`: Output sample rate in Hz (e.g. `16000`, `44100`, `48000`). Defaults to `48000`.
- `-c, --channels <COUNT>`: Channel count: `1` for mono, `2` for stereo. Defaults to `2`.
- `--no-tree`: Disables dynamic child process tree walking (only captures the exact PID/process specified).

---

## 3. Volume & Mute Control (`winaudio volume`)

Inspect and modify audio session volume and mute states without opening Windows Sound Settings.

### Set Volume
Set volume level as a float between `0.0` (silent) and `1.0` (100%):
```cmd
# Set volume by process name
winaudio volume set --name spotify.exe 0.75

# Set volume by PID
winaudio volume set --pid 8420 0.50
```

### Toggle Mute
```cmd
# Mute an application
winaudio volume mute --name zoom.exe true

# Unmute an application
winaudio volume mute --name zoom.exe false
```

---

## 4. Built-In Runnable Cheatsheet

Display all available runnable commands with copy-pasteable examples directly in your terminal:
```cmd
winaudio commands
```
