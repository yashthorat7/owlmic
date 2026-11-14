# First Connection & Pairing

Owlmic follows a strict **Trust-On-First-Use (TOFU)** security model. A new phone cannot stream microphone or camera data to your PC without explicit, two-factor numeric approval on your PC desktop.

---

## 1. Quick Pairing Walkthrough

Ensure both your PC and phone are on the same local network (Wi-Fi), or plug your phone into your PC with a USB cable.

```
+──────────────────────────+                  +──────────────────────────+
│       Android Phone      │                  │       Windows PC         │
│                          │                  │                          │
│  1. Launch Owlmic        │                  │  1. Ensure Owlmic runs   │
│  2. Sees PC Beacon       │                  │     in taskbar tray      │
│  3. Displays 4-digit     │                  │  2. Displays approval    │
│     code: "7492"         │ ───► Verify ───► │     prompt: "7492"       │
│                          │                  │  3. Click "Allow"        │
│  Connected!              │ ◄─── Welcome ─── │  Phone paired & trusted! │
+──────────────────────────+                  +──────────────────────────+
```

### Step 1: Open Owlmic on Your PC
- Verify the Owl icon is visible in your Windows system tray.
- If it is not running, launch Owlmic from your Start Menu.

### Step 2: Open Owlmic on Your Phone
- Launch Owlmic on your Android device.
- Within 1 to 2 seconds, your phone will broadcast a discovery beacon (`OWLMIC?3`) and your PC will answer (`OWLMIC!3`).
- If only one PC is detected on the network, Owlmic connects automatically. If multiple PCs are running Owlmic, tap **Choose a PC** to pick your computer.

### Step 3: Verify the 4-Digit Pairing Code
- Your phone screen will display: **"Approve on your PC · XXXX"** (e.g., `Approve on your PC · 4821`).
- On your Windows desktop, a notification and flyout panel will appear:
  > **Pixel 8 wants to connect**  
  > *Check the code on your phone: 4821*  
  > **[ Allow ]** &nbsp;&nbsp;&nbsp;&nbsp; **[ Deny ]**

### Step 4: Click Allow
- Click **Allow** on your PC.
- Both devices establish long-term pairing keys derived from NIST P-256 elliptic-curve cryptography.
- The phone screen turns OLED pure black with active controls, and the PC tray icon turns green (`#30D158`).
- **From this point forward, reconnecting is completely seamless and instant without requiring approval codes.**

---

## 2. Pairing Over USB Cable

If your PC and phone are connected via a USB cable:
1. **USB Tethering** (Fastest & Simplest):
   - Turn on **USB Tethering** in your Android Settings (`Settings -> Network & internet -> Hotspot & tethering -> USB tethering`).
   - Owlmic detects the tethering network interface immediately and connects without wireless encryption overhead.
2. **USB Debugging (ADB)**:
   - Enable **Developer options** and **USB debugging** on your phone.
   - When connected via USB cable, the Owlmic desktop app automatically executes `adb reverse tcp:7653 tcp:7653`.
   - Your phone connects over the lightning-fast USB bridge with ultra-low latency (<10 ms).

---

## 3. Managing Paired Devices

You can view, block, or forget paired devices at any time:
- **On PC**: Click the Owl tray icon -> Open Settings -> **Phones**. You can view paired phones, rename them, or click **Forget** / **Block**.
- **On Phone**: Tap the link tile (top right) to see your PCs: the connected one first, then the others, with blocked PCs at the bottom. Tap a PC to see its IP address and when it last connected, and to **Block** or **Unblock** it. A blocked PC is never connected to until you unblock it. **Reconnect** drops the link and connects again at once. To remove a pairing, long-press a tile to open settings -> **PCs** -> **Forget**.
