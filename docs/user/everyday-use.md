# Everyday Use

Once paired, Owlmic operates transparently. This guide covers daily controls, switching cameras, muting/unmuting, configuring in meeting apps, and using the speaker loopback.

---

## 1. Daily Connection Workflow

1. Unlock your phone and open **Owlmic** (or tap the persistent notification if background service is running).
2. Within seconds, the app connects to your PC and displays the main dashboard.
3. The mic, camera, and speaker start in the **OFF** state by default for privacy.

---

## 2. Phone Controls & Dashboard

```
+------------------------+-----+-----+
| OWLMIC                 |  ?  |  @  |   ? = Setup & Help, @ = link tile
+------------------------+-----+-----+
|                                    |
|            preview area            |
|   (owl, camera, sound wave and     |
|    status messages show here)      |
|                                    |
+------------+-----------+-----------+
|   camera   |    mic    |  speaker  |
+------------+-----------+-----------+
```

- **Camera, mic and speaker tiles**: tap to turn each one on or off. Long-press a tile for its settings. They stay dimmed until your PC is connected.
- **Preview area**: shows the owl while everything is off.
  - **Mic only**: the sound wave fills the area and follows your voice. Tap the waveform to slide up the mic's settings from the bottom: voice isolation (keeps what the phone's speaker plays out of the mic), auto level (keeps your voice at the same loudness near or far), noise reduction (all the way left is off), and volume (`-12` to `+12 dB`, `0 dB` in the middle).
  - **Camera on**: the whole picture the PC gets, with a blurred copy of it only where it leaves room. It starts with the front camera, and your own view is mirrored like a mirror; the PC always gets the picture the right way round. The round buttons at the top right turn your own view off and on (the PC still gets the picture), mirror your view, and flip between the lenses. With the mic on too, the wave sits in a capsule at the bottom; tap it for the mic's settings. The buttons stay faint until you touch the picture.
  - Only when you need to do something does a short message show, such as the 4-digit code to check on your PC.
- **Link tile** (top right): shows the link in use (Wi-Fi, USB debugging, USB tethering or Bluetooth) once connected, a globe that gently dims in and out while Owlmic looks for your PC, or an alert when something needs you. Tap it to open your PCs:
  - The connected PC comes first with a tick, then the PCs you used before. Tap one to see its address, the link it used and when it was last connected, and to block it (blocked PCs move under the line, and Owlmic never connects to them until you unblock them).
  - The buttons at the bottom: Reconnect (drops the link and connects again, or looks again right away), then which links Owlmic may use: USB debugging, Wi-Fi, USB tethering and Bluetooth.
- **? tile**: How-to (the three setup steps), Privacy Policy, Rate, GitHub, Donate, Credits and the app version.
- **Turning the phone sideways**: the tiles stay where they are; what is on the preview fades out, the icons turn, and it comes back upright, drawers and settings included, so the phone can stand sideways during a call.

---

## 3. PC Tray Flyout & Controls

Click the **Owl icon** in your Windows taskbar system tray:
- **Bidirectional Controls (Hive Mind)**: Toggling the **Camera**, **Mic**, or **Speaker** button on your PC companion immediately toggles the hardware capture, streaming, and UI tiles on your Android phone, and vice versa. Neither device gets out of sync.
- **Microphone Waveform Dock**: Displays a live, dynamic audio waveform that responds in real time to your voice level. Clicking the waveform opens the quick audio controls (voice isolation, auto level, noise reduction slider, and volume).
- **Camera Live Preview**: Displays a smooth 30 FPS low-latency preview of your video feed with resolution, framerate, and aspect ratio controls.
- **Connection Drawer**: Tap the connection button in the bottom dock to review your PC's IP, active phone link, toggle supported transports (USB debugging, tethering, Wi-Fi, Bluetooth) online or offline, and refresh nearby devices. Tap any bottom feature button to close the drawer.
- **Speaker Tile**: Toggle PC audio redirection. When enabled, Owlmic captures Windows system audio and streams it to your phone.
  - *Quiet PC Speakers option*: When enabled, Owlmic automatically mutes your PC's desktop speakers while streaming audio to your phone.

---

## 4. Selecting Owlmic in Meeting & Streaming Apps

### Discord
1. Go to **User Settings** (gear icon) -> **Voice & Video**.
2. **Input Device**: Select **Owlmic Mic (Owlmic Bridge)**.
3. **Camera**: Select **Owlmic Cam**.

### Zoom / Microsoft Teams / Google Meet
1. Open Settings -> **Audio**.
2. **Microphone**: Select **Owlmic Mic**.
3. Open Settings -> **Video**.
4. **Camera**: Select **Owlmic Cam**.

### OBS Studio
- **Audio Input Capture**: Add a new source and select **Owlmic Mic**.
- **Video Capture Device**: Add a new source and select **Owlmic Cam**. Set resolution to Match Source (up to 1080p60).

---

## 5. Quick Notification Controls

When Owlmic is actively streaming, Android displays a low-overhead, persistent media notification in your notification tray:
- Shows the current active streams (e.g., `Mic & Camera on · Desktop-PC`).
- **Pause Mic**: Temporarily suspends mic audio capture.
- **Stop All**: Immediately halts all media streaming and sets hardware sensors to sleep.
