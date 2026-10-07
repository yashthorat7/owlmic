# Media Pipelines

This document describes the capture, encoding, transmission, jitter compensation, decoding, and rendering pipelines for microphone audio, webcam video, and speaker loopback.

---

## 1. Microphone Audio Pipeline

```
+───────────────────────────+                  +────────────────────────────+
│       Android Phone       │                  │         Windows PC         │
│                           │                  │                            │
│  Hardware Mic Array       │                  │  Incoming Audio Datagrams  │
│          │                │                  │             │              │
│  Oboe / AAudio Capture    │                  │  Adaptive Jitter Buffer    │
│  (48 kHz, 16-bit PCM mono)│                  │  (10-30 ms target depth)   │
│          │                │                  │             │              │
│  [USB] PCM  │ [Wi-Fi] Opus│                  │  Cubic Drift Resampler     │
│  (10 ms pkt)│ (48kbps FEC)│                  │  (±0.2% crystal lock)      │
│          │                │                  │             │              │
│          ▼                │                  │  Optional RNNoise DSP      │
│   Network Transmission ───┼─────────────────►│             │              │
│                           │                  │  WASAPI Virtual Render     │
│                           │                  │  "Owlmic Mic (Bridge)"     │
+───────────────────────────+                  +────────────────────────────+
```

### Android Audio Capture
- **Low Latency Native Capture**: Implemented in native C++ via Google's `Oboe` library (`AAudio` backend with `AudioRecord` fallback on older devices).
- **Frame Cadence**: 10 ms chunks (480 samples at 48,000 Hz = 960 bytes raw 16-bit PCM). Over Bluetooth, 20 ms chunks (960 samples) are used to fit baseband frame constraints.
- **Dynamic Encoding**:
  - **USB Connections**: Streamed uncompressed as raw 16-bit little-endian PCM for maximum fidelity and zero CPU overhead.
  - **Wi-Fi**: Encoded in real-time via `libopus` at 48 kbps mono with in-band Forward Error Correction (FEC) and Voice/Audio speech profile.
  - **Bluetooth**: Encoded via `libopus` at 24 kbps mono.

### PC Audio Processing & Jitter Buffer
- **Adaptive Jitter Buffer (`pc/crates/owlmic-media/src/audio/mic.rs`)**:
  - Dynamically measures network jitter using arrival timestamps.
  - Target queue depth is adaptively steered between 10 ms (wired USB) and 30 ms (Wi-Fi).
  - Packets arriving out of order are re-sequenced; missing packets trigger Opus Packet Loss Concealment (PLC).
- **Cubic Drift Resampler**:
  - Compares the average fill level of the jitter buffer against crystal oscillator variations between the phone and PC sound cards.
  - Gently speeds up or slows down playback pitch by at most ±0.2% using a smooth cubic interpolator, eliminating audible clicks, skips, or buffer under-runs.
- **DSP Noise Suppression & Automatic Gain Control (AGC)**:
  - Built-in RNNoise neural network model (`nnnoiseless`) removes background typing, air conditioning, and room hiss without coloring vocal timbre.
  - Can be toggled between "Phone only", "Phone + PC", or "Off".
  - **Speech Normalization (AGC)**: An asymmetric peak envelope follower (8 ms attack, 250 ms release) normalizes phone microphone audio to a consistent -14 dBFS speech target with up to +18 dB boost. Includes a -42 dBFS noise gate to avoid boosting room noise during pauses, a 15 ms gain smoothing filter to eliminate zipper distortion, and a soft-knee limiter to prevent digital clipping.

---

## 2. Webcam Video Pipeline

```
+───────────────────────────+                  +────────────────────────────+
│       Android Phone       │                  │         Windows PC         │
│                           │                  │                            │
│  Camera2 / CameraX Sensor │                  │  Incoming Video Fragments  │
│          │                │                  │             │              │
│  OpenGL ES 2.0 / 3.0 FBO  │                  │  Reassembly Buffer         │
│  (Rotate / Mirror / Crop) │                  │  (Loss detect -> Keyframe) │
│          │                │                  │             │              │
│  MediaCodec Hardware Enc  │                  │  H.264 Slice Decoder       │
│  (H.264 Annex B NALs)     │                  │  (NV12 Frame Converter)    │
│          │                │                  │             │              │
│  Fragmenter (<= 1200 B)   │                  │  Shared Memory Ring Buffer │
│          │                │                  │             │              │
│          ▼                │                  │  Windows 11 MF Virtual Cam │
│   Network Transmission ───┼─────────────────►│  "Owlmic Cam"              │
+───────────────────────────+                  +────────────────────────────+
```

### Android Camera Capture & GPU Processing
- **CameraX Pipeline**: Captures camera frames into a private `SurfaceTexture`.
- **Target Rotation & Aspect Ratio**: Maps orientation (`landscape`, `portrait`, `auto`) to CameraX `targetRotation` and matching resolution bounds, preserving the natural sensor field of view without squash or stretch distortion.
- **OpenGL Shaders**: Renders the preview and encoder frames using a dedicated GPU frame-buffer.
  - Handles rotation with 20° gravity orientation hysteresis so flipping the phone does not jitter the video.
  - Applies user-selected framing: **Fill** (center-cropped to 16:9) or **Fit** (letterboxed with clean black bars).
  - Handles horizontal mirroring for front-facing lenses.
- **Hardware Encoder**: Feeds GPU textures directly into Android's hardware `MediaCodec` H.264 encoder.
  - Generates Annex B byte streams with inline SPS and PPS headers on every IDR keyframe.
  - Configurable bitrate: ~2.5 Mbps for 720p, ~4.5 Mbps for 1080p.

### PC Video Reassembly & Rendering
- **Reassembly Engine (`reassembly.rs`)**:
  - Collects slices into full frames based on `frame_id`, `fragment_index`, and `fragment_count`.
  - If a fragment is lost, the entire frame is dropped and LinkHub throttles a `KEYFRAME_REQUEST` back to the phone (max once every 500 ms).
- **Backpressure & Drop Policy**: Queue depth is strictly 1 (`KEEP_ONLY_LATEST`). Intermediate frames are dropped immediately if the decoder falls behind.
- **Visible Aperture Decoding (`decoder.rs`)**: Decodes H.264 into NV12 buffers, clipping to `MF_MT_MINIMUM_DISPLAY_APERTURE` visible width to eliminate 16-byte macroblock stride skew and preserve exact aspect ratio.
- **Shared Memory Ring (`shared.rs`)**:
  - Writes decoded NV12 frames to a lock-free, atomic shared memory ring buffer.
  - Windows 11 Media Foundation (`owlmic_vcam.dll`) or Windows 10 DirectShow (`softcam.dll`) reads the newest buffer and serves it to client applications (Zoom, Teams, Discord, OBS).

---

## 3. Speaker Loopback Pipeline

Owlmic allows the PC's audio to be monitored or played through the phone's speaker or headphones:
- **PC WASAPI Capture**: Captures the default audio playback endpoint via WASAPI Loopback (`AUDCLNT_STREAMFLAGS_LOOPBACK`).
- **Quiet PC Speakers**: When enabled in settings, Owlmic activates a lightweight virtual mute filter on the PC speakers while routing loopback audio to the phone.
- **Streaming**: Encoded as stereo 48 kHz PCM (cable) or Opus at 128 kbps (Wi-Fi).
- **Phone Playback**: Rendered via low-latency Android `AudioTrack` / `Oboe` sink directly to speaker or connected Bluetooth headphones.
