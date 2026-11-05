# 🔒 Privacy Policy

**Effective Date:** November 6, 2026  
**Product:** WinAudio  
**Trademark:** SINARY™  

---

## 1. Overview & Commitment

**WinAudio** is an open-source, local-first per-process Windows audio capture, monitoring, and routing utility provided by **SINARY™**.

Your privacy is paramount. WinAudio is built with a strict **zero-telemetry, zero-cloud, local-only** design philosophy. We do not collect, store, transmit, or monetize any of your audio data, process information, or personal identifiers.

---

## 2. Local-Only Processing (No Cloud / No Network Transmission)

- **100% Offline Execution:** WinAudio operates entirely offline on your local Windows system.
- **No Network Requests:** WinAudio contains zero network calls, telemetry libraries, tracking beacons, analytics scripts, or remote logging services.
- **Audio Stored Locally Only:** Audio recordings created using `winaudio record` or the Python API are saved strictly to the file path you specify on your local drive (e.g. `recorded_audio.wav`). At no point is audio data uploaded or streamed over the internet.

---

## 3. Scope of Audio Capture

- **Render Loopback Only:** WinAudio exclusively captures digital audio rendered to Windows audio endpoints by user applications (via native Windows WASAPI Process Loopback).
- **No Microphone Access:** WinAudio does **not** access, listen to, or record microphone inputs, headsets, webcam inputs, or ambient room audio.
- **Target Isolation:** WinAudio isolates only the application(s) you explicitly target. Unrelated applications and background system processes can be excluded or ignored based on your parameters.

---

## 4. System Inspection & Memory

To allow you to target applications by name, PID, or window title, WinAudio queries Windows system APIs (`CreateToolhelp32Snapshot`, `IAudioSessionManager2`, and `EnumWindows`).
- This information is processed **strictly in volatile system memory (RAM)**.
- It is used solely to match your specified target application and resolve child worker processes (e.g., mapping `msedgewebview2.exe` to a host application).
- None of this process information is logged to persistent disk files or transmitted externally.

---

## 5. Security & Transparency

- **User-Mode Only:** WinAudio runs entirely in standard Windows user-mode without requiring kernel drivers, virtual audio cable drivers, or elevated administrator privileges.
- **Open Source:** The complete source code of WinAudio is publicly auditable and inspectable in this repository.

---

## 6. Trademark & Copyright

Copyright © 2026 **SINARY™**. All rights reserved.

**SINARY™** and the SINARY logo are trademarks of SINARY. All other trademarks, product names, and company logos cited herein are the property of their respective owners.

For licensing terms, please refer to [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
