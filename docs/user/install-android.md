# Installing Owlmic on Android

The Owlmic Android app acts as the client hardware sensor. It captures pristine audio from your phone's microphone array and high-definition video from your rear or front cameras, transmitting both to your PC in real-time.

---

## 1. System Requirements

- **Android Version**: Android 8.0 (API Level 26 - Oreo) or newer. Android 10+ recommended for lowest audio capture latency.
- **Hardware**: Any ARM64 or ARMv7 smartphone or tablet with a microphone and camera.
- **Permissions Required**:
  - `RECORD_AUDIO`: Required to capture microphone input.
  - `CAMERA`: Required to stream video from phone cameras.
  - `POST_NOTIFICATIONS`: Required on Android 13+ for the foreground service status and quick controls.
  - `BLUETOOTH_CONNECT` / `BLUETOOTH_SCAN`: Required on Android 12+ for Bluetooth RFCOMM audio failover.
  - `NEARBY_WIFI_DEVICES`: Optional on Android 13+ for Wi-Fi discovery optimization.

---

## 2. Installation Methods

### Option A: Google Play Store (Recommended)
1. Open the **Google Play Store** on your Android device.
2. Search for **Owlmic** (published by Owlmic Open Source Project).
3. Tap **Install**. Updates will automatically install through the Play Store.

### Option B: Sideloading the Release APK
1. Visit the [GitHub Releases](https://github.com/diveshpatil9104/owlmic/releases) page on your Android phone.
2. Download the latest `Owlmic-v1.1.0.apk`.
3. Tap the downloaded file to install. If prompted by Android, enable **Install unknown apps** for your browser or file manager.
4. Follow the on-screen installer prompts.

---

## 3. First Launch & Permissions

1. Open **Owlmic** from your app drawer.
2. When prompted:
   - Tap **While using the app** to grant **Microphone** permission.
   - Tap **While using the app** to grant **Camera** permission.
   - Tap **Allow** for **Notifications** (this enables the persistent foreground control notification).
3. If using Bluetooth:
   - Tap **Allow** when prompted for **Nearby devices / Bluetooth** access.

---

## 4. Battery & Background Optimization (Important)

Owlmic uses an Android Foreground Service with an ongoing notification to ensure smooth, un-interrupted streaming even when you switch to other apps or turn the screen off.

However, aggressive OEM battery management (such as Samsung One UI, Xiaomi MIUI/HyperOS, Huawei EMUI, or OnePlus OxygenOS) may attempt to kill background networking.

To prevent disconnections:
1. Long-press the **Owlmic** app icon on your home screen -> tap the **(i)** Info icon.
2. Tap **Battery** or **App battery usage**.
3. Select **Unrestricted** (or "Don't optimize").
4. Ensure **Allow background activity** is enabled.
