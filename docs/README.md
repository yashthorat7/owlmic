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

### 1.3 Shared Design & Assets
- **[`shared/design/`](../shared/design/)**: Canonical cross-platform schemas — `tokens.json` (OLED palette, typography, geometry), `copy.json` (UI strings and product name invariant), `settings.json` (versioned settings schema).
- **[`shared/design/prototypes/`](../shared/design/prototypes/)**: Interactive HTML UI blueprints for Android + PC layout exploration.
- **[`shared/assets/`](../shared/assets/)**: Single source of truth for shared visual assets — `brand/` (master owl SVG marks), `icons/` (tray and status glyphs), `fonts/` (shared UI fonts).
- **[`shared/vendor/`](../shared/vendor/)**: Centralized third-party submodules (`opus/`).

### 1.4 Automation & Scripts
- **[`scripts/build.py`](../scripts/build.py)**: Interactive, zero-dependency release builder routing packages into `dist/`.
- **[`scripts/verify-repo-invariants.py`](../scripts/verify-repo-invariants.py)**: Zero-dependency Python 3 checker for product name casing, binary ban, Tokio restrictions, JSON validity, and version lockstep.
- **[`scripts/check.py`](../scripts/check.py)**: Master pre-flight validator (invariants + Rust fmt/clippy/test + Android lint).
- **[`scripts/setup-hooks.py`](../scripts/setup-hooks.py)**: Cross-platform Git hooks setup (`core.hooksPath` configuration).
- **[`scripts/hooks/`](../scripts/hooks/)**: Git `pre-commit` (invariants + cargo fmt) and `commit-msg` (Conventional Commits enforcement).

### 1.5 Project History
- **[`CHANGELOG.md`](CHANGELOG.md)**: Release history and version milestones starting from v0.1.0.

### 1.6 Modular Domain Skills (`docs/skills/`)
Specialized context rulebooks routed from the root [`AGENTS.md`](../AGENTS.md):
- **[`pc-rust.md`](skills/pc-rust.md)**: Threading model, zero-alloc audio, and real-time safety for PC Rust crates.
- **[`windows-backend.md`](skills/windows-backend.md)**: WASAPI virtual audio, Media Foundation camera, and Direct2D tray on Windows.
- **[`linux-backend.md`](skills/linux-backend.md)**: PipeWire virtual audio, v4l2loopback virtual camera, and XDG rules.
- **[`android-kotlin.md`](skills/android-kotlin.md)**: Jetpack Compose, foreground service lifecycle, and Oboe JNI rules.
- **[`media-and-dsp.md`](skills/media-and-dsp.md)**: Low-latency audio DSP, jitter buffers, H.264 slice reassembly, and NV12 shared memory.
- **[`web-landing.md`](skills/web-landing.md)**: React 19, TypeScript, and Vite standards for the web landing and release download site.
- **[`protocol-design-sync.md`](skills/protocol-design-sync.md)**: Zero-drift rules for wire protocol vectors and design tokens.
- **[`anti-slop-and-lean-code.md`](skills/anti-slop-and-lean-code.md)**: Code quality standards, anti-over-engineering, and lean engineering practices.
- **[`token-saving-and-context.md`](skills/token-saving-and-context.md)**: Agent token efficiency, surgical file inspection, and context hygiene.
- **[`procedures-and-verification.md`](skills/procedures-and-verification.md)**: Standard operating procedures, 5-stage task lifecycle, and verification gates.
- **[`git-workflow.md`](skills/git-workflow.md)**: Conventional Commits, branch naming, and strict read-only agent rules.
- **[`build-and-bundle.md`](skills/build-and-bundle.md)**: Builder procedures, driver caching, and centralized `dist/` distribution.

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
