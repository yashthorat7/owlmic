# Owlmic protocol, version 3

The contract between the Android app and the Windows app. Each side implements it in its own language; both test suites check themselves against the files in [`vectors/`](vectors/).

All integers are big-endian. All text is UTF-8.

## 1. Ports

| Port | Use |
| :--- | :--- |
| TCP 7653 | Control channel; on USB debugging also the media channel (through `adb reverse`) |
| UDP 7654 | Discovery |
| UDP 7655 | Media on IP links (USB tethering, Wi-Fi) |

## 2. Discovery

The phone asks, the PC answers. The PC also sends three announcements (an answer to the broadcast address) when it starts, wakes or joins a network.

**Probe** (phone → UDP 7654, broadcast on every interface):

| Bytes | Field |
| :--- | :--- |
| 8 | Magic `OWLMIC?3` |
| 16 | Phone id |
| 1 | Name length |
| n | Phone name (at most 255 bytes) |

**Answer** (PC → the probe's sender, unicast):

| Bytes | Field |
| :--- | :--- |
| 8 | Magic `OWLMIC!3` |
| 16 | PC id |
| 8 | Key hint: the first 8 bytes of SHA-256 of the PC's public key |
| 2 | TCP control port |
| 2 | UDP media port |
| 1 | Protocol version (3) |
| 1 | Flags: bit 0 busy (another phone is connected), bit 1 approval required (this phone is not approved yet) |
| 1 | Link the probe arrived over: 2 USB tethering, 3 Wi-Fi |
| 1 | Name length |
| n | PC name (at most 255 bytes) |

Anything that does not start with a magic, or is shorter than its fixed part, is ignored.

## 3. Channels

Every TCP connection to port 7653 starts with one **channel byte**: `0x01` control, `0x02` media. On USB debugging both channels travel through the same `adb reverse` rule. Bluetooth (RFCOMM) has no channel byte: it carries control and audio on one stream, and control frames and media packets are told apart by their first byte (control types are below `0x80`, wrapped media packets start with `0x80`).

## 4. Control frames

```
type (1) | length (3) | payload (length bytes, at most 1 MiB)
```

Payloads are JSON objects with camelCase keys. Binary values (keys, nonces, MACs) are base64. Unknown types and unknown keys are ignored, so later versions can add them.

| Type | Message | Direction | Payload |
| :---: | :--- | :--- | :--- |
| `0x01` | HELLO | Phone → PC | `proto`, `phoneId`, `name`, `model`, `staticPub`, `ephPub`, `nonce`, `link`, optional `resume` |
| `0x02` | HELLO_ACK | PC → Phone | `proto`, `pcId`, `name`, `staticPub`, `ephPub`, `nonce`, `status` (`known`, `new`, `busy`, `blocked`), optional `btAddr` |
| `0x03` | PROOF | Phone → PC | `mac` |
| `0x04` | PENDING | PC → Phone | `code` (4 digits) |
| `0x05` | WELCOME | PC → Phone | `sessionId`, `mac`, `settings`, `caps` |
| `0x06` | REJECT | PC → Phone | `reason` (`busy`, `denied`, `blocked`, `version`), optional `owner` (busy), optional `proto` (version: the PC's own version, so the phone can say which side to update) |
| `0x10` | PING | Both | `t` (sender's clock, microseconds) |
| `0x11` | PONG | Both | `t` (echoed) |
| `0x12` | REPORT | Both | `lossPct`, `jitterMs`, `rttMs`, `kbps` (per stream), optional `thermal` |
| `0x20` | STATE | Both | `mic`, `camera`, `speaker`: `off`, `on` or `paused` |
| `0x21` | SETTINGS | Both | `changes`: list of `{id, value, version}` |
| `0x22` | STREAM_START | Both | `stream`, `codec`, codec parameters |
| `0x23` | STREAM_STOP | Both | `stream` |
| `0x24` | KEYFRAME_REQUEST | PC → Phone | `{}` |
| `0x25` | RESTART_STREAM | Both | `stream` |
| `0x30` | SWITCH | PC → Phone | `link`; sent on the link the session moves to |
| `0x3F` | BYE | Both | optional `reason` |

### Handshake

```
Phone                                   PC
  │── HELLO ───────────────────────────►│
  │◄─────────────────────────── HELLO_ACK│
  │── PROOF ───────────────────────────►│
  │◄──────────── PENDING (new phone only)│
  │◄────────────────── WELCOME or REJECT │
  │── carrier hello on the media channel►│
```

A handshake step that does not finish in 3 seconds is abandoned and retried. The approval wait is the exception: while PENDING, the phone sends nothing and the PC keeps the connection open for up to 2 minutes for the user to answer.

### Links of one session

A session can have more than one link at a time: the active one and a warm standby, or a better link being proven before a switch. Each link runs its own handshake (HELLO with `resume` set to the session id) and has its own session keys. Right after its WELCOME, each link sends its own carrier hello, with that link's `auth` key. The PC moves the session by sending SWITCH on the link it moves to; from then on both sides send media on that link and keep the old one only as standby.

### Settings after WELCOME

Right after WELCOME, the PC sends SETTINGS with every shared setting and its version, so both sides start from the same versions. WELCOME's `settings` carries the values only, for applying at once.

## 5. Keys

HKDF below is HKDF-SHA256 (RFC 5869) with a 32-byte output. ECDH on P-256 yields the 32-byte x-coordinate of the shared point. `‖` joins bytes; quoted strings are ASCII.

- **Identity:** each device keeps one P-256 key pair. Public keys travel as 65-byte uncompressed points (`04 ‖ x ‖ y`).
- **Key hint:** the first 8 bytes of SHA-256(public key).
- **Pairing key** `K` = HKDF(salt = `"owlmic pair v3"`, ikm = ECDH(phone static, PC static), info = phone id ‖ PC id).
- **Transcript** `T` = SHA-256(HELLO payload ‖ HELLO_ACK payload), over the exact bytes sent.
- **Session master** = HKDF(salt = phone nonce ‖ PC nonce, ikm = `K` ‖ ECDH(phone ephemeral, PC ephemeral), info = `"owlmic session v3"`).
- **Session keys:** HKDF-Expand(PRK = session master, info, 32) with info `"phone->pc"` and `"pc->phone"` (the AES-256-GCM keys for each direction) and `"auth"` (the proof key).
- **Proofs:** the phone's `mac` = HMAC-SHA256(auth, `"phone"` ‖ `T`); the PC's `mac` = HMAC-SHA256(auth, `"pc"` ‖ `T`).
- **Approval code:** the first 4 bytes of HMAC-SHA256(`K`, `"code"` ‖ `T`) as an unsigned big-endian integer, modulo 10000, written with 4 digits.
- **Carrier hello MAC:** the first 16 bytes of HMAC-SHA256(auth, `"carrier"` ‖ session id).

On wireless links everything after WELCOME is encrypted: control payloads with nonce stream byte `0xFF` and the 4-byte frame header as additional data (its length counts the 16-byte tag), media as described below. HELLO through WELCOME travel in the clear; they carry only public values and MACs.

## 6. Media packets

```
stream (1) | flags (1) | seq (4) | timestamp_us (4, wrapping) | payload | GCM tag (16, wireless only)
```

| Stream | Content |
| :---: | :--- |
| 0 | Carrier hello: payload = session id (16) ‖ HMAC-SHA256(auth, `"carrier"` ‖ session id), truncated to 16 bytes |
| 1 | Mic audio, phone → PC |
| 2 | Camera video, phone → PC |
| 3 | Speaker audio, PC → phone |

Flags: bit 0 is set on video fragments of a keyframe. Other bits are zero.

**Video** payloads start with a 6-byte fragment header. A fragment carries at most 1,200 bytes of H.264, so a datagram stays within 1,232 bytes, and a picture has at most 4,096 fragments (about 4.9 MB); receivers ignore fragments that claim more:

```
frame (2) | index (2) | count (2) | H.264 bytes
```

**On stream carriers** (USB debugging, Bluetooth) each media packet is preceded by `0x80` and a 2-byte length.

**Encryption** (Wi-Fi and Bluetooth): every media packet except the carrier hello (stream 0, which proves itself with its own MAC) is sealed with AES-256-GCM using the sender's direction key; nonce = stream (1) ‖ three zero bytes ‖ the packet's `seq` as a 64-bit number; additional data = the 10-byte header. A receiver drops a packet whose `seq` it has already accepted or that is older than its 64-packet replay window. Control frames use stream byte `0xFF` and a per-direction counter that starts at 0 after WELCOME. USB links are not encrypted; the handshake proves who is on the other end. A sender never reuses a `seq` under the same keys: before a stream's `seq` would wrap, the link is closed and handshaken again, which brings new keys.

### Streams

A sender announces each stream with STREAM_START before its first packet.

| Stream | Link | STREAM_START | Payload |
| :--- | :--- | :--- | :--- |
| 1 Mic | USB debugging, USB tethering | `{"stream":1,"codec":"pcm","sampleRate":48000,"channels":1,"frameMs":10}` | Signed 16-bit little-endian mono, 10 ms per packet |
| 1 Mic | Wi-Fi | `{"stream":1,"codec":"opus","sampleRate":48000,"channels":1,"frameMs":10}` | Opus, 48 kbps, in-band FEC |
| 1 Mic | Bluetooth | `{"stream":1,"codec":"opus","sampleRate":48000,"channels":1,"frameMs":20}` | Opus, 24 kbps |
| 2 Camera | All but Bluetooth | `{"stream":2,"codec":"h264","width":W,"height":H,"fps":F}` | H.264 Annex B; every keyframe carries SPS and PPS |
| 3 Speaker | USB debugging, USB tethering | `{"stream":3,"codec":"pcm","sampleRate":48000,"channels":2,"frameMs":10}` | Signed 16-bit little-endian, interleaved stereo |
| 3 Speaker | Wi-Fi | `{"stream":3,"codec":"opus","sampleRate":48000,"channels":2,"frameMs":10}` | Opus, 128 kbps, in-band FEC |
| 3 Speaker | Bluetooth | `{"stream":3,"codec":"opus","sampleRate":48000,"channels":2,"frameMs":20}` | Opus, 64 kbps |

- The mic's STREAM_START may add `"noiseSuppressor": false` when the phone has no noise suppressor of its own; the PC then adds its own when the setting asks for noise reduction on the phone.
- The PC asks for a keyframe with KEYFRAME_REQUEST.
- **Pausing:** either side may send STATE with a feature set to `paused`. The phone pauses that feature and answers with the same STATE, and reports `on` again when the user resumes it.

## 7. Versions

The protocol version is independent of the app version. A PC accepts phones with the same protocol version and answers others with `REJECT version`.

## 8. Test vectors

| File | Covers |
| :--- | :--- |
| [`vectors/discovery.json`](vectors/discovery.json) | Probes and answers, plus packets that must be ignored |
| [`vectors/frames.json`](vectors/frames.json) | Control frame headers, media headers, video fragment headers, stream-carrier wrapping |
| [`vectors/messages.json`](vectors/messages.json) | One example payload for every control message |
| [`vectors/crypto.json`](vectors/crypto.json) | Every derived key, proof, code and MAC, plus AES-GCM sealing, generated with an independent library |
