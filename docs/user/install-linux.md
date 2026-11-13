# Installing Owlmic on Linux

Owlmic for PC runs as a lightweight, per-user background application in your desktop system tray. It integrates with **PipeWire** or **PulseAudio** for microphone input and speaker loopback, and uses **v4l2loopback** to provide a standard `/dev/video*` camera endpoint for any Linux application (Zoom, Discord, OBS, Google Meet, Firefox, Chromium).

---

## 1. System Requirements

- **Operating System**: Linux (Ubuntu 22.04+, Debian 12+, Fedora 38+, Arch Linux, or any modern distribution).
- **Architecture**: x86_64.
- **Audio Server**: PipeWire (recommended) or PulseAudio.
- **Virtual Camera**: `v4l2loopback` kernel module (optional, required only for video webcam).
- **RAM**: ~15 MB idle footprint.

---

## 2. Prerequisites & Driver Setup

### 2.1 Audio Setup (PipeWire / PulseAudio)
Owlmic creates a userspace virtual microphone source (`Owlmic Mic`) using standard sound server protocols. **Zero kernel audio drivers are required.**

Ensure PipeWire or PulseAudio is running:
```bash
# Verify PipeWire
pw-cli info 0

# Or verify PulseAudio
pactl info
```

### 2.2 Virtual Camera Setup (v4l2loopback)
To expose your phone's camera feed to desktop applications:

1. Install `v4l2loopback`:
   ```bash
   # Debian / Ubuntu
   sudo apt install v4l2loopback-dkms v4l2loopback-utils

   # Fedora
   sudo dnf install v4l2loopback

   # Arch Linux
   sudo pacman -S v4l2loopback-dkms
   ```

2. Load the module with the **Owlmic Cam** label:
   ```bash
   sudo modprobe v4l2loopback devices=1 video_nr=10 card_label="Owlmic Cam" exclusive_caps=1
   ```

3. To persist across reboots, add to `/etc/modules-load.d/owlmic.conf`:
   ```text
   v4l2loopback
   ```
   and add options to `/etc/modprobe.d/owlmic.conf`:
   ```text
   options v4l2loopback devices=1 video_nr=10 card_label="Owlmic Cam" exclusive_caps=1
   ```

---

## 3. Installation & Service Management

### 3.1 Binary Installation
Place `owlmic` into `~/.local/bin/` and ensure it is executable:
```bash
chmod +x ~/.local/bin/owlmic
```

### 3.2 Running as a Systemd User Service
Enable and start Owlmic at desktop login:
```bash
mkdir -p ~/.config/systemd/user
cp pc/installer/linux/owlmic.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now owlmic.service
```

---

## 4. Where Files & Settings Live

Settings and pairing tokens are stored in:
- `$XDG_CONFIG_HOME/owlmic/owlmic.json` (defaults to `~/.config/owlmic/owlmic.json`).

---

## 5. Verifying Installation

1. **System Tray**:
   - Look for the Owl icon in your system tray (GNOME AppIndicator, KDE StatusNotifierItem, Waybar, Polybar).
2. **Audio Input**:
   ```bash
   pactl list sources short | grep -i owlmic
   ```
   Verify that **Owlmic Mic** is listed as an audio source.
3. **Video Camera**:
   ```bash
   v4l2-ctl --list-devices
   ```
   Verify that **Owlmic Cam** is listed.

If any endpoint shows an issue, see [`troubleshooting.md`](troubleshooting.md).
