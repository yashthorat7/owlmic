# Owlmic

<p align="center">
  <a href="https://github.com/diveshpatil9104/owlmic/releases"><img src="https://img.shields.io/github/v/release/diveshpatil9104/owlmic?style=for-the-badge&color=FFFFFF&label=Download%20v0.1.0" alt="Download"></a>
  <a href="https://github.com/diveshpatil9104/owlmic/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/diveshpatil9104/owlmic/ci.yml?branch=main&style=for-the-badge&label=CI" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green?style=for-the-badge" alt="License: MIT"></a>
  <a href="https://github.com/diveshpatil9104/owlmic/discussions"><img src="https://img.shields.io/badge/Discussions-Join-blue?style=for-the-badge&logo=github" alt="Discussions"></a>
  <a href="https://github.com/diveshpatil9104/owlmic/stargazers"><img src="https://img.shields.io/github/stars/diveshpatil9104/owlmic?style=for-the-badge&color=FFD60A&logo=github" alt="Stars"></a>
  <a href="docs/README.md"><img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20Android-informational?style=for-the-badge" alt="Platforms"></a>
</p>

<p align="center"><strong>Fast. Lightweight. Real-time. Seamless.</strong><br>
Your Android phone is your PC's studio-grade microphone, HD camera, and remote speaker across four automatic, prioritized connection levels.</p>

---

## Overview

Owlmic turns your Android smartphone into your Windows or Linux PC's studio-grade **microphone**, low-latency HD **webcam**, and remote **speaker**. Video conferencing and streaming software (Discord, Zoom, Microsoft Teams, OBS Studio, Google Meet, Skype) detect **Owlmic Mic** and **Owlmic Cam** as standard native system hardware.

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
       │               PC SERVER (WINDOWS & LINUX)              │
       │  Win32 Native Tray / Linux KSNI Tray ──► AppHub        │
       │  Supervised Thread Hubs (Rust):                        │
       │   ├─ LinkHub (Listeners, Discovery responder, Probes)  │
       │   ├─ SessionHub (TOFU trust, P-256 ECDH, Sessions)     │
       │   ├─ MediaHub (Jitter buffer, H.264 reassembly, NV12)  │
       │   ├─ DeviceHub (WASAPI / PipeWire / V4L2 virtual cams) │
       │   └─ SettingsHub (owlmic.json versioned store)         │
       └────────────────────────────────────────────────────────┘
```

---

## Key Features

- 🎙️ **Owlmic Mic**: 48 kHz studio-quality audio capture with an adaptive jitter buffer, cubic drift resampling (±0.2%), and optional AI-based RNNoise suppression. Streamed uncompressed raw 16-bit PCM over USB cables and via Opus (48 kbps) with forward error correction (FEC) over wireless networks.
- 📷 **Owlmic Cam**: Low-latency H.264 video up to 1080p60. GPU OpenGL texture processing handles orientation, crop/fill framing, and mirroring. Native Windows 11 Media Foundation virtual camera (`owlmic_vcam.dll`), Windows 10 DirectShow filter (`softcam.dll`), and Linux `v4l2loopback`.
- 🔊 **Speaker Loopback**: Route PC audio back to your phone's speaker or connected Bluetooth headphones with optional "Quiet PC speakers" physical muting.
- ⚡ **Zero-Configuration Transporter**: Connects in under 1 second. Automatically migrates between USB debugging, USB tethering, Wi-Fi, and Bluetooth without dropping active calls (make-before-break).
- 🛡️ **Privacy & Trust (TOFU)**: Peer-to-peer only. Zero cloud servers, zero telemetry, zero accounts. Secured with NIST P-256 ECDH, 4-digit numeric verification codes, and AES-256-GCM authenticated encryption.
- 🪶 **Ultra Lightweight**: Single 4 MB installer on Windows. Idle memory usage under 15 MB. Tray flyout opens natively in < 15 ms using Win32 GDI/Direct2D with zero webview overhead. Native Linux StatusNotifierItem (KSNI) tray. Pure OLED black (`#000000`) Jetpack Compose UI on Android.

---

## The Four Connection Levels

