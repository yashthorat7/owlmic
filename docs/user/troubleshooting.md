# Troubleshooting

This guide provides practical solutions for common connection, audio, camera, and driver issues.

---

## 1. Quick Self-Repair: One-Click Fix

If Owlmic Mic or Owlmic Cam shows an error, or if your phone cannot discover your PC:
1. Click the **Owl icon** in your Windows taskbar system tray.
2. If any component needs attention, a yellow or red banner will appear:
   - **"Owlmic Mic needs a repair"**
   - **"Owlmic Cam needs a repair"**
   - **"Your phone can't reach this PC. Repair to allow it."**
3. Click the **Repair** button.
4. When prompted by Windows User Account Control (UAC), click **Yes**.
5. Owlmic will run a single elevated script that automatically:
   - Verifies and restores the virtual audio cable driver.
   - Cleans up corrupted Windows audio endpoint registry entries.
   - Re-registers `owlmic_vcam.dll` and `softcam.dll` virtual camera filters.
   - Opens inbound rules on Windows Defender Firewall for ports TCP 7653 and UDP 7654/7655.

---

## 2. Connection & Discovery Issues

### "Looking for your PC" / "Open Owlmic on your PC"
- **Ensure Owlmic is running on Windows**: Check that the Owl icon appears in the bottom-right tray near the clock.
- **Check Wi-Fi Network**: Both phone and PC must be connected to the same Wi-Fi router.
  - If your router has **Guest Mode** or **Client Isolation** enabled, devices cannot see each other. Switch to the primary network.
  - Ensure Windows network profile is set to **Private**, not Public.
- **Direct IP Pairing**:
  - In Owlmic PC tray panel, view **This PC's addresses** (e.g., `192.168.1.145`).
  - In the Android app, tap the connection tile (top right) -> `+` (**Add PC by address**) -> enter the IP address.

### USB Connection Not Recognized
- **USB Tethering**: If plugged in via USB cable, enable **USB tethering** on your Android phone under `Settings -> Network & Internet -> Hotspot & tethering`.
- **USB Debugging**: If using ADB, verify your PC recognizes the device by running `adb devices` in a terminal. Make sure you tap **Always allow from this computer** on the phone screen.

---

## 3. Audio / Microphone Issues

### No Sound in Meeting Apps (Discord, Zoom, Teams)
1. Verify the microphone is turned **ON** in the Owlmic Android app (tile is white and audio meter bounces when you speak).
2. In your meeting app's settings, ensure the input device is set to **Owlmic Mic (Owlmic Bridge)**, not "Default Device".
3. Check Windows Sound Settings:
   - Open `Settings -> System -> Sound -> Input`.
   - Ensure **Owlmic Mic** volume is set to 100%.

### Robotic or Stuttering Audio Over Wi-Fi
- Wi-Fi congestion or interference can cause packet loss.
- Switch to a 5 GHz Wi-Fi band or connect via USB cable for zero jitter.

### The Other Side Hears Themselves (Echo)
- This happens when the phone plays the call (the **Speaker** tile) and its mic picks it up again.
- Keep **Voice isolation** on in the mic settings: it cancels the phone's own sound out of the mic, and turns the mic down when only that sound reaches it.
- Headphones on the phone remove the echo completely.

### Your Voice Is Too Quiet or Too Loud
- Keep **Auto level** on: it brings your voice to the same loudness whether you sit close to the phone or a few metres away.
- If it is still not right for your meeting app, use the **Volume** slider in the mic settings (it applies on top of Auto level).

---

## 4. Camera / Video Issues

### "Owlmic Cam" Shows Black Screen or Placeholder
- The placeholder screen (centered Owlmic owl symbol on black) is displayed whenever the camera is turned **OFF** on your phone.
- Tap the **Camera** tile on your phone to turn it ON.
- If prompted, ensure you have granted Android camera permissions.

### Camera Not Showing in Windows 11 Apps
- Windows 11 uses the modern Media Foundation virtual camera. If an older 32-bit app or DirectShow app does not see it, run the **Repair** action in the Owlmic tray to ensure both `owlmic_vcam.dll` and `softcam.dll` are registered.

---

## 5. Background Streaming Stops Unexpectedly

If streaming stops after locking your phone or putting it in your pocket:
- Android battery savers often terminate background sockets.
- Go to Android **Settings** -> **Apps** -> **Owlmic** -> **Battery** -> Select **Unrestricted**.
