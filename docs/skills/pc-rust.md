# PC Companion App (Rust) - Domain Skill

> **Scope**: Code living in `desktop/crates/*`.

## 1. Architectural Boundaries

- The PC companion application is strictly a **server** and a **per-user tray application** (`owlmic.exe` on Windows, `owlmic` on Linux).
- **Workspace Crates**:
  - `owlmic-app`: CLI dispatch, single-instance mutex, and background orchestration.
  - `owlmic-hub`: Actor-like supervision tree and bounded crossbeam channels.
  - `owlmic-link`: Transporter (TCP `:7653` control & USB carrier, UDP `:7654` discovery, UDP `:7655` wireless media, Linux BT RFCOMM).
  - `owlmic-media`: Jitter buffers, cubic resampler ($\pm 0.2\%$), RNNoise/DSP chain, H.264 slice reassembly, NV12 shared memory.
  - `owlmic-session`: TOFU trust, P-256 ECDH, AES-256-GCM session cryptography.
  - `owlmic-settings`: Configuration store (`%APPDATA%\Owlmic\owlmic.json` on Windows, `$XDG_CONFIG_HOME/owlmic/owlmic.json` on Linux).
  - `owlmic-devices`: Audio endpoints (WASAPI loopback, virtual microphone bridge, device repair).
  - `owlmic-platform`: Shared traits (`VirtualMic`, `VirtualCam`, `AudioLoopback`, `TrayUi`).
  - `owlmic-ui`: Platform UI shell (Win32 Direct2D $560 \times 255\text{ px}$ flyout, tray coordinator).
  - `owlmic-windows`: Windows implementations (WASAPI virtual audio & loopback, MF virtual camera, Direct2D tray).
  - `owlmic-linux`: Linux implementations (PipeWire virtual audio source & sink monitor, `v4l2loopback`, `ksni` tray).
  - `owlmic-vcam`: Windows 11 Media Foundation software camera driver (`owlmic_vcam.dll`).
  - `owlmic-proto`: Protocol serialisation, packet types, and JSON schema test vectors.

---

## 2. Concurrency & Threading Invariants

1. **Standard Threads Only**:
   - Every worker, supervisor, and I/O pump runs on a blocking standard thread (`std::thread::Builder::new().name(...).spawn(...)`).
   - Thread communication is handled exclusively via bounded channels (`crossbeam_channel::bounded(CAP)`).
2. **Strict Tokio Prohibition**:
   - Tokio and any other async runtime are **strictly forbidden** in all PC crates, with **one solitary exception**: `desktop/crates/owlmic-link/src/bt.rs` on Linux (required by the `bluer` D-Bus crate).
   - Any attempt to import or reference `tokio` in `desktop/` outside `bt.rs` fails automated invariant verification (`scripts/verify-repo-invariants.py`).

---

## 3. Real-Time Audio & Video Safety

1. **Audio Constraints**:
   - Resampling is clamped to $\pm 0.2\%$ drift correction using cubic interpolation.
   - Processing pipelines must run within bounded execution time budgets ($\le 10\text{ ms}$ buffers).
2. **Video Constraints**:
   - Single-frame backpressure (`KEEP_ONLY_LATEST`). Intermediate frames must be discarded if the downstream decoder is busy.
   - Frame slices are parsed and reassembled using NAL unit boundaries without heap churn.

---

## 4. Verification

Run before submitting changes:
```bash
cargo fmt --all --check --manifest-path desktop/Cargo.toml
cargo clippy --workspace --all-targets --manifest-path desktop/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path desktop/Cargo.toml
```
