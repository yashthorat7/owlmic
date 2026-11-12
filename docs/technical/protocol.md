# Wire Protocol Version 3

This document is the normative technical specification for **Owlmic Wire Protocol Version 3**.
Official test vectors are maintained in [`protocol/vectors/`](../../protocol/vectors/).

All integers are big-endian (BE). All text strings are UTF-8 encoded.

---

## 1. Transport Channels

- **TCP Port 7653**: The primary control stream. On initial connection, the client sends a single **Channel Byte**:
  - `0x01`: Control Channel.
  - `0x02`: Media Channel (used exclusively over USB debugging via `adb reverse`).
- **UDP Port 7654**: Discovery beacon (probes, answers, announcements).
- **UDP Port 7655**: Media datagram channel on IP links (USB tethering and Wi-Fi).
- **Bluetooth RFCOMM**: Serial stream without a channel byte. Media packets are prefixed by `0x80 | length(2)`, while control frames begin with types `< 0x80`.

---

## 2. Control Frame Format

Control messages are framed as:
```
+───────────────+─────────────────────────+──────────────────────────────+
│  type (1 B)   │       length (3 B)      │       payload (JSON)         │
+───────────────+─────────────────────────+──────────────────────────────+
```
- **Type**: 1-byte opcode (`0x01` through `0x3F`).
- **Length**: 3-byte unsigned big-endian integer (maximum length 1 MiB = `0x100000`).
- **Payload**: JSON object with `camelCase` keys. Unrecognized types or JSON keys are ignored for forward-compatibility.

### Control Frame Message Table

| Opcode | Name | Direction | Payload Properties | Description |
| :---: | :--- | :--- | :--- | :--- |
| `0x01` | **HELLO** | Phone → PC | `proto`, `phoneId`, `name`, `model`, `staticPub`, `ephPub`, `nonce`, `link`, optional `resume` | Initiates session or link addition |
| `0x02` | **HELLO_ACK** | PC → Phone | `proto`, `pcId`, `name`, `staticPub`, `ephPub`, `nonce`, `status`, optional `btAddr` | Answers HELLO with PC public keys |
| `0x03` | **PROOF** | Phone → PC | `mac` | Authenticates handshake transcript |
| `0x04` | **PENDING** | PC → Phone | `code` (4 digits) | Sent when new phone awaits user desktop approval |
| `0x05` | **WELCOME** | PC → Phone | `sessionId`, `mac`, `settings`, `caps` | Mutual authentication complete; session active |
| `0x06` | **REJECT** | PC → Phone | `reason` (`busy`, `denied`, `blocked`, `version`), optional `owner`, optional `proto` | Handshake rejected |
| `0x10` | **PING** | Both | `t` (microsecond clock) | Latency tracking |
| `0x11` | **PONG** | Both | `t` (echoed microsecond clock) | Latency response |
| `0x12` | **REPORT** | Both | `lossPct`, `jitterMs`, `rttMs`, `kbps`, optional `thermal` | Network and hardware metrics |
| `0x20` | **STATE** | Both | `mic`, `camera`, `speaker`: `"off"`, `"on"`, or `"paused"` | Hardware streaming states |
| `0x21` | **SETTINGS** | Both | `changes`: `[{"id", "value", "version"}]` | Versioned settings synchronization |
| `0x22` | **STREAM_START** | Both | `stream`, `codec`, parameters (`sampleRate`, `channels`, `width`, `height`, `fps`) | Announces incoming media stream |
| `0x23` | **STREAM_STOP** | Both | `stream` | Halts media stream |
| `0x24` | **KEYFRAME_REQ**| PC → Phone | `{}` | Requests immediate H.264 IDR keyframe |
| `0x25` | **RESTART_STREAM**| Both | `stream` | Requests clean decoder restart |
| `0x30` | **SWITCH** | PC → Phone | `link` | Commands session traffic migration to target link |
| `0x3F` | **BYE** | Both | optional `reason` | Graceful disconnection |

---

## 3. Media Packet Format

Media datagrams transmitted over UDP (or framed on stream carriers) use a fixed 10-byte header:
```
+────────────+────────────+────────────+───────────────────+───────────+──────────────────────+
│ stream(1B) │ flags(1B)  │ seq (4B)   │  timestamp (4B)   │  payload  │  GCM tag (16B, Wi-Fi)│
+────────────+────────────+────────────+───────────────────+───────────+──────────────────────+
```
- **Stream**:
  - `0`: Carrier Hello (`sessionId` + HMAC proof).
  - `1`: Microphone Audio (Phone → PC).
  - `2`: Camera Video (Phone → PC).
  - `3`: Speaker Audio (PC → Phone).
- **Flags**: Bit 0 set indicates video fragments belonging to an IDR keyframe. Other bits zero.
- **Seq**: 4-byte unsigned monotonic sequence number.
- **Timestamp**: 4-byte microsecond capture timestamp (wrapping).
- **GCM Tag**: 16-byte authentication tag present only on wireless encrypted transports.

### Video Fragmentation
H.264 NAL units are fragmented to ensure UDP datagrams fit within standard network MTUs (≤ 1,200 bytes of payload per packet):
```
+────────────────+────────────────+────────────────+─────────────────────────────────────+
│ frame_id (2 B) │ fragment (2 B) │  count (2 B)   │          H.264 slice data           │
+────────────────+────────────────+────────────────+─────────────────────────────────────+
```
- Receivers drop frames exceeding 4,096 fragments (~4.9 MiB).
- An incomplete frame is dropped after 2 expected frame intervals, prompting a `KEYFRAME_REQUEST`.

---

## 4. Encryption & Nonce Rules

Wireless links (Wi-Fi and Bluetooth) encrypt all control frames after WELCOME and all media packets using AES-256-GCM.
- **Media Nonce Construction**:
  `stream (1 byte) || 0x00 0x00 0x00 (3 bytes) || seq (8-byte BE unsigned integer)`.
- **Control Nonce Construction**:
  `0xFF (1 byte) || 0x00 0x00 0x00 (3 bytes) || counter (8-byte BE unsigned integer)`.
- **Replay Protection**: Receivers enforce a 64-packet bitmask replay window.
