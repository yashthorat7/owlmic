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
│          ▼                │                  │  RNNoise, gate, level, vol │
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
- **Phone Processing**: Android's noise suppressor runs from noise reduction level 5 up. The phone never boosts the mic; the leveling and the volume are applied on the PC.
- **Voice Isolation** (`mic.isolation`, on by default): while the mic and the Speaker are both on, the Speaker's sound is kept out of the mic, so the other side of a call never hears itself.
  - The mic opens on the voice communication preset and the Speaker on voice communication usage, both on Oboe's normal path (the low latency path skips the phone's voice processing on many devices). Android's `AcousticEchoCanceler` is attached to the mic's own session.
  - Out of the phone's own speaker (no headphones), the phone goes into `MODE_IN_COMMUNICATION` with the built-in speaker as the communication device, which most phones need for the canceller to hear what plays; its previous mode comes back when either side stops (`Echo.kt`, `CallMode`).
  - A residual echo suppressor (`EchoSuppressor`) catches what a weak or missing canceller leaves: it learns how loud the Speaker comes back into the mic (the quietest the mic gets, relative to what played in the last 300 ms, over 3 s) and turns the mic down 24 dB while it hears no more than that echo. A voice 6 dB above the echo passes, so both sides can talk at once; where the canceller works, the learned echo is faint and nothing is turned down.
  - Off: the plain mic (voice recognition preset, low latency) and media playback, none of the above.

### PC Audio Processing & Jitter Buffer
- **Adaptive Jitter Buffer (`desktop/crates/owlmic-media/src/audio/mic.rs`)**:
  - Dynamically measures network jitter using arrival timestamps.
  - Target queue depth is adaptively steered between 10 ms (wired USB) and 30 ms (Wi-Fi).
  - Packets arriving out of order are re-sequenced; missing packets trigger Opus Packet Loss Concealment (PLC).
- **Cubic Drift Resampler**:
  - Compares the average fill level of the jitter buffer against crystal oscillator variations between the phone and PC sound cards.
  - Gently speeds up or slows down playback pitch by at most ±0.2% using a smooth cubic interpolator, eliminating audible clicks, skips, or buffer under-runs.
- **DSP Chain** (`desktop/crates/owlmic-media/src/audio/dsp`), in order: RNNoise, noise gate, normalization, volume, soft limiter, one 10 ms frame at a time.
  - **Room Floor** (`floor.rs`): each frame's RMS level, and the room's noise floor under it (the quietest frame of the last 2 s; digital silence teaches it nothing). A frame 10 dB above the floor is speech. The gate and the normalization decide from this, so a far voice is treated like a near one.
  - **Noise Reduction** (`mic.noiseReduction`, a 0 to 10 slider) sets the phone's suppressor, the RNNoise mix (`nnnoiseless`) and the gate:

    | Level | Phone suppressor | PC RNNoise | PC noise gate |
    | :--- | :--- | :--- | :--- |
    | 0 | off | off | off |
    | 1 to 4 | off | 15, 30, 45, 60% mixed in | off |
    | 5 (default) | on | 100%, only when the phone has no suppressor | off |
    | 6 to 10 | on | 100% | closes on what is not 4, 6, 8, 10 or 12 dB above the room |

  - **Noise Gate**: open while frames stand above the room's floor by its margin, and 150 ms after (through the gaps between syllables); it opens in 2 ms and closes over 60 ms, down to -30 dB.
  - **Speech Normalization (AGC)** (`mic.autoLevel`, on by default): brings speech to -20 dBFS RMS from anywhere between about -50 and 0 dBFS (-20 to +30 dB), so how far someone sits from the phone doesn't show. Only speech frames move it: a speech level that follows louder syllables quickly and quieter ones slowly sets the gain, which comes down fast and goes up gently, settling well under a second. Between words the gain holds, so the room never swells up in the pauses. The gain ramps across each frame (no zipper noise), and a frame whose peak would pass 29000 is turned down from its first sample.
  - **Volume** (`mic.volume`, -12 to +12 dB in 3 dB steps, 0 by default): a gain on top of the normalization, smoothed with it, so it works whether Auto level is on or off.
  - **Soft-Knee Limiter**: above 28000 peaks round off before full scale, so neither the normalization nor the volume clips. With every stage off the audio passes untouched.

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
- **Lens (`camera.lens`)**: the front lens by default; a phone or tablet without the lens asked for uses the other one.
- **Mirror preview (`camera.mirrorPreview`)**: on by default, so your own view reads like a mirror; what the PC gets is never mirrored.
- **Ratio (`camera.ratio`)**: `full` (the default) sends everything the sensor captures, at its own aspect, nothing cut: the camera binds the sensor's own frame (4:3 unless its active area is 16:9, read through Camera2 interop), and the encoder takes its shape from the real bound frame, upright for the hold, at the plan's short side (1080p: 1440x1080 sideways, 1080x1440 upright on a 4:3 sensor; 720p: 960x720 and 720x960). The long side is rounded to a multiple of 16. Front and back lenses alike.
  - The presets crop only because the user picked them, and keep their exact shape whichever way the phone is held: 16:9, 4:3, 1:1 or 9:16, at 1080p 1920x1080, 1440x1080, 1080x1080 or 1080x1920 (720p: 1280x720, 960x720, 720x720, 720x1280). 4:3 and 1:1 bind the sensor's 4:3 frame, 16:9 and 9:16 its 16:9 frame, so the crop loses as little of the view as it can.
  - The starting bitrate follows the pixel count. The bound size is given in the sensor's own landscape terms, as CameraX compares it.
- **Orientation (`camera.orientation`)**: Only which way is up. `auto` follows how the phone is held (CameraX `targetRotation`, with 20° hysteresis so a phone near a diagonal does not flip back and forth); `landscape` and `portrait` lock it. Turning the phone never crops or zooms. A full-frame picture flips between 4:3 and 3:4: the new encoder (and STREAM_START) comes with CameraX's new rotation, on the GL thread before the next frame, so the frame and the encoder always agree. A preset keeps its size, so it needs no new encoder.
- **OpenGL Shaders**: Renders the preview and encoder frames on a dedicated GL thread.
  - The camera writes its own transform (vertical flip, sensor angle and, for the front lens, a mirror) into the `SurfaceTexture`'s matrix. When CameraX reports it (`hasCameraTransform`), the renderer takes it out again before its own rotation and crop, as CameraX's own processor does. Without that the picture was turned twice: on its side and stretched to the other shape.
  - The PC always gets the picture upright and never mirrored.
  - The on-screen preview shows exactly what is sent, fitted whole and turned the way the phone is held so it reads upright. Only when fitting leaves empty space is it filled, with a blurred, dimmed duplicate of the same shown picture (never the parts a preset cut away); a picture that fills the preview gets nothing behind it.
  - The preview can go and come back at any time (the eye button turns it off): a surface that goes is released on the GL thread before the call returns, along with the backdrop's textures, and the encoder is not touched.
  - `camera.mirrorPreview` (phone only) flips just the preview. Changing it needs no new encoder and no rebind.
  - The camera counts as bound only once CameraX has bound it, so a config that arrives while a bind is pending binds again instead of being lost.
  - Fill or Fit (`camera.framing`) is the PC's: how the virtual camera, always 1920x1080, shows a picture whose shape differs from its own. Fit (the default) shows all of a 4:3 picture with bars at the sides; Fill crops its top and bottom.
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
