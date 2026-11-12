# Technical Architecture

This document specifies the internal architecture of Owlmic across Android (client) and Windows (server), detailing concurrency models, hub topology, message passing, and core system invariants.

---

## 1. System Topology & Role Invariant

Owlmic enforces a strict, asymmetrical role division:
- **Phone is Always the Client**: The Android device runs `OwlmicService` as a persistent foreground service. It initiates network discovery, opens TCP/UDP connections to the PC, captures local audio/video hardware, and transmits packets upstream.
- **PC is Always the Server**: The Windows host runs `owlmic.exe` as a per-user tray process. It listens on TCP 7653 and UDP 7654/7655, negotiates sessions, decodes incoming media, feeds virtual camera and audio drivers, and coordinates bidirectional control.

```
+─────────────────────────────────────────────────────────────+
│                       ANDROID CLIENT                        │
│  UI (Jetpack Compose) ◄── StateFlow ──► AppHub              │
│  Foreground Service (OwlmicService)                         │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────┐  │
│  │   LinkHub     │  │   MediaHub    │  │   SettingsHub   │  │
│  │ (Transporter) │  │ (AAudio/CamX) │  │(Versioned Store)│  │
│  └───────┬───────┘  └───────┬───────┘  └────────┬────────┘  │
+──────────┼──────────────────┼───────────────────┼───────────+
           │                  │                   │
           │ Wire Protocol v3 │ (TCP 7653 / UDP 7655)
           ▼                  ▼                   ▼
+──────────┴──────────────────┴───────────────────┴───────────+
│                         PC SERVER                           │
│  Native Win32 Flyout (GDI/GDI+) ◄── Bounded Handoff ──► App │
│  Supervised Thread Hubs:                                    │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────┐  │
│  │    LinkHub    │  │   SessionHub  │  │    MediaHub     │  │
│  │(Transporter/IO│  │  (TOFU Trust) │  │ (Jitter/H.264)  │  │
│  └───────────────┘  └───────────────┘  └────────┬────────┘  │
│  ┌───────────────┐  ┌───────────────┐           │           │
│  │  SettingsHub  │  │   DeviceHub   │◄──────────┘           │
│  │ (owlmic.json) │  │  (WASAPI/VCam)│                       │
│  └───────────────┘  └───────────────┘                       │
+─────────────────────────────────────────────────────────────+
```

---

## 2. Hub & Mailbox Concurrency Model

Both client and server organize functionality into decoupled **Hubs** that communicate strictly through bounded message queues (mailboxes) and broadcast state updates.

### PC Concurrency Rules (Rust)
- **Standard Threads + Bounded Channels**: The PC application uses Rust `std::thread` and bounded channels (`crossbeam_channel` or `std::sync::mpsc`). Unbounded queues are strictly forbidden to prevent memory leaks under backpressure.
- **Strict No-Async Invariant**: Async runtimes (such as Tokio) are **strictly prohibited** in the PC app core. Thread pools and blocking I/O with explicit timeouts provide predictable, deterministic, real-time audio and video delivery.
  - *Exception*: Tokio is permitted exclusively in Linux Bluetooth RFCOMM handling (`pc/src/transport/bt.rs`) due to the async requirements of the `bluer` D-Bus crate.
- **Supervision**: Every hub runs inside a supervised thread loop. If an unhandled panic occurs within a non-critical worker, the supervisor logs the event, cleans up stale handles, and restarts the hub state machine cleanly.

### Android Concurrency Rules (Kotlin)
- **Kotlin Coroutines + Structured Concurrency**: The Android client utilizes `CoroutineScope` tied to the lifecycle of `OwlmicService`.
- **StateFlow / SharedFlow**: The UI never owns business logic. Jetpack Compose observes immutable `StateFlow` emitted by `AppHub`. User actions are dispatched to `AppHub` via simple event calls.
- **Native JNI Isolation**: Low-latency audio loops run in dedicated high-priority native C++ threads (`Oboe` / `AAudio`) with zero JNI crossing in the hot audio buffer path.

---

## 3. The Five Core Hubs

### 1. LinkHub (The Transporter)
- Discovers endpoints via UDP broadcast probes on port 7654.
- Maintains the connection lifecycle over USB debugging, USB tethering, Wi-Fi, and Bluetooth.
- Supervises connection health with 1-second ping/pong heartbeats.
- Implements make-before-break carrier switching.

### 2. SessionHub
- Enforces device identity and cryptographic trust (TOFU).
- Generates ephemeral ECDH keys and verifies mutual authentication MACs.
- Manages pairing approval, pending requests, and device blocklists.
- Implements a 30-second session hold: if a connection temporarily drops, virtual camera and mic handles remain active on the PC while the client reconnects.

### 3. MediaHub
- Manages audio and video encoders/decoders.
- Handles video packet fragmentation (1,200-byte datagrams) and reassembly with NAL unit parsing.
- Runs the adaptive jitter buffer and cubic drift resampler (±0.2%) for microphone audio.
- Mixes and streams PC system audio loopback back to the phone speaker.

### 4. DeviceHub (PC Only)
- Feeds processed microphone PCM audio into the Windows virtual audio endpoint ("Owlmic Mic").
- Writes decoded NV12 video frames into the shared memory ring buffer read by `owlmic_vcam.dll` (Windows 11) or `softcam.dll` (Windows 10).
- Captures default audio output via WASAPI loopback.
- Coordinates one-click elevated repairs for drivers and firewall rules.

### 5. SettingsHub
- Maintains a versioned store of shared and local settings.
- Automatically synchronizes shared settings (e.g., lens, noise reduction, frame rate) between phone and PC via protocol `SETTINGS` frames.
- Persists PC settings to `%APPDATA%\Owlmic\owlmic.json`.

---

## 4. Real-Time & Buffer Budgets

To prevent creeping latency and memory exhaustion:
1. **Audio Latency Budget**: Total round-trip audio buffer depth is clamped to ≤ 20 ms on USB and ≤ 40 ms on Wi-Fi. Audio exceeding the hard drop threshold (200 ms) is purged.
2. **Video Backpressure (`KEEP_ONLY_LATEST`)**: Queue depth for video processing is strictly 1. When the decoder or transport falls behind, older intermediate frames are dropped immediately to favor real-time freshness over playback history.
3. **Socket Timeouts**: Every network socket has an explicit read/write timeout (maximum 3,000 ms). Blocking indefinitely on I/O is strictly disallowed.
