# Android Client (Kotlin) - Domain Skill

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
    - Software Opus audio encoder/decoder vendored at `shared/vendor/opus`.

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
4. **Dynamic Asset & Icon Sourcing (Zero Hardcoded Glyphs)**:
   - All vector icons and glyphs are dynamically parsed and generated from `shared/assets/icons/*.svg` by the Gradle task `:core:generateDesign`.
   - Compose views strictly reference `com.owlmic.core.design.GeneratedGlyphs` via `Glyphs.kt`. Never embed SVG path strings directly in Kotlin code.
   - App launcher icon uses the official centered Owlmic owl SVG on a pure solid black (`#000000`) background (`ic_launcher.xml` / `ic_launcher_round.xml`).
5. **Prototype-Accurate Bento UI Architecture**:
   - The UI matches `shared/design/prototypes/` pixel-by-pixel with clean geometry, single source of truth tokens, and zero extraneous dialogs or legacy buttons.
   - **Header**: Wordmark, Help tile, and Connection tile with pulse breathing indicator.
   - **Center Playground**:
     - Dot grid and watermark owl when idle.
     - Aspect ratio crop box with optical crop border lines (pillarbox / letterbox).
     - Compact expandable format HUD (50dp closed, 180dp open) for Resolution (FHD/HD/480p), FPS (60/30/24), and Aspect Ratio (Auto/16:9/4:3/1:1/3:4/9:16) with 4s idle auto-dim.
     - Centered waveform (288dp width, 84dp bars) when camera off; docked waveform capsule (236dp width, 24dp bars) when camera on, expandable to `calc(100% - 24dp)` with hardware faders (`.owl-switch` and `.fader`).
   - **Drawers**:
     - Connection Drawer: Inline handshake sweep (`sync-beam` animated gradient line on the connection row), inline Add Host row (`+`), and 6-column transport footer (refresh + 4 link toggles).
     - Help Drawer: 2x3 bento grid (How-to, Privacy, Rate, GitHub, Donate, Credits) with subpages.
6. **Strict Library Blacklist**:
   - Never add: Retrofit, Hilt/Dagger, Room, Firebase, or external analytics/telemetry SDKs.
   - Keep the dependency footprint minimal and lightweight.
7. **On-Demand APK Generation (Never Build APKs During Development)**:
   - **Never run `assembleDebug`, `assembleRelease`, or package APKs (`.apk` / `.aab`) during normal development, refactoring, or verification.**
   - **Do NOT build APKs unless explicitly requested by the user** (or when running deliberate release packaging via `python scripts/menu.py build android` / `python scripts/core/build.py --target android`).
   - Routine development verification is strictly unit tests and lint:
     `./gradlew testDebugUnitTest lintDebug`
   - Building APKs produces heavy binaries, consumes unnecessary CPU/time/tokens, and violates binary staging guardrails.

---

## 3. Verification Commands

Before proposing changes to `android/` (unit tests and lint only — do NOT build APKs):
```bash
cd android
./gradlew testDebugUnitTest lintDebug
```
(On Windows: `.\gradlew.bat testDebugUnitTest lintDebug`)

> **Note**: Never run `assembleDebug`, `assembleRelease`, or full `build` tasks during pre-flight checks. Only compile APKs when the user explicitly requests an APK artifact.
