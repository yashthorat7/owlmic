# Uninstalling Owlmic

This document details how to completely remove Owlmic, its virtual drivers, registry keys, and settings from Windows and Android.

---

## 1. Windows Uninstall

Owlmic provides an automated uninstaller that cleanly deregisters all virtual camera filters, restores Windows default audio devices, and purges the driver endpoints.

### Automatic Removal (Standard)
1. Open Windows **Settings** (`Win + I`) -> **Apps** -> **Installed apps** (or **Apps & features**).
2. Search for **Owlmic**.
3. Click the three dots `...` and select **Uninstall**.
4. When prompted by Windows UAC, click **Yes** to allow `setup-audio-device.ps1 -Uninstall` to remove the virtual audio cable driver and restore your previous default audio endpoints.
5. Follow the uninstaller prompts. The uninstaller removes:
   - Application binaries (`owlmic.exe`, `owlmic_vcam.dll`, `softcam.dll`).
   - Virtual camera COM registrations.
   - Autostart registry keys under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
   - Windows Defender Firewall rules for port 7653, 7654, and 7655.

### Optional: Removing Settings & Pairing History
To remove your local device pairings and settings:
1. Press `Win + R`, type `%APPDATA%`, and press Enter.
2. Delete the `Owlmic` directory (`%APPDATA%\Owlmic`).

---

## 2. Android Uninstall

1. Long-press the **Owlmic** app icon on your home screen or app drawer.
2. Tap **App info** (or drag the icon to **Uninstall** at the top of the screen).
3. Tap **Uninstall** and confirm with **OK**.
4. Android will remove the application package, foreground service components, and all local cryptographic pairing keys.
