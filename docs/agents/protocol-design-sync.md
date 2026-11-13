# Protocol & Design Synchronization - Agent Rules

> **Scope**: Code living in `protocol/` and `design/`.

## 1. Wire Protocol Version 3 Normative Framing

- **Framing**: `type u8 | len u24 BE | payload (JSON, max 1 MiB)`.
- **Ports**:
  - `TCP :7653`: Bidirectional JSON control channel; carrier for USB debugging media.
  - `UDP :7654`: Local discovery broadcast/multicast (`OWLMIC?3` probe, `OWLMIC!3` beacon).
  - `UDP :7655`: Media datagram transport on IP links (Wi-Fi, USB tethering).
- **Control Opcodes**:
  - `0x01` HELLO / `0x02` HELLO_ACK
  - `0x03` PROOF / `0x04` PENDING / `0x05` WELCOME / `0x06` REJECT
  - `0x10` PING / `0x11` PONG
  - `0x12` REPORT
  - `0x20` STATE / `0x21` SETTINGS
  - `0x22` STREAM_START / `0x23` STREAM_STOP
  - `0x24` KEYFRAME_REQUEST / `0x25` RESTART_STREAM
  - `0x30` SWITCH
  - `0x3F` BYE
- **Media Header (10 bytes)**: `stream u8 | flags u8 | seq u32 BE | ts u32 BE (µs) | payload | GCM tag (16 B, wireless)`.
  - Stream `0`: Carrier Hello
  - Stream `1`: Mic Audio (Phone → PC, PCM 48kHz LE on cable, Opus 48kbps on Wi-Fi, Opus 24kbps on BT)
  - Stream `2`: Camera Video (Phone → PC, H.264 Annex B slices $\le$ 1,200 bytes)
  - Stream `3`: Speaker Audio (PC → Phone, PCM 48kHz stereo on cable, Opus 128kbps on Wi-Fi, Opus 64kbps on BT)

---

## 2. Design System Single Sources of Truth (`design/`)

- `design/tokens.json`:
  - OLED Black Palette: Background `#000000`, Tile Surface `#0A0A0A`, Hairline `#262626`.
  - Text Primary `#FFFFFF`, Text Secondary `#8E8E93`, Inactive `#3A3A3C`.
  - Status Indicators: OK `#30D158`, Waiting/Warning `#FFD60A`, Disconnected/Error `#FF453A`.
  - Geometry: $560 \times 255\text{ px}$ fixed panel.
- `design/copy.json`:
  - Canonical UI strings across Android, Windows, and Linux.
  - Product name casing invariant: strictly `Owlmic` or lowercase `owlmic`.
- `design/settings.json`:
  - Versioned schema for synchronized user preferences.

---

## 3. Mandatory Synchronization Rules (Zero Drift)

1. **Vectors and Code Synchrony**:
   - Any modification to wire framing, opcodes, or crypto nonces requires updating `protocol/vectors/*.json`.
   - Both Rust unit tests (`cargo test -p owlmic-proto`) and Kotlin unit tests (`./gradlew :core:test`) validate against these vectors.
2. **Design Tokens Synchrony**:
   - Never hardcode colors, padding, typography, or UI copy directly in Kotlin or Rust.
   - `pc/crates/owlmic-ui/build.rs` compiles `tokens.json` and `copy.json` into Rust constants.
   - `android/core` validates design constants against `design/tokens.json` in `DesignTest.kt`.
