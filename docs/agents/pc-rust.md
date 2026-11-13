# PC Companion App (Rust) - Agent Rules

> **Scope**: Code living in `pc/crates/*` and `pc/vcam/`.

## 1. Architectural Boundaries

- The PC companion application is strictly a **server** and a **per-user tray application** (`owlmic.exe`).
- **Workspace Crates**:
  - `owlmic-app`: CLI dispatch, single-instance mutex, and background orchestration.
  - `owlmic-hub`: Actor-like supervision tree and bounded crossbeam channels.
  - `owlmic-link`: Transporter (TCP `:7653` control & USB carrier, UDP `:7654` discovery, UDP `:7655` wireless media, Linux BT RFCOMM).
  - `owlmic-media`: Jitter buffers, cubic resampler ($\pm 0.2\%$), RNNoise/DSP chain, H.264 slice reassembly, NV12 shared memory.
  - `owlmic-session`: TOFU trust, P-256 ECDH, AES-256-GCM session cryptography.
  - `owlmic-settings`: Configuration store (`%APPDATA%\Owlmic\owlmic.json` on Windows, `$XDG_CONFIG_HOME/owlmic/owlmic.json` on Linux).
  - `owlmic-devices`: Audio endpoints (WASAPI loopback, virtual microphone bridge, repair script).
  - `owlmic-platform`: Shared traits (`VirtualMic`, `VirtualCam`, `AudioLoopback`, `TrayUi`).
  - `owlmic-win`: Windows implementations (WASAPI, Win32 GDI $560 \times 255\text{ px}$ flyout, shared memory camera ring).
  - `owlmic-linux`: Linux implementations (PipeWire virtual audio source & sink monitor, `v4l2loopback`, `ksni` tray).
  - `owlmic-vcam`: Windows 11 Media Foundation software camera driver (`owlmic_vcam.dll`).

---

## 2. Concurrency & Threading Invariants

1. **Standard Threads Only**:
   - Every worker, supervisor, and I/O pump runs on a blocking standard thread (`std::thread::Builder::new().name(...).spawn(...)`).
   - Thread communication is handled exclusively via bounded channels (`crossbeam_channel::bounded(CAP)`).
2. **Strict Tokio Prohibition**:
   - Tokio and any other async runtime are **strictly forbidden** in all PC crates, with **one solitary exception**: `pc/crates/owlmic-link/src/bt.rs` on Linux (required by the `bluer` D-Bus crate).
   - Any attempt to import or reference `tokio` in `pc/` outside `bt.rs` fails automated invariant verification (`scripts/verify-repo-invariants.py`).

---

## 3. Real-Time Audio & Video Safety

1. **Audio Path Rules**:
   - Audio capture, processing, and playback threads must **never block** on network I/O, disk operations, or mutexes held by other threads.
   - Zero dynamic heap allocations in hot audio loops (`AudioRing`, DSP transforms, resampling). Pre-allocate static or bounded buffers at stream initialization.
   - Handoffs must be bounded, lock-free, or non-blocking try-locks (`try_send` / `try_recv`). When full, drop the oldest sample to preserve real-time latency.
2. **Video Backpressure Invariant**:
   - Video backpressure strategy is strictly `KEEP_ONLY_LATEST` (queue depth = 1).
   - If rendering or transport falls behind, drop intermediate frames immediately. Never accumulate frames in an unbounded queue.
3. **Socket Timeouts**:
   - Every network socket must specify an explicit read and write timeout (maximum 3,000 ms). Never use indefinite blocking network calls.

---

## 4. UI & Rendering Rules (`owlmic-ui`)

- Panel geometry is fixed at **$560 \times 255\text{ px}$** positioned above the system tray.
- Rendering uses double-buffered Win32 GDI with zero webview overhead.
- Colors, dimensions, and typography must be derived from `design/tokens.json`.
- State reflects `SessionHub` via lock-free or try-lock snapshots; the UI thread never mutates core network or DSP states directly.

---

## 5. Verification Commands

Before proposing any changes to `pc/`:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
