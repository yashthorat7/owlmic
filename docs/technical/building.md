# Building Owlmic

This document provides step-by-step instructions for building the Android client and the desktop application (Windows and Linux) from source.

---

## 1. Prerequisites

### Windows Desktop App
- **Rust**: Version 1.80+ (stable toolchain: `x86_64-pc-windows-msvc` or `x86_64-pc-windows-gnu`).
- **Inno Setup**: Version 6.2+ (for building the installer executable `ISCC.exe`).
- **Git**: With submodule support.

### Linux Desktop App
- **Rust**: Version 1.80+ (stable toolchain: `x86_64-unknown-linux-gnu`).
- **System Libraries**: `pkg-config`, `libasound2-dev`, `libdbus-1-dev`, `libpipewire-0.3-dev`, and `v4l2loopback-dkms`.
- **Systemd**: For user service activation (`owlmic.service`).

### Android Client
- **JDK**: Java Development Kit 17 (Temurin or OpenJDK recommended).
- **Android SDK**: API 34+ with Android NDK version 26+ installed.
- **CMake**: Version 3.22+ (for compiling native Oboe and Opus JNI libraries).

---

## 2. Building the Desktop App (Rust)

1. **Clone the Repository with Submodules**:
   ```bash
   git clone --recurse-submodules https://github.com/diveshpatil9104/owlmic.git
   cd owlmic/desktop
   ```

2. **Run Tests**:
   ```bash
   cargo test --workspace
   ```

3. **Check Formatting & Linting**:
   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets -- -D warnings
   ```

4. **Build Release Binaries**:
   ```bash
   cargo build --release
   ```
   This generates:
   - Windows: `target/release/owlmic.exe` (primary background tray app) and `target/release/owlmic_vcam.dll` (Windows 11 virtual camera driver).
   - Linux: `target/release/owlmic` (native daemon with PipeWire, v4l2loopback, and KSNI status notifier tray).

5. **Package for Distribution**:
   - **Windows Installer**:
     ```bash
     python menu.py build windows
     # Or directly: python scripts/build.py --target windows
     ```
     Generates `dist/windows/Owlmic-Setup-0.1.0.exe`. Packaging dependencies (`VB-CABLE` audio driver and `softcam.dll` virtual camera fallback) are tracked in `scripts/packaging/windows/dependencies.json`, verified via SHA-256, and cached locally for offline and reproducible builds. Use `python menu.py deps --check` to audit dependencies.
   - **Linux Portable Archive**:
     ```bash
     python menu.py build linux
     # Or directly: python scripts/build.py --target linux
     ```
     Generates `dist/linux/Owlmic-v0.1.0-linux-x86_64.tar.gz` containing the `owlmic` binary, systemd user service unit (`owlmic.service`), and setup script (`setup-linux-devices.sh`).
   - **All Platforms Release Bundle**:
     ```bash
     python menu.py build release
     ```

---

## 3. Building the Android App (Kotlin & NDK)

1. **Navigate to the Android Directory**:
   ```bash
   cd owlmic/android
   ```

2. **Run Unit Tests & Linting**:
   ```bash
   ./gradlew testDebugUnitTest lintDebug
   ```

3. **Build Debug APK**:
   ```bash
   ./gradlew assembleDebug
   ```
   The APK will be generated at `app/build/outputs/apk/debug/app-debug.apk`.

4. **Build Release APK & App Bundle**:
   ```bash
   ./gradlew assembleRelease bundleRelease
   ```

---

## 4. Building the Website (`website/`)

1. **Navigate to Website Directory**:
   ```bash
   cd owlmic/website
   ```

2. **Install Dependencies & Build**:
   ```bash
   npm install
   npm run build
   ```
   The static site assets are output to `website/dist/`.
