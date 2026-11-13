# Connection Levels & Switching

Owlmic features a zero-configuration, multi-tier transporter that automatically connects and switches between four different transport levels based on availability and quality.

---

## 1. The Four Connection Levels

| Priority | Level | Transport | Max Performance | Media Encryption | Best For |
| :---: | :--- | :--- | :--- | :--- | :--- |
| **1** | **USB Debugging** | Direct USB via `adb reverse` | Latency < 10 ms, uncompressed PCM & 1080p60 | Not required (direct local bus) | Gaming, competitive calls, studio streaming |
| **2** | **USB Tethering** | USB cable via standard NDIS/RNDIS | Latency < 15 ms, uncompressed PCM & 1080p60 | Not required (direct local cable) | Everyday wired use without USB debugging enabled |
| **3** | **Wi-Fi (LAN)** | Local Area Network / Hotspot | Latency < 25 ms, Opus 48 kbps & 1080p30/60 | AES-256-GCM (military grade) | Wireless freedom around the room or office |
| **4** | **Bluetooth** | RFCOMM Serial Channel | Latency ~45 ms, Opus 24 kbps (Audio only) | AES-256-GCM authenticated | Fallback when outside Wi-Fi coverage |

---

## 2. Priority Ladder & Auto-Switching

Owlmic implements a **make-before-break** migration model:
- **Upgrades are instantaneous**: If you are using Wi-Fi (Level 3) and plug in a USB cable with USB tethering or USB debugging enabled, Owlmic proves the new USB connection in the background. Once verified, it sends a `SWITCH` control frame, and media instantly shifts to the cable with zero dropped audio samples or video stutter.
- **Graceful Failover**: If you unplug your USB cable while on a call, Owlmic detects the link drop and immediately falls back to Wi-Fi. If Wi-Fi is unavailable or weak, it drops down to Bluetooth RFCOMM to keep your microphone line alive.
- **Anti-Flap Hysteresis**: If a network link is unstable and drops repeatedly, Owlmic places it on a 10-second probationary timeout before attempting to promote to it again, avoiding annoying audio fluttering.

---

## 3. Optimizing Each Connection Level

### Level 1: USB Debugging (Fastest)
1. On your phone: Open **Settings** -> **About Phone** -> Tap **Build Number** 7 times to enable Developer Options.
2. In **Developer Options**, enable **USB debugging**.
3. Plug your phone into the PC. When prompted on your phone, tap **Always allow from this computer**.
4. Owlmic PC automatically opens the port tunnel over USB.

### Level 2: USB Tethering
1. Connect your phone to your PC via a USB-C or USB-A cable.
2. In Android Settings, turn on **USB tethering** (or tap the handy shortcut in Owlmic Settings).
3. Windows will detect a high-speed virtual Ethernet adapter and Owlmic will immediately connect over it.

### Level 3: Wi-Fi
- Connect both devices to the same 5 GHz Wi-Fi router for optimal latency and bandwidth.
- Avoid guest Wi-Fi networks that have "AP Isolation" or "Client Isolation" turned on, as they prevent devices on the same network from discovering each other.

### Level 4: Bluetooth (Audio Fallback)
- Pair your phone and PC in Windows Bluetooth settings.
- If Wi-Fi and cables are disconnected, Owlmic will offer a **"Try Bluetooth"** button to establish an RFCOMM audio link. Note that video is disabled over Bluetooth due to bandwidth limitations.
