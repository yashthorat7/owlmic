export interface DocItem {
  id: string
  title: string
  category: 'User Guides' | 'Technical Reference'
  content: string
}

export const docsData: DocItem[] = [
  {
    id: 'install-windows',
    title: 'Installing on Windows',
    category: 'User Guides',
    content: `# Installing Owlmic on Windows

Owlmic for PC runs as a lightweight, per-user background application in your Windows system tray. It installs virtual drivers that allow any application (Zoom, Microsoft Teams, Discord, OBS, Google Meet, Skype) to pick up your phone as an everyday microphone and webcam.

### System Requirements
- **Operating System**: Windows 10 (64-bit, version 1903+) or Windows 11 (all versions).
- **Architecture**: x86_64 (64-bit Intel / AMD).
- **RAM**: ~15 MB idle footprint.
- **Privileges**: Standard user account for running Owlmic. Administrator rights are required once during setup to install the virtual audio and camera drivers.

### Download & Installation
1. **Download the Setup Package**: Download \`Owlmic-Setup-1.0.0.exe\` from GitHub Releases.
2. **Run the Installer**: Follow the installer prompts and choose whether Owlmic should launch on startup.
3. **Driver Configuration**: Click **Yes** on the UAC prompt to allow \`setup-audio-device.ps1\` to configure the virtual audio endpoint and Windows Firewall rules.
4. **Finish**: Owlmic launches silently into your taskbar system tray.

### What Gets Installed
- \`owlmic.exe\`: The primary background application and tray coordinator.
- \`owlmic_vcam.dll\`: High-performance Media Foundation virtual camera driver for Windows 11.
- \`softcam.dll\`: DirectShow virtual camera driver filter for Windows 10 compatibility.
- \`setup-audio-device.ps1\`: Automated endpoint management and repair script.`,
  },
  {
    id: 'install-android',
    title: 'Installing on Android',
    category: 'User Guides',
    content: `# Installing Owlmic on Android

The Owlmic Android app acts as the client hardware sensor. It captures pristine audio from your phone's microphone array and high-definition video from your rear or front cameras, transmitting both to your PC in real-time.

### System Requirements
- **Android Version**: Android 8.0 (API Level 26 - Oreo) or newer. Android 10+ recommended.
- **Hardware**: Any ARM64 or ARMv7 smartphone or tablet with a microphone and camera.
- **Permissions**: Audio recording, Camera, Notifications, and Nearby devices (Bluetooth).

### Installation
- **Google Play Store**: Search for "Owlmic" and tap Install.
- **APK Sideload**: Download \`owlmic-1.0.0.apk\` from GitHub Releases and install directly.

### Battery Optimization
To prevent aggressive OEM battery management from terminating background streaming:
1. Long press Owlmic icon -> **App info**.
2. Tap **Battery** -> Select **Unrestricted**.`,
  },
  {
    id: 'first-connection',
    title: 'First Connection & Pairing',
    category: 'User Guides',
    content: `# First Connection & Pairing

Owlmic follows a strict **Trust-On-First-Use (TOFU)** security model. A new phone cannot stream microphone or camera data to your PC without explicit, two-factor numeric approval on your PC desktop.

### Pairing Walkthrough
1. **Open Owlmic on PC**: Verify the Owl icon is running in your taskbar tray.
2. **Open Owlmic on Phone**: The phone discovers your PC over Wi-Fi or USB cable within 1 second.
3. **Check the 4-Digit Code**: The phone displays "Approve on your PC · XXXX".
4. **Click Allow on PC**: Your PC displays a prompt showing the phone model and identical 4-digit code. Click **Allow**.
5. Both devices derive permanent pairing keys using NIST P-256 ECDH. Subsequent connections are completely instant and automatic!`,
  },
  {
    id: 'everyday-use',
    title: 'Everyday Use',
    category: 'User Guides',
    content: `# Everyday Use

Once paired, Owlmic operates transparently.

### Controls & Toggles
- **Mic Tile**: Tap to turn microphone capture ON or OFF. High-contrast white when ON. Live audio level meter bounces dynamically.
- **Camera Tile**: Tap to turn video streaming ON or OFF. Flip button switches between front and rear lenses.
- **Speaker Tile**: Tap to stream your PC's audio playback through your phone speaker or headphones.

### Selecting in Meeting Apps
- **Discord**: User Settings -> Voice & Video -> Input: **Owlmic Mic (Owlmic Bridge)**, Camera: **Owlmic Cam**.
- **Zoom / Teams / Meet**: Select **Owlmic Mic** as microphone and **Owlmic Cam** as camera.
- **OBS Studio**: Add Audio Input Capture source -> **Owlmic Mic**.`,
  },
  {
    id: 'links',
    title: 'Connection Levels & Switching',
    category: 'User Guides',
    content: `# Connection Levels & Switching

Owlmic features a zero-configuration, multi-tier transporter that automatically connects and switches between four connection levels:

1. **Level 1 — USB Debugging**: Lowest latency (< 10 ms), uncompressed 48 kHz PCM audio and 1080p60 video via \`adb reverse tcp:7653 tcp:7653\`.
2. **Level 2 — USB Tethering**: High-speed wired connection (< 15 ms) via standard USB cable without developer mode.
3. **Level 3 — Wi-Fi (LAN)**: Wireless connection (< 25 ms) with Opus audio and AES-256-GCM encryption.
4. **Level 4 — Bluetooth**: Audio-only RFCOMM failover when outside Wi-Fi range.

### Make-Before-Break Switching
Upgrading from Wi-Fi to USB is seamless: the new connection is proven in the background, a \`SWITCH\` opcode is sent, and media instantly shifts without dropping calls or stuttering.`,
  },
  {
    id: 'troubleshooting',
    title: 'Troubleshooting & Repair',
    category: 'User Guides',
    content: `# Troubleshooting & Repair

### One-Click Repair
If Owlmic Mic or Cam shows an error:
1. Click the Owl icon in your PC tray.
2. If an alert banner appears, click **Repair**.
3. Accept the Windows UAC elevation prompt.
4. The automated script re-registers camera filters, restores Windows audio endpoints, and verifies firewall rules.

### Common Fixes
- **Phone cannot find PC**: Ensure both devices are on the same Wi-Fi network and AP Isolation is disabled. Try entering your PC's IP address directly under Settings -> "Add PC by address".
- **Microphone silent**: Check that the mic tile on your phone is ON and the volume in Windows Sound Settings is set to 100%.`,
  },
  {
    id: 'privacy',
    title: 'Privacy & Security',
    category: 'User Guides',
    content: `# Privacy & Security Model

- **Zero Cloud & Zero Telemetry**: 100% peer-to-peer. No servers, no accounts, no analytics, no trackers.
- **Default-OFF Policy**: Microphone and camera always start in the OFF state.
- **AES-256-GCM Encryption**: All wireless control and media datagrams are sealed with authenticated encryption.
- **Local Storage Only**: Pairing tokens are kept exclusively in local configuration on your phone and PC.`,
  },
  {
    id: 'uninstall',
    title: 'Uninstalling',
    category: 'User Guides',
    content: `# Uninstalling Owlmic

### Windows
1. Open Windows Settings -> Apps -> Installed apps.
2. Find Owlmic and click **Uninstall**.
3. The uninstaller removes application binaries, deregisters virtual camera filters, cleans up audio endpoints, and restores Windows default devices.

### Android
Long press Owlmic -> tap **Uninstall**.`,
  },
  {
    id: 'architecture',
    title: 'Technical Architecture',
    category: 'Technical Reference',
    content: `# Technical Architecture

### Core Invariants
- **Client/Server Division**: Phone is always the client; PC is always the server.
- **Hub & Mailbox Concurrency**: Decoupled supervised thread hubs communicating via bounded channels.
- **No Async Runtime on PC**: Strictly standard threads (\`std::thread\`) and crossbeam channels. Zero Tokio outside Linux Bluetooth.
- **Real-time Safety**: Audio loops never block on network I/O. Video uses queue depth = 1 (\`KEEP_ONLY_LATEST\`).

### The Hubs
- **LinkHub**: Discovery, carrier negotiation, health monitoring, make-before-break switching.
- **SessionHub**: TOFU trust, P-256 ECDH key exchange, session hold state machine.
- **MediaHub**: Audio jitter buffer, cubic resampler (±0.2%), H.264 slice reassembly, NV12 conversion, RNNoise DSP.
- **DeviceHub**: Feeds virtual audio cable and Media Foundation virtual camera.
- **SettingsHub**: Versioned settings store with bidirectional synchronization.`,
  },
  {
    id: 'transporter',
    title: 'The Transporter',
    category: 'Technical Reference',
    content: `# The Transporter & Networking

### Ports
- **TCP 7653**: Control stream & USB debugging media.
- **UDP 7654**: Discovery beacon (\`OWLMIC?3\` / \`OWLMIC!3\`).
- **UDP 7655**: High-speed media datagrams on IP links.
- **Bluetooth RFCOMM**: Multiplexed control and audio serial stream.

### Health & Supervision
- 1,000 ms ping/pong heartbeat interval.
- 3,000 ms socket timeout threshold.
- 30-second session hold retains virtual devices on PC during temporary carrier drops.`,
  },
  {
    id: 'protocol',
    title: 'Wire Protocol v3',
    category: 'Technical Reference',
    content: `# Wire Protocol Version 3

All integers are big-endian. Text strings are UTF-8.

### Control Frames
\`type (1 B) | length (3 B) | payload (JSON)\`

- \`0x01\` HELLO
- \`0x02\` HELLO_ACK
- \`0x03\` PROOF
- \`0x04\` PENDING
- \`0x05\` WELCOME
- \`0x06\` REJECT
- \`0x10\` PING / \`0x11\` PONG
- \`0x12\` REPORT
- \`0x20\` STATE
- \`0x21\` SETTINGS
- \`0x22\` STREAM_START / \`0x23\` STREAM_STOP
- \`0x24\` KEYFRAME_REQUEST
- \`0x30\` SWITCH
- \`0x3F\` BYE

### Media Datagrams
\`stream (1) | flags (1) | seq (4) | timestamp (4) | payload | tag (16, wireless)\`
- Stream 0: Carrier Hello
- Stream 1: Microphone (Phone -> PC)
- Stream 2: Camera (Phone -> PC)
- Stream 3: Speaker (PC -> Phone)`,
  },
  {
    id: 'media',
    title: 'Media Pipelines',
    category: 'Technical Reference',
    content: `# Media Pipelines

### Audio Pipeline
- Android Oboe / AAudio native capture at 48,000 Hz 16-bit PCM mono.
- 10 ms packet interval (20 ms over Bluetooth).
- Adaptive jitter buffer (10-30 ms target depth).
- Cubic drift resampler steers playback pitch within ±0.2% to lock crystal clocks without audible artifacts.
- Integrated RNNoise neural network speech enhancement.

### Video Pipeline
- CameraX capture with OpenGL ES 2.0 / 3.0 shader processing.
- Hardware MediaCodec H.264 Annex B encoder.
- MTU-safe fragmentation (<= 1,200 bytes per packet).
- Decoded to NV12 and written into an atomic shared memory ring buffer.`,
  },
  {
    id: 'virtual-devices',
    title: 'Virtual Devices & Drivers',
    category: 'Technical Reference',
    content: `# Virtual Devices & Drivers

### Owlmic Mic
- WASAPI stream bridge feeding VB-Audio virtual cable core.
- Configured with custom friendly names: "Owlmic Mic" (Capture) and "Owlmic Bridge" (Render).

### Owlmic Cam
- **Windows 11**: Native Media Foundation Software Virtual Camera (\`owlmic_vcam.dll\`) via \`MFCreateVirtualCamera\`.
- **Windows 10**: DirectShow capture source filter (\`softcam.dll\`).
- Shared memory ring buffer ensures lock-free, zero-copy frame handoff.`,
  },
  {
    id: 'security',
    title: 'Security & Cryptography',
    category: 'Technical Reference',
    content: `# Security & Cryptography

- **Curves**: NIST P-256 (secp256r1) for identity and ephemeral key exchange.
- **KDF**: HKDF-SHA256 (RFC 5869).
- **Encryption**: AES-256-GCM with 96-bit nonces and 128-bit authentication tags.
- **Replay Protection**: 64-packet bitmask replay window.
- **Rate Limits**: Max 20 probe answers/second, max 4 handshakes/second.`,
  },
  {
    id: 'design-language',
    title: 'Design Language & UI',
    category: 'Technical Reference',
    content: `# Design Language & UI System

### Principles
- Pure functional design: zero gradients, zero shadows, zero glassmorphism.
- OLED pure black palette (\`#000000\`), tile surface (\`#0A0A0A\`), hairlines (\`#262626\`).
- High-contrast state accents: Green (\`#30D158\`), Yellow (\`#FFD60A\`), Red (\`#FF453A\`).

### PC Tray Flyout
- 560 x 255 px Win32 panel rendered in double-buffered GDI/GDI+.
- Cold start in < 15 ms, memory footprint < 15 MB.`,
  },
  {
    id: 'building',
    title: 'Building from Source',
    category: 'Technical Reference',
    content: `# Building from Source

### Desktop App
\`\`\`powershell
cd desktop
cargo test --workspace
cargo build --release
\`\`\`

### Android Client
\`\`\`bash
cd android
./gradlew testDebugUnitTest lintDebug
./gradlew assembleDebug
\`\`\`

### Website
\`\`\`bash
cd website
npm install
npm run build
\`\`\``,
  },
  {
    id: 'releasing',
    title: 'Release Process',
    category: 'Technical Reference',
    content: `# Release Process

1. Verify \`cargo test --workspace\` and \`./gradlew testDebugUnitTest lintDebug\`.
2. Validate wire vectors against \`protocol/vectors/*.json\`.
3. Check product name spelling invariant in CI.
4. Tag \`v1.0.0\` to trigger automated Inno Setup, APK, and AAB release builds on GitHub Actions.`,
  },
]
