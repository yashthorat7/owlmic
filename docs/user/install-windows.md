# Installing Owlmic on Windows

Owlmic for PC runs as a lightweight, per-user background application in your Windows system tray. It installs virtual drivers that allow any application (Zoom, Microsoft Teams, Discord, OBS, Google Meet, Skype) to pick up your phone as an everyday microphone and webcam.

---

## 1. System Requirements

- **Operating System**: Windows 10 (64-bit, version 1903+) or Windows 11 (all versions).
- **Architecture**: x86_64 (64-bit Intel / AMD).
- **RAM**: ~15 MB idle footprint.
- **Privileges**: Standard user account for running Owlmic. Administrator rights are required once during setup to install the virtual audio and camera drivers.

---

## 2. Download and Installation

1. **Download the Setup Package**:
   Download the latest `Owlmic-Setup-1.0.0.exe` installer from [GitHub Releases](https://github.com/diveshpatil9104/owlmic/releases) or the official website at [owlmic.app](https://owlmic.app).

2. **Run the Installer**:
   - Double-click `Owlmic-Setup-1.0.0.exe`.
   - Accept the license agreement.
   - Choose whether Owlmic should launch automatically when you log in to Windows (recommended).
   - Click **Install**.

3. **Driver Installation Prompt**:
   - During installation, Owlmic will configure the **Owlmic Mic** audio endpoint and the **Owlmic Cam** virtual camera.
   - If prompted by Windows User Account Control (UAC), click **Yes** to allow `setup-audio-device.ps1` to configure the virtual audio endpoint and Windows Firewall rules.
   - When setup completes, click **Finish**. Owlmic will launch silently into your taskbar system tray.

---

## 3. What Gets Installed

Owlmic installs files into `%LOCALAPPDATA%\Programs\Owlmic`:
- `owlmic.exe`: The primary background application and tray coordinator.
- `owlmic_vcam.dll`: High-performance Media Foundation virtual camera driver for Windows 11.
- `softcam.dll`: DirectShow virtual camera driver filter for Windows 10 compatibility.
- `setup-audio-device.ps1`: Automated endpoint management and repair script.
- `driver/`: Virtual audio cable driver files (installed by Owlmic).

Settings and pairing tokens are stored securely in:
- `%APPDATA%\Owlmic\owlmic.json` (device pairing database and settings).

---

## 4. Verifying Installation

1. Look for the **Owl icon** in your Windows notification area (system tray, next to the clock).
2. Right-click or left-click the Owl icon:
   - The Owlmic tray panel will slide open above the taskbar.
   - You should see tiles for **Owlmic Mic**, **Owlmic Cam**, and **Speaker**.
3. Open Windows Settings (`Win + I`) -> **System** -> **Sound**:
   - Under **Input**, verify that **Owlmic Mic** is listed as an available recording device.
4. Open the Windows **Camera** app:
   - Verify that **Owlmic Cam** is available in your camera switcher.

If either device shows an alert, see [`troubleshooting.md`](troubleshooting.md) or click the **Repair** button in the Owlmic tray panel.