| Priority | Level | Carrier | Latency | Media Format | Best For |
| :---: | :--- | :--- | :--- | :--- | :--- |
| **1** | **USB Debugging** | Direct USB via `adb reverse` | < 10 ms | Raw PCM & 1080p60 | Competitive gaming, studio podcasts, live streaming |
| **2** | **USB Tethering** | Standard USB cable (RNDIS/NCM) | < 15 ms | Raw PCM & 1080p60 | Everyday high-speed wired connection |
| **3** | **Wi-Fi** | Local Area Network / Hotspot | < 25 ms | Opus (48 kbps) & 1080p | Wireless freedom around your desk and room |
| **4** | **Bluetooth** | RFCOMM Serial Profile | ~45 ms | Opus (24 kbps, audio only) | Failover when outside Wi-Fi coverage |

---

## Quick Start

### 1. Install Owlmic
- **Windows**: Download and run [`Owlmic-Setup-0.1.0.exe`](https://github.com/diveshpatil9104/owlmic/releases) (or compile with `python menu.py build windows`). Owlmic runs silently in the system notification tray.
- **Linux**: Extract the release tarball, install the user service (`systemctl --user enable --now owlmic.service`), or run `owlmic` directly. See the [Linux Installation Guide](docs/user/install-linux.md).
- **Android**: Install the APK from [GitHub Releases](https://github.com/diveshpatil9104/owlmic/releases) or compile with `python menu.py build android`.

### 2. Connect Your Phone
- **USB Cable (Fastest, Recommended)**:
  - Plug your phone into your PC with a standard USB cable.
  - Enable **USB Tethering** (instant zero-config IP link) or **USB Debugging** (automatic `adb reverse`).
- **Wi-Fi (Wireless Freedom)**:
  - Connect your PC and phone to the same Wi-Fi router (or phone mobile hotspot).

### 3. Pair & Stream (TOFU Security)
1. Open **Owlmic** on your Android phone. It discovers your PC within 1–2 seconds.
2. Confirm the 4-digit pairing safety code displayed on your phone and PC desktop flyout.
3. Click **Allow** on your PC. Both devices are now cryptographically paired; future reconnects are instant.
4. Select **Owlmic Mic** as your microphone and **Owlmic Cam** as your camera in Discord, Zoom, Teams, Google Meet, or OBS Studio!

---

## Unified Developer & Release Menu

All builds, test suites, invariant checks, dependency audits, and release packaging are unified in the single root-level tool `menu.py` (zero external dependencies, standard library Python 3):

```bash
# Launch interactive developer & release menu:
python menu.py

# Non-interactive CLI automation:
python menu.py build [release|all|desktop|android|windows|linux]
python menu.py check [--fix]
python menu.py verify
python menu.py deps [--check|--fetch]
python menu.py clean
python menu.py hooks
```

| Command | Purpose | Output / Target |
| :--- | :--- | :--- |
| `python menu.py` | Full interactive terminal dashboard | Interactive selection |
| `python menu.py build release` | Build full release bundle (all platforms) | `dist/release/` |
| `python menu.py build android` | Compile Android release client APK | `dist/android/Owlmic-v0.1.0.apk` |
| `python menu.py build windows` | Windows installer & portable binaries | `dist/windows/Owlmic-Setup-0.1.0.exe` |
| `python menu.py build linux` | Linux standalone tarball & systemd unit | `dist/linux/Owlmic-v0.1.0-linux-x86_64.tar.gz` |
| `python menu.py check [--fix]` | Full pre-flight verification gate | Invariants + Rust + Android lint/tests |
| `python menu.py verify` | Repository invariants check | Casing, binary ban, JSON, tokio, versions |
| `python menu.py deps [--check\|--fetch]` | Audit or prefetch packaging dependencies | Driver cache (`VB-CABLE`, `softcam.dll`) |
| `python menu.py clean` | Clean build artifacts | Reset `dist/` |
| `python menu.py hooks` | Activate repository git hooks | `core.hooksPath` -> `scripts/hooks` |

Build outputs are saved to `dist/android/`, `dist/windows/`, and `dist/linux/`. Packaging dependencies (`VB-CABLE` audio driver and `softcam.dll` virtual camera fallback) are tracked in `scripts/packaging/windows/dependencies.json`, verified via SHA-256, and cached locally for offline and reproducible builds.

---

## Documentation

Comprehensive guides, specifications, and architecture manuals are located in [`docs/`](docs/README.md):

### User Guides
- [Installing on Windows](docs/user/install-windows.md) — Windows 10/11 installation and driver setup
- [Installing on Linux](docs/user/install-linux.md) — PipeWire virtual source, v4l2loopback camera, systemd service
- [Installing on Android](docs/user/install-android.md) — APK installation, permissions, battery optimization
- [First Connection & Pairing](docs/user/first-connection.md) — 4-digit safety code confirmation, TOFU trust
- [Everyday Use](docs/user/everyday-use.md) — Mic, camera, and speaker toggles, video quality, tray panel
- [Connection Levels & Switching](docs/user/connection-levels.md) — USB, Wi-Fi, Bluetooth ladder and make-before-break
- [Troubleshooting & Repair](docs/user/troubleshooting.md) — Firewall, drivers, connection drops, one-click repair
- [Privacy & Security](docs/user/privacy.md) — Local peer-to-peer guarantees, zero telemetry, on-device crypto
- [Uninstalling](docs/user/uninstall.md) — Clean driver and endpoint removal

### Technical Reference
- [System Architecture](docs/technical/architecture.md) — Topology, role invariant, Hub & Mailbox threading model
- [The Transporter & Networking](docs/technical/transporter.md) — UDP beacons, link priority ladder, carrier migration
- [Wire Protocol v3 Specification](docs/technical/protocol.md) — Binary framing, JSON control packets, media datagrams
- [Audio & Video Media Pipelines](docs/technical/media.md) — AAudio, Opus, jitter buffer, cubic resampler, H.264 NV12
- [Virtual Devices & Platform Integration](docs/technical/virtual-devices.md) — WASAPI bridge, Media Foundation, DirectShow, repair engine
- [Settings & State Synchronization](docs/technical/settings.md) — Versioned store (`owlmic.json`), bidirectional sync
- [Security & Cryptography](docs/technical/security.md) — NIST P-256 ECDH, HKDF-SHA256, AES-256-GCM authenticated encryption
- [Design Language & UI System](docs/technical/design-language.md) — OLED pure black tokens, Win32 flyout, Jetpack Compose
- [Building from Source](docs/technical/building.md) — Build steps for Windows, Linux, Android, and Website
- [Release Process & Versioning](docs/technical/releasing.md) — Milestones, lockstep versions, artifact signing, checksums

### Modular Domain Skills Catalog
Specialized engineering context rulebooks in [`docs/skills/`](docs/skills/README.md) across three operational priority tiers:
- **Priority 1 (Core Output, Clean Code & Token Economy — Always Active)**:
  - [`anti-slop-and-lean-code.md`](docs/skills/anti-slop-and-lean-code.md) — Anti-over-engineering, zero chatty comments, YAGNI
  - [`token-saving-and-context.md`](docs/skills/token-saving-and-context.md) — Surgical inspection, targeted edits, context hygiene
  - [`procedures-and-verification.md`](docs/skills/procedures-and-verification.md) — 5-stage task lifecycle, pre-flight verification gates
- **Priority 2 (Subsystem & Platform Development — Active on Modification)**:
  - [`pc-rust.md`](docs/skills/pc-rust.md) — Rust 2024 desktop server, thread supervision, Tokio bans outside Linux BT
  - [`windows-backend.md`](docs/skills/windows-backend.md) — WASAPI virtual audio, Media Foundation camera, Direct2D tray
  - [`linux-backend.md`](docs/skills/linux-backend.md) — PipeWire virtual audio source, v4l2loopback virtual camera, KSNI tray
  - [`android-kotlin.md`](docs/skills/android-kotlin.md) — Jetpack Compose, foreground service lifecycle, zero-alloc Oboe JNI
  - [`media-and-dsp.md`](docs/skills/media-and-dsp.md) — 48 kHz audio DSP, adaptive jitter buffers, cubic resampler, H.264 slicing
  - [`protocol-design-sync.md`](docs/skills/protocol-design-sync.md) — Wire Protocol v3 framing, test vectors lockstep, design tokens
- **Priority 3 (On-Demand & Specialized Operations — Explicit Call Only)**:
  - [`git-workflow.md`](docs/skills/git-workflow.md) — Strict read-only agent rule, convenient `git add .` staging, Conventional Commits
  - [`build-and-bundle.md`](docs/skills/build-and-bundle.md) — Builder procedures, driver caching, centralized `dist/` distribution
  - [`web-landing.md`](docs/skills/web-landing.md) — React 19, TypeScript, Vite, zero telemetry landing site

---

## Repository Structure

```text
owlmic/
├── .github/          # CI/CD workflows, issue templates, and CODEOWNERS
├── android/          # Android Client (Kotlin, Jetpack Compose, CameraX, Oboe/AAudio)
│   ├── app/          # UI, MainActivity, Foreground Service, Notifier
│   ├── core/         # Hubs, Link Transporter, Protocol v3, Settings store
│   └── media/        # Oboe JNI, AAudio, Opus wrapper, CameraX GL pipelines
├── desktop/          # Desktop Server Workspace (Rust 2024, Windows & Linux)
│   └── crates/       # 13 modular crates:
│                     #   owlmic-app      - Tray lifecycle, Win32/Linux event loop
│                     #   owlmic-devices  - WASAPI & PipeWire audio, VCam feeds, repair
│                     #   owlmic-hub      - Hub supervisor, mailboxes, restart policies
│                     #   owlmic-link     - Transporter: ADB reverse, Tether, Wi-Fi, BT
│                     #   owlmic-linux    - PipeWire source, v4l2loopback, KSNI tray
│                     #   owlmic-media    - Audio DSP, RNNoise, jitter buffer, H.264/NV12
│                     #   owlmic-platform - Platform abstractions, paths, single instance
│                     #   owlmic-proto    - Protocol v3 framing, binary/JSON envelopes
│                     #   owlmic-session  - TOFU trust, NIST P-256 ECDH, AES-256-GCM
│                     #   owlmic-settings - Thread-safe owlmic.json store, state sync
│                     #   owlmic-ui       - Native Win32 GDI/Direct2D tray flyout (<15ms)
│                     #   owlmic-vcam     - Windows 11 MF virtual camera (owlmic_vcam.dll)
│                     #   owlmic-windows  - WASAPI loopback, registry property store, COM
├── docs/             # User guides, technical architecture, and domain skills catalog
│   ├── user/         # User manuals: installation (Win/Linux/Android), everyday use, FAQ
│   ├── technical/    # Deep technical specifications: protocols, media, security, build
│   └── skills/       # 3-tier modular domain skills catalog for AI agents & contributors
├── scripts/          # Unified builder (build.py), pre-flight checks, packaging, and Git hooks
│   ├── packaging/    # Windows Inno Setup installer, Linux packaging, driver cache
│   │   ├── cache/    # Untracked third-party driver cache (softcam.dll)
│   │   ├── linux/    # Linux systemd service (owlmic.service) & setup scripts
│   │   └── windows/  # Windows Inno Setup script (owlmic.iss) & setup-audio-device.ps1
│   └── hooks/        # Pre-commit & commit-msg verification hooks
├── shared/           # Single source of truth for assets, design, protocol, and vendor source
│   ├── assets/       # Canonical visual brand, icons, and fonts
│   ├── design/       # Tokens, copy, and settings schemas
│   ├── protocol/     # Protocol v3 wire specification and test vectors
│   └── vendor/       # Vendored libopus submodule
├── website/          # React + Vite website (owlmic.app)
├── menu.py           # Unified interactive developer & release dashboard
└── dist/             # Centralized build distribution destination (git-ignored)
```

---

## Contributing

Contributions are welcomed! Check out our [Contributing Guide](.github/CONTRIBUTING.md) to set up your environment in 10 minutes.
See [`AGENTS.md`](AGENTS.md) and [`docs/skills/`](docs/skills/README.md) for domain guidelines.

Join the conversation in [GitHub Discussions](https://github.com/diveshpatil9104/owlmic/discussions).

---

## License

Owlmic is licensed under the [MIT License](LICENSE).
Owlmic incorporates or interfaces with VB-CABLE (VB-Audio), softcam (MIT), RNNoise (BSD-3-Clause), and Opus (BSD-3-Clause). See [`scripts/packaging/windows/THIRD-PARTY-NOTICES.txt`](scripts/packaging/windows/THIRD-PARTY-NOTICES.txt) for license notices.
