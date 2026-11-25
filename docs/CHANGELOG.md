# Changelog

All notable changes to the Owlmic project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [1.1.0] - 2026-11-26

### Added
- **Release Version Automation (`scripts/core/release-update.py`)**:
  - Added lockstep version bumper across `desktop/Cargo.toml`, `android/app/build.gradle.kts` (with automatic `versionCode` increment), and `scripts/packaging/windows/owlmic.iss`, integrated into `scripts/menu.py` (`release-update`) with invariant verification and release prompt generation.

### Changed
- **USB / ADB Lifecycle & Process Cleanup (`owlmic-link`, `owlmic-app`)**:
  - Hardened Windows ADB management in `adb.rs` with Job Object process-tree cleanup, active TCP listener detection, bounded ADB timeouts, and explicit `owlmic_link::adb::shutdown()` on exit (`start.rs`).
- **Speaker Loopback & Oboe Synchronization (`android/media`, `owlmic-devices`)**:
  - Added `Oboe.burstFrames` JNI helper, condition-variable wakeup (`Smoother.awaitData`) in `SpeakerPipeline`, and increased Windows WASAPI loopback queue headroom (`MAX_QUEUED_FLOATS` from 4 to 6 frames).
- **USB Icon Glyph (`shared/assets/icons/usb.svg`)**:
  - Updated the USB connector vector icon for cleaner visual clarity.

### Fixed
- **Link Switching & Mid-Stream Camera Recovery (`LinkHub`, `MediaRouter`, `Packets`, `Seeker`, `CameraPipeline`, `owlmic-media`, `owlmic-devices`)**:
  - Fixed Wi-Fi ↔ USB carrier transitions so `CameraOutput` stays active without unnecessary teardown, resets video frame counters cleanly (`allow_frame_reset`), and guards against stale tunnel acceptance (`canAcceptTunnelNow`).
  - Fixed H.264 decoder recovery on mid-stream resolution switches (`decoder.rs`) and aligned desktop camera preview visibility gating (`panel.rs`).

---

## [1.0.0] - 2026-11-26

### Added
- **Linux Desktop Companion & Packaging (`owlmic-linux`)**:
  - Native userspace PipeWire / PulseAudio virtual microphone (`Owlmic Mic`) and sink loopback monitor, `v4l2loopback` virtual camera (`/dev/video*`), and StatusNotifierItem (`ksni`) tray integration.
  - Systemd user unit (`owlmic.service`), `setup-linux-devices.sh`, CI target (`desktop-linux`), and portable `.tar.gz` release packaging (`Owlmic-v1.0.0-linux-x86_64.tar.gz`) with Windows-to-Linux cross-build support.
- **Unified OLED Bento UI (Android & Desktop)**:
  - Redesigned Android Jetpack Compose screens (`MainScreen`, `Playground`, `Drawers`) and native `312 × 312 px` Win32 Direct2D / GDI tray flyout (`owlmic-ui`) aligned with `shared/design/prototypes/`.
  - Dynamic SVG glyph compilation from `shared/assets/icons/*.svg` (`build.rs` and `:core:generateDesign`), dynamic brand owl watermark (`shared/assets/brand/icon.svg`), sideways letter-by-letter Android wordmark/UI rotation, and 2×3 Help drawer with subpages.
- **Bidirectional Feature & Settings Sync (Hive Mind)**:
  - Real-time `STATE` (`0x20`) and versioned `SETTINGS` (`0x21`) synchronization across Android and PC for mic, camera, and speaker toggles, camera format, audio DSP, and transport preferences (`link.*`).
  - Offline transport configuration on PC persists locally and reconciles deterministically upon connection.
- **Camera Format HUD, Mirror, Flip & Preview Toggle**:
  - Expandable camera ratio HUD (`Auto`/`Full`, `16:9`, `4:3`, `2:1`, `1:1`, `1:2`, `3:4`, `9:16`) with landscape/portrait toggle, symmetric horizontal mirroring (`camera.mirror`), lens flip, cover-fit preview with optical crop guides, eye/eye-off local preview toggle, and centered Direct2D `Owlmic Cam` owl placeholder (`owlmic-vcam`).
- **Audio DSP, Voice Isolation & Live Waveform Metering**:
  - Real-time RMS audio metering (`WAVE_GAIN = 1.2`) driving live centered and camera-docked waveforms across Android and PC.
  - `EchoSuppressor` (-24 dB residual echo attenuation), `CallMode` live headset hot-plug switching (`OutputChanged`), live Android `NoiseSuppressor` toggling without reopening Oboe, pre-denoiser room noise floor tracking (`-99 dBFS` digital-zero guard), eased `Agc` auto-level normalization (`-20` to `+36 dB`) with burst clamping, center-filled `-12` to `+12 dB` volume fader, and soft-knee limiter.
- **Unified Developer & Release Tooling**:
  - Standard-library Python toolkit (`scripts/menu.py` and `scripts/core/build.py`) routing build outputs to `releases/`, automated invariant verifier (`scripts/core/verify-repo-invariants.py`), master checker (`scripts/core/check.py`), Git hooks (`pre-commit`, `commit-msg`, `pre-push`), and SHA-256 pinned Windows packaging manifest (`dependencies.json`).

