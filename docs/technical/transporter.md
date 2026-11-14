# The Transporter

The **Transporter** is Owlmic's networking engine, implemented in `desktop/crates/owlmic-link` and `android/core/src/main/java/com/owlmic/core/link`. It provides automatic endpoint discovery, multi-carrier transport negotiation, seamless make-before-break carrier switching, and link health monitoring.

---

## 1. Network Topology & Ports

| Port | Protocol | Purpose | Framing |
| :--- | :--- | :--- | :--- |
| **7653** | **TCP** | Control channel on all IP links; Media carrier on USB debugging (via `adb reverse`) | Binary frame (`type(1) \| len(3) \| payload`) |
| **7654** | **UDP** | Discovery beacon (probes and unicast/broadcast answers) | Fixed magic header + device identifiers |
| **7655** | **UDP** | High-performance media datagram carrier for IP links (USB tethering, Wi-Fi) | Media header (`stream(1) \| flags(1) \| seq(4) \| ts(4)`) |
| **RFCOMM** | **Bluetooth** | Serial Port Profile (UUID `00001101-0000-1000-8000-00805F9B34FB`) | Stream-wrapped multiplexed control & media |

---

## 2. Discovery Protocol

Discovery operates without centralized registration. The phone periodically broadcasts UDP probe packets on port 7654, and any listening Owlmic PC responds with a unicast answer.

```
       Phone                                     PC
         │                                       │
         │─────── Probe (UDP 7654 Broadcast) ───►│
         │        Magic: "OWLMIC?3"              │
         │        Phone ID, Phone Name           │
         │                                       │
         │◄────── Answer (UDP Unicast) ──────────│
         │        Magic: "OWLMIC!3"              │
         │        PC ID, Key Hint, Ports, Name   │
```

- **Probe Structure**: Magic `OWLMIC?3` (8 bytes) + Phone ID (16 bytes) + Name Length (1 byte) + Phone Name (UTF-8).
- **Answer Structure**: Magic `OWLMIC!3` (8 bytes) + PC ID (16 bytes) + Key Hint (8 bytes) + TCP Port (2 bytes) + UDP Media Port (2 bytes) + Version (1 byte, `3`) + Flags (1 byte: busy, approval required) + Link ID (1 byte) + Name Length (1 byte) + PC Name (UTF-8).
- **Broadcast Announcements**: When the PC starts up, wakes from sleep, or connects to a new network adapter, it emits three rapid UDP broadcast announcements so phones discover it instantly.

---

## 3. The Four Connection Levels

Connections are ranked by a strict preference score:
```
Priority 1: USB Debugging  (adb reverse tcp:7653 tcp:7653)
Priority 2: USB Tethering  (RNDIS/NCM dynamic IP interface)
Priority 3: Wi-Fi LAN      (Local 802.11 network / hotspot)
Priority 4: Bluetooth      (RFCOMM serial profile, audio only)
```

### Carrier Characteristics

1. **Level 1 — USB Debugging**:
   - The PC's ADB watcher monitors the ADB server (`127.0.0.1:5037`).
   - When a device is authorized, the PC invokes `adb reverse tcp:7653 tcp:7653`.
   - The phone connects to `127.0.0.1:7653`. Both control and media share the TCP stream, with media packets wrapped using `0x80 | length(2)`.
2. **Level 2 — USB Tethering**:
   - Detected by scanning network interfaces for typical tether subnets and interface names.
   - Operates with separate TCP control (:7653) and UDP media (:7655).
3. **Level 3 — Wi-Fi**:
   - Standard LAN networking.
   - All control frames after WELCOME and all UDP media packets are sealed with AES-256-GCM.
4. **Level 4 — Bluetooth RFCOMM**:
   - Audio-only fallback. Video is disabled due to Bluetooth bandwidth constraints.
   - Control frames (<0x80) and media packets (wrapped with 0x80) are multiplexed over a single serial stream.

---

## 4. Make-Before-Break Carrier Migration

When an active session exists on a lower-priority carrier (e.g., Wi-Fi) and a higher-priority carrier becomes available (e.g., USB cable plugged in):

```
       Phone                                     PC
         │  (Active on Wi-Fi: streaming media)   │
         │                                       │
         │─── [USB] HELLO (resume: session_id) ─►│ (Step 1: Probe on USB)
         │◄── [USB] HELLO_ACK ───────────────────│
         │─── [USB] PROOF ───────────────────────►│
         │◄── [USB] WELCOME ─────────────────────│ (Step 2: Proved on USB)
         │                                       │
         │◄── [USB] SWITCH (link: USB) ──────────│ (Step 3: PC issues SWITCH)
         │                                       │
         │=== Media seamlessly shifts to USB === │ (Zero audio drop)
         │  (Old Wi-Fi link closed or standby)   │
```

1. **Concurrent Handshake**: The phone opens a secondary connection on the candidate carrier, setting `resume` to the current `sessionId`.
2. **Key Derivation**: Fresh ephemeral keys and auth tokens are verified for the candidate link.
3. **Switch Command**: Once verified, the PC sends a `SWITCH` (`0x30`) control frame on the *new* link.
4. **Instant Handoff**: Both sides immediately route all subsequent audio and video packets through the new carrier without stopping virtual devices or tearing down pipelines.

---

## 5. Health Monitoring & Recovery Ladder

- **Heartbeat Ping/Pong**: LinkHub transmits `PING` (`0x10`) every 1,000 ms. The receiver echoes back `PONG` (`0x11`) with the same timestamp to track round-trip time (RTT).
- **Timeout & Watchdog**: If no packets or heartbeats arrive for 3,000 ms, the link is declared dead.
- **Session Hold (30 Seconds)**: When a link dies unexpectedly, the PC does *not* teardown virtual camera or microphone devices. It enters a 30-second **Session Hold** state. If the phone reconnects on another carrier within 30 seconds, streaming resumes immediately.
- **Anti-Flap Hysteresis**: If a carrier fails twice in succession, it is placed in a 10-second penalty box before another promotion attempt is allowed.

---

## 6. Blocked PCs & Reconnect

- **Blocking a PC**: The phone's connection list can block any PC it has met, paired or not. The block lives in the phone's store (`blocked`, with the PC's last address), so it survives restarts. A blocked PC is left out of ranking: it is never dialled on its own, never offered as a choice, and a tap on it never connects.
- **Blocking the Connected PC**: The session ends with `BYE`, as when the PC is forgotten, and the phone goes back to searching without it. A handshake with that PC still in flight is dropped.
- **Blocked Behind a Cable**: A USB debugging tunnel only learns which PC it reaches from `HELLO_ACK`. If that PC is blocked, the phone hangs up before `PROOF`, so the PC never pairs or holds a session, and the tunnel waits 60 seconds before it is tried again.
- **Unblocking**: The PC is an ordinary candidate again at once. A PC that was blocked without ever pairing is forgotten entirely.
- **Connection History**: For each PC the phone's store keeps when it was last connected, and the address and link of that connection, for the connection list. The time is saved at the welcome, again every minute while connected (so a crash or a pulled cable leaves a close value) and when the link goes; the address and link follow a switch too. A PC that answered but never connected keeps where and how it answered, and no time.
- **Reconnect**: The phone's hard reconnect. While connected, the links close as a dead link would, so the session is held and dialled again within a second and resumes with fresh keys. While searching, held, refused, busy or asked to update, every backoff and penalty is cleared and the search starts over at once. A handshake already running is left to finish.
