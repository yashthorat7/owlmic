# Linux Backend (Rust) - Domain Skill

> **Scope**: Code living in `desktop/crates/owlmic-linux/` and Linux-specific branches of `desktop/crates/`.

## 1. Linux Subsystem Architecture

The Linux desktop implementation abstracts OS-specific media, camera, and tray facilities behind the shared traits in `owlmic-platform`:

| Subsystem | Linux Implementation (`owlmic-linux`) | Specification |
|---|---|---|
| **Virtual Microphone** | Native userspace **PipeWire / PulseAudio virtual source** | `media.class = "Audio/Source/Virtual"`, node name `"Owlmic Mic"`. Zero kernel module install required. |
| **Virtual Camera** | Direct frame feed to **`v4l2loopback`** device node | Formats NV12 / YUYV to `/dev/videoX` (card label `"Owlmic Cam"`) via standard Linux `ioctl` (`VIDIOC_S_FMT` + `write()`). |
| **Speaker Loopback** | Default audio sink monitor via **PipeWire stream capture** | `STREAM_CAPTURE_SINK` with optional sink muting via `wpctl` / `pactl`. |
| **Tray & Flyout** | **StatusNotifierItem (`ksni`)** + X11/Wayland window | Minimal borderless window matching `shared/design/tokens.json` OLED black layout ($560 \times 255\text{ px}$). |
| **Configuration Path** | XDG Base Directory specification | `$XDG_CONFIG_HOME/owlmic/owlmic.json` (defaults to `~/.config/owlmic/owlmic.json`). |

---

## 2. Invariants & Rules for Linux Development

1. **Userspace First**:
   - PipeWire virtual sources and sink monitors must run entirely in userspace via PipeWire's client API or PulseAudio compatibility layer (`libpulse-simple`).
   - Never require root privileges to run `owlmic` on Linux. Virtual camera output assumes the user has access to `v4l2loopback` (via `video` group or udev rules in `scripts/packaging/linux/`).
2. **Tokio Isolation**:
   - Tokio is **only permitted** inside `desktop/crates/owlmic-link/src/bt.rs` for `bluer` (Linux Bluetooth RFCOMM).
   - All PipeWire and V4L2 pumps must operate on blocking `std::thread` loops with bounded channels.
3. **Packaging Discipline**:
   - Packaging specs live in `scripts/packaging/linux/`:
     - Systemd user service: `owlmic.service`
     - Device setup script: `setup-linux-devices.sh`
   - Linux binary distribution targets portable `.tar.gz` archive and systemd service.

---

## 3. Verification

```bash
cargo check -p owlmic-linux
cargo test -p owlmic-linux
```
