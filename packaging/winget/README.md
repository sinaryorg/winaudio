# WinAudio Winget Manifests

This directory contains the official Windows Package Manager (`winget`) manifests for **WinAudio** (`Sinary.WinAudio`).

## Manifest Files

- `manifests/s/Sinary/WinAudio/0.1.0/Sinary.WinAudio.yaml`: Version manifest
- `manifests/s/Sinary/WinAudio/0.1.0/Sinary.WinAudio.installer.yaml`: Portable installer pointing to the GitHub Release zip
- `manifests/s/Sinary/WinAudio/0.1.0/Sinary.WinAudio.locale.en-US.yaml`: Metadata, copyright, description, and tags

---

## 1. Local Validation

To validate the manifest format against Microsoft's schema:

```powershell
winget validate --manifest packaging\winget\manifests\s\Sinary\WinAudio\0.1.0
```

---

## 2. Test Local Installation

Once the release archive `winaudio-v0.1.0-windows-x86_64.zip` is available at `https://github.com/sinaryorg/winaudio/releases/download/v0.1.0/winaudio-v0.1.0-windows-x86_64.zip`, you can test local installation:

```powershell
winget install --manifest packaging\winget\manifests\s\Sinary\WinAudio\0.1.0\Sinary.WinAudio.yaml
```

This will automatically extract `winaudio.exe` and place it in the user's `PATH` as `winaudio`.

---

## 3. Submitting to the Official Microsoft Winget Repository

There are two easy methods to submit WinAudio to `microsoft/winget-pkgs`:

### Option A: Using `wingetcreate` (Automated & Recommended)

1. Install `wingetcreate`:
   ```powershell
   winget install Microsoft.WingetCreate
   ```

2. Submit the release archive:
   ```powershell
   wingetcreate submit https://github.com/sinaryorg/winaudio/releases/download/v0.1.0/winaudio-v0.1.0-windows-x86_64.zip
   ```
   *Follow the interactive prompt or authenticate with your GitHub account. `wingetcreate` will automatically fork `microsoft/winget-pkgs`, commit the manifest, and open the official pull request.*

### Option B: Manual GitHub Pull Request

1. Fork the official repository: [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs).
2. Clone your fork locally.
3. Copy the folder `packaging/winget/manifests/s/Sinary/WinAudio/0.1.0` into `manifests/s/Sinary/WinAudio/0.1.0` in your fork.
4. Commit, push, and open a Pull Request to `microsoft:master`.
5. Microsoft's automated CI will validate the package, run smoke tests, and merge the PR.
6. Once merged, anyone on Windows can install WinAudio via:
   ```powershell
   winget install Sinary.WinAudio
   ```
