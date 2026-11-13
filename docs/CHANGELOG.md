# Changelog

All notable changes to the Owlmic project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-10-04

### Added
- **Core Architecture**:
  - Rebuilt client and server on a unified Hub & Mailbox supervised threading model.
  - Multi-platform protocol engine implemented in Rust (`desktop/crates/owlmic-proto`) and Kotlin (`android/core/proto`).
  - Wire protocol version 3 with structured JSON control frames, MTU-safe media datagrams, and independent test vectors (`protocol/vectors/`).
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
  - Bi-directional PC speaker loopback streaming audio from Windows back to phone speaker/headphones.
- **Video Pipeline**:
  - Hardware-accelerated H.264 video streaming via Android CameraX and MediaCodec encoder.
  - GPU OpenGL ES 2.0 / 3.0 texture processor supporting rotation, mirroring, and framing modes (crop/fill vs. letterbox/fit).
  - Packet fragmentation under 1,200 bytes per datagram with reassembly and keyframe recovery requests.
  - Native Windows 11 virtual camera driver using Media Foundation (`MFCreateVirtualCamera` via `owlmic_vcam.dll`).
  - Windows 10 DirectShow fallback virtual camera filter (`softcam.dll`).
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
  - Lightweight, instant-open Win32 native flyout panel rendered in GDI/GDI+ with zero webview overhead.
  - One-click device repair tool elevating PowerShell to restore audio endpoint friendly names and firewall rules.
- **Distribution & Installation**:
  - Single, self-contained Windows installer generated via Inno Setup (`Owlmic-Setup-1.0.0.exe`).
  - Automated virtual microphone driver configuration script (`setup-audio-device.ps1`).
  - Android signed APK and Google Play App Bundle packaging.
