# Owlmic Documentation

Welcome to the official documentation for **Owlmic** — the high-performance, lightweight bridge that transforms your Android phone into a studio-grade microphone, HD webcam, and remote speaker for your PC over four seamless connection levels.

---

## 1. Documentation Index

### 1.1 User Guides (`docs/user/`)
Everyday instructions, installation, configuration, and troubleshooting for end users:

| Guide | Description |
| :--- | :--- |
| **[`install-windows.md`](user/install-windows.md)** | Installing Owlmic on Windows 10 and 11, setting up virtual audio and camera drivers |
| **[`install-linux.md`](user/install-linux.md)** | Installing Owlmic on Linux, configuring PipeWire virtual microphone and v4l2loopback |
| **[`install-android.md`](user/install-android.md)** | Installing the Android APK or Google Play release, required permissions, battery optimizations |
| **[`first-connection.md`](user/first-connection.md)** | Step-by-step first pairing, 4-digit safety code confirmation, trust-on-first-use (TOFU) |
| **[`everyday-use.md`](user/everyday-use.md)** | Daily workflow: toggling mic, camera, and speaker; selecting video quality; managing tray flyout |
| **[`connection-levels.md`](user/connection-levels.md)** | The four connection levels (USB debugging, USB tethering, Wi-Fi, Bluetooth) and auto-switching |
| **[`troubleshooting.md`](user/troubleshooting.md)** | Solving firewall blocks, driver errors, connection drops, and audio latency issues |
| **[`privacy.md`](user/privacy.md)** | Privacy-first principles: local network only, zero telemetry, on-device encryption, manual capture control |
| **[`uninstall.md`](user/uninstall.md)** | Clean removal from Windows and Android, driver deregistration, endpoint cleanup |

### 1.2 Technical Reference (`docs/technical/`)
Deep-dive architectural specifications, protocols, media pipelines, and developer guidelines:

| Document | Description |
| :--- | :--- |
| **[`architecture.md`](technical/architecture.md)** | System topology, client-server invariants, Hub & Mailbox model, supervised threading |
| **[`transporter.md`](technical/transporter.md)** | Discovery beacon, link priority ladder, make-before-break migration, recovery watchdogs |
| **[`protocol.md`](technical/protocol.md)** | Wire protocol version 3 framing, control messages, media headers, test vectors |
| **[`media.md`](technical/media.md)** | Real-time audio (AAudio/Opus/PCM), video (CameraX/H.264/NV12), and speaker loopback pipelines |
| **[`virtual-devices.md`](technical/virtual-devices.md)** | Owlmic Mic (WASAPI bridge), Owlmic Cam (Windows 11 MF & Windows 10 DirectShow), repair engine |
| **[`settings.md`](technical/settings.md)** | Synchronized versioned settings (`design/settings.json`), store format, persistence |
| **[`security.md`](technical/security.md)** | P-256 ECDH, HKDF-SHA256, AES-256-GCM wireless encryption, replay protection, rate limits |
| **[`design-language.md`](technical/design-language.md)** | OLED pure black UI tokens, typography, native GDI win32 panel, Jetpack Compose |
| **[`building.md`](technical/building.md)** | Building Android (Gradle/NDK) and PC (Cargo/MSVC/GNU), dependencies, environment setup |
| **[`releasing.md`](technical/releasing.md)** | Packaging Inno Setup installer, APK/AAB signing, GitHub Releases, Google Play distribution |

### 1.4 Desktop Crates Architecture (`desktop/crates/`)
The desktop companion server is partitioned into 13 decoupled crates under Rust 2024:
- **`owlmic-app`**: Application entry point, tray lifecycle, Win32 message loop & Linux event handling.
- **`owlmic-devices`**: System virtual audio endpoints (WASAPI / PipeWire), camera frame feeder, and repair engine.
- **`owlmic-hub`**: Actor-like Hub supervisor, mailbox channels, and restart policies.
- **`owlmic-link`**: Multi-transport link manager (USB debugging via ADB, USB tethering, Wi-Fi UDP/TCP, Bluetooth RFCOMM).
- **`owlmic-linux`**: Linux platform integration: PipeWire virtual source, v4l2loopback feeder, KSNI status notifier.
- **`owlmic-media`**: Low-latency audio DSP (RNNoise, cubic drift resampler, jitter buffer) and video frame processing (H.264 slice reassembly, NV12 conversion).
- **`owlmic-platform`**: Cross-platform abstractions (paths, single instance mutex, OS detection, elevation).
- **`owlmic-proto`**: Wire Protocol v3 framing, binary envelopes, JSON control framing, and serialization.
- **`owlmic-session`**: Cryptographic trust (TOFU), NIST P-256 ECDH key exchange, AES-256-GCM cipher, 4-digit code confirmation.
- **`owlmic-settings`**: Thread-safe configuration store (`owlmic.json`), schema validation, and bidirectional state sync.
- **`owlmic-ui`**: Native Win32 GDI/Direct2D dark OLED flyout UI (< 15 ms open time, zero webview overhead) and tray widgets.
- **`owlmic-vcam`**: Windows 11 Media Foundation virtual camera COM server (`owlmic_vcam.dll`) and shared memory ring buffer.
- **`owlmic-windows`**: Windows platform integration: WASAPI audio loopback, registry property store, COM setup.

