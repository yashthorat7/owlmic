# Real-Time Media & DSP Pipelines - Domain Skill

> **Scope**: Audio DSP, video codecs, jitter buffering, and real-time media pipelines in `desktop/crates/owlmic-media/`, `android/media/`, and `shared/vendor/opus/`.

---

## 1. Real-Time Audio Pipeline

```text
[Android Phone]                                           [PC Server]
Hardware Mic Array                                        Incoming Audio Datagrams
       │                                                              │
Oboe / AAudio C++ Capture (48 kHz, 16-bit mono)           Adaptive Jitter Buffer (10-30 ms)
       │                                                              │
[USB] PCM  │  [Wi-Fi] Opus (48 kbps)                      Cubic Drift Resampler (±0.2%)
       │                                                              │
Network Transmission (UDP :7655 / TCP :7653) ────────────► RNNoise -> Gate -> AGC -> Limiter
                                                                      │
                                                          WASAPI / PipeWire Virtual Mic
```

### Invariants & Specifications

1. **Cadence & Sampling**:
   - Universal broadcast standard: **48,000 Hz**, 16-bit little-endian PCM.
   - Frame interval: **10 ms** (480 samples mono = 960 bytes). Bluetooth uses **20 ms** (960 samples) for baseband efficiency.
2. **Dynamic Transport Encoding**:
   - **USB Cable**: Raw uncompressed 16-bit PCM (zero CPU encode overhead, maximum fidelity).
   - **Wi-Fi**: `libopus` at 48 kbps mono with in-band Forward Error Correction (FEC) and Voice/Audio speech profile.
   - **Bluetooth**: `libopus` at 24 kbps mono.
   - **Speaker Loopback**: Stereo 48 kHz PCM on cable, Opus 128 kbps on Wi-Fi, Opus 64 kbps on BT.
3. **Android Capture Invariants**:
   - Implemented in C++ via Google Oboe (`android/media/src/main/cpp/`).
   - **Zero heap allocations on the audio callback thread**: all sample transfer buffers and ring queues must be pre-allocated during pipeline initialization.
   - Voice isolation (`mic.isolation`): binds Android `AcousticEchoCanceler` to the microphone session and engages `EchoSuppressor` (-24 dB residual suppression) when the speaker loopback is active.
4. **PC Jitter Buffer & Drift Resampler**:
   - Adaptive jitter buffer targets 10 ms depth on wired links and 20–30 ms on Wi-Fi. Missing packets trigger Opus Packet Loss Concealment (PLC).
   - Cubic drift resampler adjusts playback rate by at most **$\pm 0.2\%$** to lock to the phone's crystal oscillator without audible clicks or skips.
5. **PC DSP Chain Order**:
   - Execute strictly per frame: `Room Noise Floor` -> `RNNoise / nnnoiseless mix` -> `Noise Gate (-30 dB)` -> `Speech Normalization (AGC to -20 dBFS)` -> `Volume gain` -> `Soft-knee limiter (threshold 28,000)`.

---

## 2. Real-Time Video Pipeline

```text
[Android Phone]                                           [PC Server]
CameraX Sensor Surface (GPU)                              Incoming Video Slices (<= 1200 B)
       │                                                              │
OpenGL ES Shaders (Rotate / Aspect / Preview Mirror)      Reassembly Engine (Loss -> Keyframe Req)
       │                                                              │
MediaCodec Hardware H.264 Encoder (Annex B NALs)          H.264 Slice Decoder -> NV12 Frames
       │                                                              │
Fragmenter (<= 1,200 bytes per slice)                     Visible Aperture Stride Clip
       │                                                              │
Network Transmission (UDP :7655 / TCP :7653) ────────────► Atomic Lock-Free Shared Memory Ring
                                                                      │
                                                          MF Virtual Cam / v4l2loopback
```

### Invariants & Specifications

1. **Backpressure & Queue Discipline**:
   - Strictly **`KEEP_ONLY_LATEST`** (queue depth = 1).
   - If downstream encoder or network transmission is congested, intermediate frames must be dropped immediately. Never buffer video frames in memory.
2. **MTU Safety & Slicing**:
   - Max packet payload size is **1,200 bytes**.
   - With 10-byte Owlmic media header and transport headers, datagrams remain strictly under 1,232 bytes, avoiding IP fragmentation on all networks.
3. **Orientation & Framing**:
   - CameraX preview mirror is for phone UI only (`camera.mirrorPreview`). The video feed sent to the PC is **never mirrored** and is **always upright**.
   - Presets maintain exact aspect ratios (16:9, 4:3, 1:1, 9:16). PC applies `Fit` (letterbox) or `Fill` (crop) via virtual camera settings.
4. **Loss Recovery**:
   - If any fragment of an H.264 frame is lost, the entire frame is dropped.
   - PC reassembly engine issues a throttled `KEYFRAME_REQUEST` (`0x24`) back to the phone, rate-limited to at most once per 500 ms.
5. **Visible Aperture Decoding & Shared Memory**:
   - Decoded NV12 frames are clipped to `MF_MT_MINIMUM_DISPLAY_APERTURE` to strip 16-byte macroblock stride skew.
   - Decoded frames are committed to an atomic, lock-free ring buffer for zero-copy consumption by `owlmic_vcam.dll` or `v4l2loopback`.

---

## 3. Verification

```bash
# Rust Media Crate
cargo test -p owlmic-media
cargo clippy -p owlmic-media -- -D warnings

# Android Media Module (unit tests & lint only; do NOT build APKs)
cd android
./gradlew :media:testDebugUnitTest :media:lintDebug
```