### Changed
- **Compact Opus Speaker Loopback**:
  - Switched PC speaker loopback (`Stream 3`) to compact 48 kHz stereo Opus across all links (`48–96 kbps` cable, `32–96 kbps` Wi-Fi, `24–64 kbps` Bluetooth) with telemetry-driven adaptation, anti-replay jitter buffer (`Smoother`), ±0.4% drift resampling, and real-time Oboe `AudioStreamDataCallback` ring-buffer playback.
- **Video Bitrate & Preview Tuning**:
  - Tuned `VideoPlan` starting bitrates across `360p`/`480p`/`720p`/`1080p` for wired and Wi-Fi links, raised PC preview cadence to 30 FPS (`preview.rs`), and stabilized Windows 11 Media Foundation sample timestamps and clean-point metadata for WebRTC browsers.
- **Repository Layout & Packaging Paths**:
  - Standardized root directory layout (`android/`, `desktop/`, `docs/`, `scripts/`, `shared/`, `website/`), moved release output from `dist/` to `releases/`, and updated GitHub Actions CI and release workflows.

### Fixed
- **Connectivity & Link Switching Stability**:
  - Stabilized make-before-break carrier migration, ADB tunnel and network interface detection, disconnect/hold state synchronization, and Bluetooth camera capability gating.
- **H.264 Multi-Slice Reassembly & Decoder Recovery**:
  - Fixed multi-slice NAL frame drops in `owlmic-devices` / `owlmic-media`, visible-aperture NV12 stride clipping (`MF_MT_MINIMUM_DISPLAY_APERTURE`), and smooth preview fade transitions without stalls.
- **Icon Sizing & Visual Clarity**:
  - Reduced `24×24` canvas padding in `eye.svg` and `eye-off.svg` for clearer camera preview toggle rendering.

---

## [0.1.0] - 2026-10-04

### Added
- **Core Architecture**:
  - Rebuilt client and server on a unified Hub & Mailbox supervised threading model.
  - Multi-platform protocol engine implemented in Rust (`desktop/crates/owlmic-proto`) and Kotlin (`android/core/proto`).
  - Wire protocol version 3 with structured JSON control frames, MTU-safe media datagrams, and independent test vectors (`shared/protocol/vectors/`).
- **Transporter & Networking**:
  - Automatic connection across four prioritized tiers:
    1. USB debugging via automatic `adb reverse` forwarder.
    2. USB tethering over dynamic IP interfaces.
    3. Wi-Fi Local Area Network with discovery beacons (`OWLMIC?3` / `OWLMIC!3`).
    4. Bluetooth RFCOMM serial channel for audio-only failover.
  - Make-before-break transport migration using the `SWITCH` control handshake.
  - Background connection watchdog and recovery ladder with anti-flap hysteresis.
- **Audio Pipeline**:
  - Studio-quality 48 kHz mono microphone streaming with 10 ms packet interval (20 ms over Bluetooth).
  - Uncompressed 16-bit PCM streaming over wired USB links; high-efficiency Opus encoding with in-band FEC over wireless links.
  - Android high-performance native audio capture using Oboe / AAudio.
  - PC adaptive jitter buffer with cubic drift resampling (±0.2%) to synchronize crystal oscillators.
  - Optional AI-powered noise reduction via embedded RNNoise DSP.
  - Bi-directional PC speaker loopback streaming audio from Windows/Linux back to phone speaker/headphones.
- **Video Pipeline**:
  - Hardware-accelerated H.264 video streaming via Android CameraX and MediaCodec encoder.
  - GPU OpenGL ES 2.0 / 3.0 texture processor supporting rotation, mirroring, and framing modes (crop/fill vs. letterbox/fit).
  - Packet fragmentation under 1,200 bytes per datagram with reassembly and keyframe recovery requests.
  - Native Windows 11 virtual camera driver using Media Foundation (`MFCreateVirtualCamera` via `owlmic_vcam.dll`).
  - Windows 10 DirectShow fallback virtual camera filter (`softcam.dll`).
  - Linux virtual camera driver feeding `/dev/video*` via `v4l2loopback`.
  - Native 10 FPS low-latency video preview embedded directly into the PC tray panel.
- **Security & Trust**:
  - Cryptographic identity pairs using NIST P-256 elliptic curves.
  - Trust-On-First-Use (TOFU) pairing workflow with 4-digit numeric verification code (`HMAC-SHA256`).
  - AES-256-GCM authenticated encryption for control and media on all wireless transports.
  - 64-packet replay window and strict per-direction packet sequencing.
  - DoS mitigation: discovery rate-limiting (max 20 probes/sec) and handshake rate-limiting (max 4 per sec).
- **User Interface & Design**:
  - Pure functional design system rooted in OLED pure black (`#000000`) and high-contrast status tokens.
  - Jetpack Compose interface on Android with reactive StateFlow observation from foreground service.
  - Lightweight, instant-open Win32 native flyout panel rendered in GDI/Direct2D with zero webview overhead.
  - Linux StatusNotifierItem (KSNI) tray integration with X11/Wayland flyout support.
  - One-click device repair tool elevating PowerShell to restore audio endpoint friendly names and firewall rules.
- **Distribution & Installation**:
  - Single, self-contained Windows installer generated via Inno Setup (`Owlmic-Setup-0.1.0.exe`).
  - Automated virtual microphone driver configuration script (`setup-audio-device.ps1`).
  - Linux standalone portable archive with systemd user unit (`owlmic.service`) and `setup-linux-devices.sh`.
  - Android signed APK and Google Play App Bundle packaging.