### 1.5 Shared Design & Assets
- **[`shared/design/`](../shared/design/)**: Canonical cross-platform schemas — `tokens.json` (OLED palette, typography, geometry), `copy.json` (UI strings and product name invariant), `settings.json` (versioned settings schema).
- **[`shared/design/prototypes/`](../shared/design/prototypes/)**: Interactive HTML UI blueprints for Android + PC layout exploration.
- **[`shared/assets/`](../shared/assets/)**: Single source of truth for shared visual assets — `brand/` (master owl SVG marks), `icons/` (tray and status glyphs), `fonts/` (shared UI fonts).
- **[`shared/vendor/`](../shared/vendor/)**: Centralized third-party submodules (`opus/`).

### 1.6 Automation & Scripts
- **[`menu.py`](../menu.py)**: Unified root interactive developer & release menu (build, check, verify, deps, clean, hooks).
- **[`scripts/build.py`](../scripts/build.py)**: Interactive and CLI release packaging tool routing artifacts to `dist/`.
- **[`scripts/verify-repo-invariants.py`](../scripts/verify-repo-invariants.py)**: Zero-dependency Python 3 checker for product name casing, binary ban, Tokio restrictions, JSON validity, and version lockstep.
- **[`scripts/check.py`](../scripts/check.py)**: Master pre-flight validator (invariants + Rust fmt/clippy/test + Android lint).
- **[`scripts/setup-hooks.py`](../scripts/setup-hooks.py)**: Cross-platform Git hooks setup (`core.hooksPath` configuration).
- **[`scripts/hooks/`](../scripts/hooks/)**: Git `pre-commit` (invariants + cargo fmt) and `commit-msg` (Conventional Commits enforcement).

### 1.7 Project History
- **[`CHANGELOG.md`](CHANGELOG.md)**: Release history and version milestones starting from v0.1.0.

### 1.8 Modular Domain Skills (`docs/skills/`)
Specialized context rulebooks routed from the root [`AGENTS.md`](../AGENTS.md) across three priority tiers:
- **[Domain Skills Catalog Index](skills/README.md)**: Complete guide to the 3-tier operational priority system.
- **Priority 1 (Always Active)**:
  - **[`anti-slop-and-lean-code.md`](skills/anti-slop-and-lean-code.md)**: Zero chatty comments, YAGNI, zero code churn, lean engineering.
  - **[`token-saving-and-context.md`](skills/token-saving-and-context.md)**: Surgical file reads, targeted edits, and context hygiene.
  - **[`procedures-and-verification.md`](skills/procedures-and-verification.md)**: 5-stage task lifecycle and pre-flight verification gates.
- **Priority 2 (Active on Modification)**:
  - **[`desktop-rust.md`](skills/desktop-rust.md)**: Rust 2024 desktop server, threading model, and Tokio bans outside Linux BT.
  - **[`windows-backend.md`](skills/windows-backend.md)**: WASAPI audio bridge, Media Foundation virtual camera, and Direct2D tray on Windows.
  - **[`linux-backend.md`](skills/linux-backend.md)**: PipeWire virtual audio source, v4l2loopback virtual camera, and KSNI tray.
  - **[`android-kotlin.md`](skills/android-kotlin.md)**: Jetpack Compose, foreground service lifecycle, and zero-alloc Oboe JNI rules.
  - **[`media-and-dsp.md`](skills/media-and-dsp.md)**: 48 kHz audio DSP, jitter buffers, cubic drift resampling, H.264 slice reassembly.
  - **[`protocol-design-sync.md`](skills/protocol-design-sync.md)**: Wire Protocol v3 framing, vector test lockstep, tokens and copy sync.
- **Priority 3 (Explicit User Request Only)**:
  - **[`git-workflow.md`](skills/git-workflow.md)**: Strict read-only agent rule, convenient `git add .` staging, Conventional Commits.
  - **[`build-and-bundle.md`](skills/build-and-bundle.md)**: Developer menu, dependency caching, and `dist/` release bundles.
  - **[`website.md`](skills/website.md)**: React 19, TypeScript, and Vite website standards.

---

## 2. Key System Constants & Parameters

| Parameter | Value | Location / Reference | Description |
| :--- | :---: | :--- | :--- |
| **TCP Control Port** | `7653` | [`protocol.md`](technical/protocol.md) | Bidirectional JSON control framing and USB media carrier |
| **UDP Discovery Port** | `7654` | [`protocol.md`](technical/protocol.md) | Local network multicast/broadcast discovery (`OWLMIC?3` / `OWLMIC!3`) |
| **UDP Media Port** | `7655` | [`protocol.md`](technical/protocol.md) | High-speed datagram media carrier for IP links (USB tethering, Wi-Fi) |
| **Protocol Version** | `3` | [`protocol.md`](technical/protocol.md) | Wire protocol version number |
| **Audio Sample Rate** | `48,000 Hz` | [`media.md`](technical/media.md) | Studio broadcast standard rate (mono mic, stereo speaker) |
| **Audio Frame Interval** | `10 ms` (20 ms on BT) | [`media.md`](technical/media.md) | 480 samples per packet (960 on BT) for ultra-low latency |
| **Max Video Packet** | `1,200 bytes` | [`protocol.md`](technical/protocol.md) | H.264 slice size to keep IP datagrams within 1,232 bytes (MTU-safe) |
| **Heartbeat Interval** | `1,000 ms` | [`transporter.md`](technical/transporter.md) | Ping/pong liveness verification between endpoints |
| **Connection Timeout** | `3,000 ms` | [`transporter.md`](technical/transporter.md) | Reconnect trigger threshold upon lost heartbeats |
| **Pairing Approval Timeout** | `120,000 ms` | [`security.md`](technical/security.md) | User authorization wait window on PC |
| **Max Frame Payload** | `1 MiB` | [`protocol.md`](technical/protocol.md) | Framing memory exhaustion safety bound |

---

## 3. High-Level Architecture

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
