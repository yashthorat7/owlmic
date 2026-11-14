# Windows Backend (Rust) - Domain Skill

> **Scope**: Code living in `desktop/crates/owlmic-windows/`, `desktop/crates/owlmic-vcam/`, and Windows platform integrations.

## 1. Windows Subsystem Architecture

The Windows platform implementation provides virtual audio, virtual camera, and tray facilities implementing the traits from `owlmic-platform`:

| Subsystem | Windows Implementation (`owlmic-windows` / `owlmic-vcam`) | Specification |
|---|---|---|
| **Virtual Microphone** | WASAPI bridge to virtual audio cable | Renders incoming 48 kHz 16-bit PCM mono stream into the virtual mic bridge (`WasapiMic`). VB-CABLE driver installed via installer. |
| **Virtual Camera** | Windows 11 Media Foundation (`IMFVirtualCamera`) & Win 10 DirectShow fallback | High-performance frame feed into `Owlmic Cam` (`WindowsCam`). Win 11 uses native Media Foundation software driver (`owlmic_vcam.dll`); Win 10 uses `softcam.dll`. |
| **Speaker Loopback** | WASAPI loopback capture (`AUDCLNT_STREAMFLAGS_LOOPBACK`) | Captures default render device audio (`WasapiLoopback`) with optional local speaker mute for Quiet PC mode. |
| **Tray & Flyout UI** | Direct2D + Win32 `Shell_NotifyIcon` | Native borderless panel ($560 \times 255\text{ px}$) matching `shared/design/tokens.json` OLED black layout. |
| **Configuration Path** | `%APPDATA%\Owlmic\owlmic.json` | Managed by `owlmic-settings` with atomic file writes. |

---

## 2. Invariants & Implementation Guidelines

1. **Zero Tokio Prohibition**:
   - Like all PC crates outside Linux `bt.rs`, Windows platform code must run on standard blocking threads (`std::thread::Builder`).
   - Cross-thread synchronization is handled via bounded crossbeam channels (`crossbeam_channel::bounded`).
2. **Real-Time WASAPI Safety**:
   - Audio feed loops must maintain steady 10 ms cadence (480 samples at 48,000 Hz).
   - Zero heap allocations inside the hot WASAPI audio callback or feed loop. Pre-allocate sample transfer buffers.
3. **Media Foundation Software Camera (`owlmic-vcam`)**:
   - Compiles to `owlmic_vcam.dll`.
   - Reads decoded NV12 frames directly from the atomic shared memory ring buffer.
   - Must handle `MF_MT_MINIMUM_DISPLAY_APERTURE` clipping to eliminate 16-byte macroblock stride skew and preserve exact aspect ratios.
4. **Driver Caching & Self-Healing**:
   - Driver dependencies (`VB-CABLE`, `softcam.dll`) are verified against immutable SHA-256 hashes in `scripts/packaging/cache/`.
   - `owlmic-devices` monitors endpoint health and triggers automatic repair if virtual endpoints become unregistered or disconnected.

---

## 3. Verification

```bash
cargo check -p owlmic-windows
cargo test -p owlmic-windows
cargo check -p owlmic-vcam
cargo clippy -p owlmic-windows -p owlmic-vcam -- -D warnings
```
