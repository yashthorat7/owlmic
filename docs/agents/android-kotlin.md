# Android Client (Kotlin) - Agent Rules

> **Scope**: Code living in `android/` (`app`, `core`, `media`).

## 1. Architectural Roles & Module Boundaries

- The Android phone is **always the client**; the PC is **always the server**.
- **Module Hierarchy**:
  - `android/app`:
    - Jetpack Compose UI (`MainActivity.kt`, `MainScreen.kt`, `Theme.kt`).
    - Foreground service (`OwlmicService.kt`) with persistent notification (`Notifier.kt`).
    - UI coordinator (`AppHub.kt`) and screen state model (`ScreenModel.kt`).
  - `android/core`:
    - Protocol v3 framing and message serialization (`proto/`).
    - The Transporter link state machine (`link/LinkHub.kt`, `Carriers.kt`, `Ranker.kt`, `Watchdog.kt`).
    - Settings store and cryptographic key wrapper (`settings/Store.kt`).
  - `android/media`:
    - Native Oboe / AAudio C++ real-time microphone capture.
    - CameraX GPU OpenGL surface processing and hardware H.264 encoder.
    - Software Opus audio encoder/decoder (`libopus`).

---

## 2. Invariants & Lifecycle Rules

1. **State & Permissions**:
   - Microphone and camera capture **must always initialize in the OFF state**. Never auto-enable capture on connection or from the background without explicit user action.
   - Foreground service (`OwlmicService`) manages the active session and link lifecycle. Capture components run strictly within the foreground service context with an active notification.
2. **UI & Business Logic Separation**:
   - The UI never owns business logic or socket state.
   - Jetpack Compose views observe reactive `StateFlow` streams exposed by `OwlmicService` via `AppHub`.
3. **Real-Time Audio & Video Constraints**:
   - Audio loops in C++ (Oboe) must use pre-allocated buffers with zero heap allocations on the audio callback thread.
   - Video backpressure is strictly `KEEP_ONLY_LATEST` (queue depth = 1). Drop intermediate camera frames immediately if encoding or network transmission falls behind.
4. **Strict Library Blacklist**:
   - Never add: Retrofit, Hilt/Dagger, Room, Firebase, or external analytics/telemetry SDKs.
   - Keep the dependency footprint minimal and lightweight.

---

## 3. Verification Commands

Before proposing changes to `android/`:
```bash
cd android
./gradlew testDebugUnitTest lintDebug
```
(On Windows: `.\gradlew.bat testDebugUnitTest lintDebug`)
