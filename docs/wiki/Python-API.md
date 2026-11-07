# Python API Guide (`sinary-winaudio`)

The official Python package **`sinary-winaudio`** provides native CPython bindings (via PyO3) to the high-performance Rust WASAPI engine.

---

## 1. Installation

```bash
pip install sinary-winaudio
```

### Import Module
In your Python code, import `winaudio`:
```python
import winaudio
```

---

## 2. Session Discovery & Volume Control

### List Active Sessions
```python
import winaudio

for session in winaudio.list_sessions():
    state_str = "Active" if session.state == 1 else "Inactive"
    print(f"[{session.pid}] {session.name} (Host: {session.host_app_name}) - Vol: {session.volume * 100:.0f}% ({state_str})")
```

**`AudioSession` Object Attributes:**
- `pid`: Integer process ID.
- `name`: String executable name (e.g. `spotify.exe`).
- `host_app_name`: String host application name.
- `display_name`: String session display title.
- `volume`: Float between `0.0` and `1.0`.
- `is_muted`: Boolean (`True` if muted).
- `state`: Integer session state (`1` = Active, `0` = Inactive, `2` = Expired).

### Set Application Volume and Mute
```python
import winaudio

# Set volume (0.0 to 1.0)
winaudio.set_volume_by_name("spotify.exe", 0.8)
winaudio.set_volume_by_pid(1234, 0.5)

# Mute / Unmute
winaudio.set_mute_by_name("zoom.exe", True)   # Mute
winaudio.set_mute_by_name("zoom.exe", False)  # Unmute
winaudio.set_mute_by_pid(1234, False)
```

---

## 3. Audio Capture (`winaudio.Capture`)

### Constructor Parameters
```python
winaudio.Capture(
    pid=None,            # int: Target process ID
    name=None,           # str: Target executable name (or comma-separated names)
    window=None,         # str: Window title substring
    all=False,           # bool: If True, captures all system audio
    exclude=None,        # str: Comma-separated names to exclude
    sample_rate=48000,   # int: Output sample rate in Hz (e.g. 16000, 44100, 48000)
    channels=2,          # int: 1 for mono, 2 for stereo
    include_tree=True    # bool: Include child audio renderer processes
)
```

---

## 4. Recording Examples

### Per-Process Capture (Timed)
```python
import winaudio

with winaudio.Capture(name="Spotify", sample_rate=48000, channels=2) as cap:
    print(f"Recording from PID {cap.target_pid}...")
    cap.record_to_wav("spotify_track.wav", duration_seconds=10.0)
```

### Continuous Live Recording with `Ctrl+C` Stop
Omitting `duration_seconds` records indefinitely. Handling `KeyboardInterrupt` ensures the WAV header is safely finalized:

```python
import winaudio

with winaudio.Capture(name="Discord") as cap:
    print("Recording live stream... Press Ctrl+C to stop.")
    try:
        cap.record_to_wav("live_meeting.wav")
    except KeyboardInterrupt:
        print("\nRecording stopped safely!")
```

### Master Loopback (All Desktop Sounds)
```python
import winaudio

with winaudio.Capture(all=True) as cap:
    cap.record_to_wav("all_pc_sound.wav", duration_seconds=15.0)
```

### Multi-Application Mixing
Mix audio from multiple applications simultaneously into one WAV file:
```python
import winaudio

with winaudio.Capture(name="Spotify,chrome") as cap:
    cap.record_to_wav("mixed_audio.wav", duration_seconds=20.0)
```

### Exclude Mode
Capture all audio playing on your PC *except* specific applications:
```python
import winaudio

with winaudio.Capture(exclude="steam,discord") as cap:
    cap.record_to_wav("clean_stream.wav", duration_seconds=30.0)
```

---

## 5. Real-Time Streaming for Whisper / AI / DSP

`cap.read_frame(timeout_ms=100)` returns raw 16-bit PCM byte chunks as audio arrives. Ideal for passing real-time audio into speech recognition or neural networks:

```python
import winaudio

# Configure 16 kHz Mono (Standard for OpenAI Whisper & Speech AI)
with winaudio.Capture(name="chrome.exe", sample_rate=16000, channels=1) as cap:
    print("Streaming real-time 16 kHz PCM frames...")
    
    for _ in range(100):
        # Reads raw 16-bit signed PCM bytes
        pcm_bytes = cap.read_frame(timeout_ms=100)
        if pcm_bytes:
            print(f"Received frame: {len(pcm_bytes)} bytes")
            # Example: pass pcm_bytes to numpy or your STT model:
            # audio_array = np.frombuffer(pcm_bytes, dtype=np.int16)
```
