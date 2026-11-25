# Settings & State Synchronization

This document covers Owlmic's configuration management, versioned key-value store, persistence model, and bidirectional synchronization across devices.

---

## 1. Single Source of Truth (`design/settings.json`)

All setting keys, allowable value enums, scopes, and target UI visibility rules are declared centrally in `design/settings.json`. Both the Android client and the Windows application compile their setting models directly from this schema.

### Schema Fields
- `id`: Dotted string identifier (e.g., `camera.quality`, `mic.noiseReduction`).
- `values`: String enum of allowable values.
- `default`: The fallback default setting value.
- `scope`:
  - `shared`: Synchronized between phone and PC. Changing the value on either device updates the other.
  - `phone`: Local to the phone.
  - `pc`: Local to the Windows PC.
- `shownOn`: Array indicating where the setting is exposed in the UI (`["phone", "pc"]` or single platform).

### Settings Inventory

| Setting ID | Values | Default | Scope | Shown On | Description |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `mic.noiseReduction` | `0` to `10` | `5` | shared | phone, pc | Noise reduction strength: 0 is off; 1 to 4 PC only and gentle; 5 the phone's suppressor; 6 to 10 adds the PC's denoiser and a noise gate that wants more above the room at each level ([media.md](media.md)) |
| `mic.volume` | `-12`, `-9`, `-6`, `-3`, `0`, `3`, `6`, `9`, `12` | `0` | shared | phone, pc | Volume in dB, applied on the PC after the normalization |
| `mic.isolation` | `on`, `off` | `on` | shared | phone, pc | Keeps the Speaker's sound out of the mic: Android's echo canceller in communication mode, with the phone's own echo suppressor behind it |
| `mic.autoLevel` | `on`, `off` | `on` | shared | phone, pc | The PC's speech normalization (AGC): the voice at one loudness from near or far |
| `camera.lens` | `back`, `front` | `front` | shared | phone, pc | Active camera lens |
| `camera.quality` | `auto`, `480p`, `720p`, `1080p` | `auto` | shared | phone, pc | Video streaming resolution |
| `camera.frameRate` | `24`, `30`, `60` | `24` | shared | phone, pc | Video capture frame rate |
| `camera.orientation` | `auto`, `landscape`, `portrait` | `auto` | shared | phone | Camera sensor orientation lock |
| `camera.ratio` | `full`, `16:9`, `4:3`, `1:1`, `3:4`, `9:16` | `full` | shared | phone, pc | Shape of the picture the PC receives: the sensor's whole frame, or a crop the user picks |
| `camera.framing` | `fill`, `fit` | `fit` | pc | pc | How the PC's 16:9 virtual camera shows a picture of another shape: `fit` (the default) shows all of it with bars, `fill` crops it |
| `camera.mirror` | `off`, `on` | `off` | shared | phone, pc | Horizontal video mirroring synced between phone preview and PC preview/virtual camera |
| `speaker.quietPc` | `on`, `off` | `on` | pc | pc | Mute PC physical speakers during loopback |
| `link.usbDebugging` | `on`, `off` | `on` | shared | phone, pc | Allow Level 1 ADB transport |
| `link.usbTethering` | `on`, `off` | `on` | shared | phone, pc | Allow Level 2 USB tethering transport |
| `link.wifi` | `on`, `off` | `on` | shared | phone, pc | Allow Level 3 Wi-Fi transport |
| `link.bluetooth` | `on`, `off` | `on` | shared | phone, pc | Allow Level 4 Bluetooth transport |
| `general.startWithWindows` | `on`, `off` | `on` | pc | pc | Windows Run registry autostart |

---

## 2. Synchronization Protocol

Shared settings synchronize via `SETTINGS` (`0x21`) control messages:

```json
{
  "changes": [
    {
      "id": "camera.quality",
      "value": "1080p",
      "version": 4
    }
  ]
}
```

### Versioning & Conflict Resolution
- Every change to a setting increments its local 64-bit integer version counter.
- When a `SETTINGS` frame is received, the receiver compares incoming versions against its current state.
- **Higher Version Wins**: The newer version replaces the existing value.
- **Tie Breaker**: If versions are identical, the PC's setting value takes precedence to ensure determinism.

### Offline & Transport Link Configuration
- Transport links (`link.usbDebugging`, `link.usbTethering`, `link.wifi`, `link.bluetooth`) can be toggled on the PC companion at any time, even before a phone connects.
- While offline, transport settings persist to the PC's general settings (`d.general`). Upon phone connection, active transport preferences automatically synchronize to the phone pairing book and link monitors.

---

## 3. Storage & Persistence

### Windows PC
- Stored as human-readable JSON in `%APPDATA%\Owlmic\owlmic.json`.
- Holds:
  - Local PC preferences.
  - Per-phone paired device records, including phone name, static public key, last-used link, and paired setting book.
- Flushed to disk atomically using write-to-temp and rename to prevent corruption on unexpected power loss.

### Android Phone
- Stored in private application storage using Android `SharedPreferences` or EncryptedSharedPreferences.
- Cleared automatically if the user uninstalls the app or clears app data.
