# Owlmic

<p align="center">
  <a href="https://github.com/diveshpatil9104/owlmic/releases"><img src="https://img.shields.io/github/v/release/diveshpatil9104/owlmic?style=for-the-badge&color=FFFFFF&label=Download%20v1.0.0" alt="Download"></a>
  <a href="https://github.com/diveshpatil9104/owlmic/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/diveshpatil9104/owlmic/ci.yml?branch=main&style=for-the-badge&label=CI" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green?style=for-the-badge" alt="License: MIT"></a>
  <a href="https://github.com/diveshpatil9104/owlmic/discussions"><img src="https://img.shields.io/badge/Discussions-Join-blue?style=for-the-badge&logo=github" alt="Discussions"></a>
  <a href="https://github.com/diveshpatil9104/owlmic/stargazers"><img src="https://img.shields.io/github/stars/diveshpatil9104/owlmic?style=for-the-badge&color=FFD60A&logo=github" alt="Stars"></a>
</p>

<p align="center"><strong>Fast. Quick. Lightweight. Seamless.</strong><br>
Your phone is your PC's microphone, camera, and speaker over four automatic connection levels.</p>

---

## Overview

Owlmic turns your Android smartphone into your Windows PC's studio-grade **microphone**, HD **webcam**, and remote **speaker**. Meeting and streaming apps (Discord, Zoom, Microsoft Teams, OBS Studio, Google Meet) detect **Owlmic Mic** and **Owlmic Cam** as standard system hardware.

```
       ┌────────────────────────────────────────────────────────┐
       │                 ANDROID CLIENT APP                     │
       │  Jetpack Compose UI (OLED Black) ──► AppHub            │
       │  OwlmicService (Foreground Service)                    │
       │   ├─ SettingsHub (Versioned store)                     │
       │   ├─ MediaHub (AAudio capture, CameraX, H.264, Audio)  │
       │   └─ LinkHub (Transporter: ADB, Tether, Wi-Fi, BT)     │
       └───────────────────────────┬────────────────────────────┘
                                   │
  4 Connection Levels:             │ Level 1: USB Debugging (TCP :7653 via adb reverse)
                                   │ Level 2: USB Tethering (TCP :7653 + UDP :7655)
                                   │ Level 3: Wi-Fi LAN (TCP :7653 + UDP :7655, AES-GCM)
                                   │ Level 4: Bluetooth RFCOMM (Serial Port Profile)
                                   ▼
       ┌────────────────────────────────────────────────────────┐
       │                  PC SERVER (WINDOWS)                   │
       │  Win32 Native Tray & Flyout (GDI/GDI+) ──► AppHub      │
       │  Supervised Thread Hubs (Rust):                        │
       │   ├─ LinkHub (Listeners, Discovery responder, Probes)  │
       │   ├─ SessionHub (TOFU trust, P-256 ECDH, Sessions)     │
       │   ├─ MediaHub (Jitter buffer, H.264 reassembly, NV12)  │
       │   ├─ DeviceHub (Owlmic Mic bridge, Owlmic Cam, WASAPI) │
       │   └─ SettingsHub (%APPDATA%\Owlmic\owlmic.json)        │
       └────────────────────────────────────────────────────────┘
```

---

## Key Features

- 🎙️ **Owlmic Mic**: 48 kHz studio-quality audio capture with an adaptive jitter buffer, cubic drift resampling (±0.2%), and optional AI-based RNNoise suppression. Streamed uncompressed over USB cables and via Opus with FEC over wireless networks.
- 📷 **Owlmic Cam**: Low-latency H.264 video up to 1080p60. GPU OpenGL texture processing handles orientation, crop/fill framing, and mirroring. Windows 11 Media Foundation virtual camera (`owlmic_vcam.dll`) and Windows 10 DirectShow filter (`softcam.dll`).
- 🔊 **Speaker Loopback**: Route Windows PC audio back to your phone's speaker or connected Bluetooth headphones with optional "Quiet PC speakers" physical muting.
- ⚡ **Zero-Configuration Transporter**: Connects in under 1 second. Automatically migrates between USB debugging, USB tethering, Wi-Fi, and Bluetooth without dropping active calls (make-before-break).
- 🛡️ **Privacy & Trust (TOFU)**: Peer-to-peer only. Zero cloud servers, zero telemetry, zero accounts. Secured with NIST P-256 ECDH, 4-digit numeric verification codes, and AES-256-GCM authenticated encryption.
- 🪶 **Ultra Lightweight**: Single 4 MB installer on Windows. Idle memory usage under 15 MB. Tray flyout opens natively in < 15 ms using Win32 GDI with zero webview overhead.

---

## The Four Connection Levels

