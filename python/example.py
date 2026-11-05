"""
WinAudio Python Example
======================
Demonstrates discovering active audio sessions and capturing application audio.

Ensure winaudio.pyd is built and located in this directory:
    cargo build --release -p winaudio-python
    Copy-Item target/release/winaudio.dll -Destination python/winaudio.pyd
"""

import winaudio

def main():
    print("--- 🔊 Discovering Audio Sessions ---")
    sessions = winaudio.list_sessions()
    print(f"Total audio sessions found: {len(sessions)}\n")

    for s in sessions:
        host = f" (Host: {s.host_app_name})" if s.host_app_name else ""
        print(f"[{s.pid:<5}] {s.display_name:<20}{host:<15} | Volume: {s.volume * 100:>3.0f}% | Muted: {str(s.is_muted):<5} | State: {s.state}")

    # Example 1: Record ALL system audio for 5 seconds
    # print("\n--- Recording ALL system audio for 5 seconds ---")
    # with winaudio.Capture(all=True) as cap:
    #     cap.record_to_wav("all_sound.wav", duration_seconds=5.0)
    # print("Saved to all_sound.wav")

    # Example 2: Record specific application (e.g., Riff or Chrome)
    # print("\n--- Recording Riff audio for 5 seconds ---")
    # with winaudio.Capture(name="Riff", sample_rate=48000, channels=2) as cap:
    #     cap.record_to_wav("riff.wav", duration_seconds=5.0)
    # print("Saved to riff.wav")

if __name__ == "__main__":
    main()
