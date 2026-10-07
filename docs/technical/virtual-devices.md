# Virtual Devices & Windows Integration

This document covers how Owlmic creates and manages virtual hardware devices on Windows, including the virtual microphone bridge, Windows 11 Media Foundation camera, Windows 10 DirectShow filter, and the automated repair engine.

---

## 1. Owlmic Mic (Virtual Audio Endpoint)

To make microphone audio available to all Windows applications without requiring complex kernel driver signing, Owlmic employs a dedicated, low-latency audio bridge:

### Driver Architecture
- Owlmic utilizes a high-fidelity virtual audio device driver (VB-Audio virtual cable core) installed in `%LOCALAPPDATA%\Programs\Owlmic\driver`.
- During setup, `setup-audio-device.ps1` configures the device and injects custom Windows Property Store keys into the Windows Audio registry (`HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio`):
  - **Capture Endpoint Name**: `Owlmic Mic`
  - **Render Endpoint Name**: `Owlmic Bridge`
  - **Adapter Name**: `Owlmic Audio Device`
- The `AudioEndpointBuilder` service is refreshed so Windows applications display **Owlmic Mic** immediately.

### Playback & Feed Loop
- When a phone streams microphone audio to the PC, Owlmic opens a WASAPI exclusive or low-latency shared render client targeting the `Owlmic Bridge` render endpoint.
- Audio played into `Owlmic Bridge` is internally bridged by the driver into the `Owlmic Mic` capture endpoint, where recording applications read it with near-zero latency.

---

## 2. Owlmic Cam (Virtual Camera Architecture)

Owlmic provides dual virtual camera implementations to support both modern and legacy Windows architectures:

```
                      ┌────────────────────────────────────────┐
                      │               owlmic.exe               │
                      │  Decodes H.264 into NV12 / BGR24       │
                      │  Writes into Shared Memory Ring Buffer │
                      └───────────────────┬────────────────────┘
                                          │
                  ┌───────────────────────┴───────────────────────┐
                  ▼                                               ▼
     [ Windows 11 / Media Foundation ]               [ Windows 10 / DirectShow ]
             owlmic_vcam.dll                                 softcam.dll
     Registered via MFCreateVirtualCamera            Registered as DirectShow filter
     CLSID: {F39D55C8-6086-4447-9759-4D3B011684AA}  CLSID: {9F3E4861-5509-4C5A-A6D8-9A3E4E8A6822}
                  │                                               │
                  ▼                                               ▼
          Modern UWP / Web / Desktop Apps                 Legacy 32-bit & Win32 Video Apps
```

### Windows 11: Media Foundation Virtual Camera (`owlmic_vcam.dll`)
- Implemented in `pc/vcam` using Rust and native Windows Media Foundation APIs.
- Registers as a system virtual camera using Windows 11's `MFCreateVirtualCamera` API (`MF_VIRTUALCAMERA_TYPE_SOFTWARE_CAMERA_ADD`).
- Streams NV12 frames directly from the shared memory ring buffer into Windows Media Foundation pipelines.
- Delivers 0-based monotonic presentation timestamps (`inner.frame_count * inner.duration`), keyframe clean points (`MFSampleExtension_CleanPoint`), and system reference time (`MFSampleExtension_DeviceReferenceSystemTime`) on every sample for smooth, freeze-free streaming in Chromium WebRTC (Google Meet, Chrome, Edge) and desktop apps (Teams, Zoom).
- Supports instant frame resolution matching (720p, 1080p, 24/30/60 fps) and delivers pristine picture quality in modern browsers and desktop conferencing apps.

### Windows 10: DirectShow Filter (`softcam.dll`)
- A DirectShow capture source filter for backward compatibility on Windows 10 and older 32-bit capture software.
- Reads RGB24 / NV12 frames from the shared memory ring buffer and exposes the standard `IAMStreamConfig` DirectShow interfaces.

### Shared Memory Ring Buffer
- Implemented in `pc/vcam/src/shared.rs`.
- Backed by a named Win32 file mapping (`Local\OwlmicCamSharedMem`).
- Uses atomic sequence numbers and buffer index pointers to guarantee tear-free reads without locking reader and writer threads.
- If no frame has arrived for 500 ms, the reader falls back to a clean, calm placeholder frame (*"Owlmic Cam · Open Owlmic on your phone"*).

---

## 3. The One-Click Repair Engine

Windows driver updates, major OS upgrades, or aggressive registry cleaning can occasionally corrupt audio endpoints or camera COM registrations.

Owlmic embeds an automated repair engine in `pc/crates/owlmic-devices/src/repair.rs` and `pc/installer/setup-audio-device.ps1`:
- **Single UAC Elevation**: Clicking **Repair** executes one consolidated PowerShell script with Administrator credentials.
- **Actions Performed**:
  1. Detects missing audio driver files and re-installs them silently.
  2. Cleans up stale registry entries in `MMDevices\Audio\Capture` and `MMDevices\Audio\Render`.
  3. Re-applies the friendly names `Owlmic Mic` and `Owlmic Bridge`.
  4. Restarts `AudioEndpointBuilder` and `Audiosrv` services.
  5. Re-registers `owlmic_vcam.dll` and `softcam.dll` using `regsvr32.exe`.
  6. Adds inbound firewall rules for TCP port 7653 and UDP ports 7654/7655 to Windows Defender Firewall.