| Priority | Level | Carrier | Latency | Media Format | Best For |
| :---: | :--- | :--- | :--- | :--- | :--- |
| **1** | **USB Debugging** | Direct USB via `adb reverse` | < 10 ms | Raw PCM & 1080p60 | Competitive gaming, studio podcasts, live streaming |
| **2** | **USB Tethering** | Standard USB cable (RNDIS/NCM) | < 15 ms | Raw PCM & 1080p60 | Everyday high-speed wired connection |
| **3** | **Wi-Fi** | Local Area Network / Hotspot | < 25 ms | Opus (48 kbps) & 1080p | Wireless freedom around your desk and room |
| **4** | **Bluetooth** | RFCOMM Serial Profile | ~45 ms | Opus (24 kbps, audio only) | Failover when outside Wi-Fi coverage |

---

## Documentation

Comprehensive guides and architectural specifications are located in [`docs/`](docs/README.md):

### User Guides
- [Installing on Windows](docs/user/install-windows.md)
- [Installing on Android](docs/user/install-android.md)
- [First Connection & Pairing](docs/user/first-connection.md)
- [Everyday Use](docs/user/everyday-use.md)
- [Connection Levels & Switching](docs/user/connection-levels.md)
- [Troubleshooting & Repair](docs/user/troubleshooting.md)
- [Privacy & Security](docs/user/privacy.md)
- [Uninstalling](docs/user/uninstall.md)

### Technical Reference
- [System Architecture](docs/technical/architecture.md)
- [The Transporter & Networking](docs/technical/transporter.md)
- [Wire Protocol v3 Specification](docs/technical/protocol.md)
- [Audio & Video Media Pipelines](docs/technical/media.md)
- [Virtual Devices & Windows Integration](docs/technical/virtual-devices.md)
- [Settings & State Synchronization](docs/technical/settings.md)
- [Security & Cryptography](docs/technical/security.md)
- [Design Language & UI System](docs/technical/design-language.md)
- [Building from Source](docs/technical/building.md)
- [Release Process](docs/technical/releasing.md)

---

## Quick Start: Building from Source

### Windows Application (`pc/`)
Prerequisites: Windows 10/11, [Rust](https://rustup.rs/) (stable), and Visual C++ Build Tools or GCC.

```powershell
cd pc
cargo test --workspace
cargo build --release
```

### Android Application (`android/`)
Prerequisites: JDK 17, Android SDK (API 34+), NDK (r26+), and CMake 3.22+.

```bash
cd android
./gradlew testDebugUnitTest lintDebug
./gradlew assembleDebug
```

### Web Landing Site (`landing/`)
Prerequisites: Node.js 20+ and npm.

```bash
cd landing
npm install
npm run dev
```

---

## Repository Structure

```text
owlmic/
├── .github/          # CI/CD workflows, issue templates, and CODEOWNERS
├── android/          # Android Client (Kotlin, Jetpack Compose, CameraX, Oboe/AAudio)
│   ├── app/          # UI, MainActivity, Foreground Service, Notifier
│   ├── core/         # Hubs, Link Transporter, Protocol v3, Settings store
│   └── media/        # Oboe JNI, AAudio, Opus, CameraX GL pipelines
├── assets/           # Single source of truth for visual & binary assets (brand, icons, fonts)
├── design/           # Shared cross-platform schemas (tokens, copy, settings) & prototypes
├── docs/             # Structured user & technical documentation, and AI agent context
├── landing/          # React + Vite web landing page (owlmic.app)
├── pc/               # Desktop Server Workspace (Windows 10/11 & Linux)
│   ├── crates/       # Modular crates: link, media, proto, platform, linux, devices, ui
│   ├── installer/    # Inno Setup Windows installer (windows/) & Linux packages (linux/)
│   └── vcam/         # Windows 11 Media Foundation virtual camera driver (owlmic_vcam.dll)
├── protocol/         # Wire protocol version 3 normative vectors & specification
└── scripts/          # Automated pre-flight checks, linters, and Git hooks
```

---

## Contributing

Contributions are welcomed! Check out our [Contributing Guide](.github/CONTRIBUTING.md) to set up your environment in 10 minutes.

Join the conversation in [GitHub Discussions](https://github.com/diveshpatil9104/owlmic/discussions).

---

## License

Owlmic is licensed under the [MIT License](LICENSE).
Owlmic incorporates or interfaces with VB-CABLE (VB-Audio), softcam (MIT), RNNoise (BSD-3-Clause), and Opus (BSD-3-Clause). See [`pc/installer/windows/THIRD-PARTY-NOTICES.txt`](pc/installer/windows/THIRD-PARTY-NOTICES.txt) for license notices.
