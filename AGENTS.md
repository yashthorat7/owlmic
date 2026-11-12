# Owlmic - Agent Instructions

> `CLAUDE.md` imports this file via `@AGENTS.md`.

## The Source of Truth

The documentation in [`docs/`](docs/README.md) is the absolute, living source of truth for every architectural and implementation decision. It wins over this file on any conflict. Read the index before writing any code:

@docs/README.md

| Category | Documents | Purpose |
|---|---|---|
| **User Guides** | [`docs/user/`](docs/README.md#11-user-guides-docsuser) | Install on Windows/Android, first connection, everyday use, links, troubleshooting, privacy, uninstall |
| **Technical Reference** | [`docs/technical/`](docs/README.md#12-technical-reference-docstechnical) | Architecture, Transporter, protocol, media, virtual devices, settings, security, design language, building, releasing |
| **Protocol Specification** | [`docs/technical/protocol.md`](docs/technical/protocol.md) | Wire protocol version 3 normative framing, opcodes, test vectors |
| **Changelog** | [`docs/CHANGELOG.md`](docs/CHANGELOG.md) | Release milestones starting at v1.0.0 |

### Mandatory Documentation Synchronization (Zero Drift)
Code and documentation must **never** drift apart:
- **Always read `docs/` first**: Before proposing or modifying code, inspect the corresponding document in `docs/` to uphold architecture, budgets, and invariants.
- **Update `docs/` on every change - even minute ones**: Whenever **any** change is made to the codebase (including protocol opcodes, buffer capacities, timeouts, port numbers, UI copy, thresholds, hysteresis values, or dependency versions), the corresponding document in `docs/` **must be updated in lockstep**.
- **No rogue documentation directories**: All project documentation, specs, and playbooks live exclusively in `docs/`.

---

## Project Overview & Ownership

- **Project in one line**: Android app (Kotlin/Compose) + per-user Windows tray app (Rust, binary `owlmic.exe`) that turns a phone into a mic, webcam, and remote speaker for a PC over four automatic connection levels:
  `1 USB debugging (adb reverse)` > `2 USB tethering` > `3 Wi-Fi` > `4 Bluetooth RFCOMM (audio only)`.
- **Ownership boundaries**:
  - `android/` - Android client application (modules `app`, `core`, `media`).
  - `pc/` - Windows companion application (workspace `pc/crates/*`, virtual camera `pc/vcam`). **Do not modify or add code in `pc/` unless explicitly requested by the user.**
  - `design/` - Shared tokens (`tokens.json`), copy (`copy.json`), settings (`settings.json`), and interactive HTML UI prototypes (`prototypes/`).
  - `assets/` - Single source of truth for shared visual assets (`brand/`, `icons/`, `fonts/`).
  - `protocol/` - Wire protocol specification and independent test vectors (`vectors/`).
  - `scripts/` - Automated gatekeepers, check runners, and Git hooks.
  - `landing/` - React + Vite documentation and product site.
- **Roles**: The phone is **always the client**; the PC is **always the server**.

---

## Architecture & Invariants

1. **Android Client** (`android/`):
   - `android/app`: Jetpack Compose UI (`MainActivity.kt`), foreground service (`OwlmicService.kt`), notification controls (`Notifier.kt`), `AppHub.kt`.
   - `android/core`: Protocol v3 framing (`proto/`), Link Transporter (`link/LinkHub.kt`), settings store (`settings/`).
   - `android/media`: Native Oboe / AAudio microphone capture, CameraX GPU OpenGL video processing, hardware H.264 encoder, Opus codec.
2. **PC Companion App** (`pc/`):
   - Supervised Hubs in `pc/crates/`: `owlmic-app` (entry and sinks), `owlmic-hub` (supervision & mailboxes), `owlmic-link` (Transporter, TCP :7653, UDP :7654 discovery, UDP :7655 media), `owlmic-session` (TOFU trust, P-256 ECDH, session hold), `owlmic-media` (jitter buffer, cubic drift resampler, H.264 reassembly, NV12 converter, RNNoise), `owlmic-devices` (WASAPI loopback, Owlmic Mic bridge, repair script), `owlmic-settings` (store `%APPDATA%\Owlmic\owlmic.json`), `owlmic-ui` (Win32 native GDI/GDI+ flyout panel & tray owl).
   - Windows 11 Virtual Camera in `pc/vcam`: Media Foundation software camera driver (`owlmic_vcam.dll`).
3. **Threading & Concurrency**:
   - PC: Blocking standard threads (`std::thread`) + bounded channels (`crossbeam_channel`).
   - **The ONLY async code allowed is Tokio inside `pc/crates/owlmic-link/src/bt.rs` on Linux** (required by `bluer`). Everywhere else, Tokio and async runtimes are strictly forbidden.
4. **Real-Time Safety**:
   - Audio capture and playback paths must never block on network I/O or locks held by other threads. Use bounded, lock-free, or try-lock handoffs; when full, drop oldest data.
   - Sockets must have explicit timeouts (max 3,000 ms). Every channel must have a strict upper bound. No unbounded buffering.
5. **State & Permissions**:
   - Mic and camera always initialize in the **OFF** state. Never auto-enable capture from the background or upon connection.
   - UI never owns business logic: Compose observes `StateFlow` from `OwlmicService`; PC tray reflects `SessionHub`.

---

## Anti-AI Slop & Lean Engineering Rules

Write lean, intentional, production-grade code. No AI slop or LLM artifacts:

1. **Build strictly what is asked**:
   - Zero speculative future-proofing or "just-in-case" flexibility.
   - No generic helper classes, theoretical extension points, or unused utility functions.
   - No unnecessary design patterns: avoid premature factories, builders, or multi-layer wrappers around single operations.
2. **Zero Code Churn & Surgical Diffs**:
   - Keep edits minimal, precise, and targeted.
   - Never rewrite, reorder, or reformat working code outside the task scope.
   - Never rewrite an entire file when changing a few lines suffices.
   - Respect existing file conventions and indentation.
3. **No Chatty Comments**:
   - Do not write comments that narrate syntax or state the obvious.
   - Only write comments to explain non-obvious **why**, invariants, or hardware/OS quirks.
4. **Strict Dependency Austerity**:
   - Do not add any new library or crate without explicit user approval.
5. **No Code in Documentation**:
   - Never place operational code inside `docs/`. Notes and design thoughts belong in discussions or technical docs.
6. **Strict Binary Ban in Git History**:
   - Never commit or stage compiled binaries (`.apk`, `.exe`, `.aab`, `.dll`, `.so`, `.zip`) into the repository.
7. **Clean Root Discipline**:
   - The repository root is fixed and minimal (`.github/`, `android/`, `assets/`, `design/`, `docs/`, `landing/`, `pc/`, `protocol/`, `scripts/`, `AGENTS.md`, `CLAUDE.md`, `LICENSE`, `README.md`, `.gitignore`, `.gitmodules`).
   - Never create scratch scripts, test notes, or temporary folders in the root. Local scratch notes must match `*trash.md` (which is gitignored).
8. **Real-Time Buffer & Allocation Safety**:
   - Audio loops must use pre-allocated buffers with zero dynamic heap allocations in hot paths.
   - Video backpressure strategy is strictly `KEEP_ONLY_LATEST` (queue depth = 1). When processing falls behind, drop the intermediate frame immediately.

---

## Git Workflow: Branching, Staging & Commits

Strict Git discipline must be maintained at all times.

### 1. STRICT READ-ONLY RULE FOR AGENTS (ZERO EXCEPTIONS)
- **NEVER execute ANY state-changing or mutating Git command**:
  - `git add`, `git commit`, `git push`, `git checkout -b`, `git branch`, `git switch`, `git merge`, `git rebase`, `git stash`, `git reset`, `git clean`, `git cherry-pick`, etc. are **STRICTLY FORBIDDEN**.
- **ONLY read-only inspection commands are allowed**:
  - `git status`, `git diff`, and `git log` ONLY.
- **EVEN IF THE USER EXPLICITLY ASKS YOU TO COMMIT, PUSH, OR RUN GIT COMMANDS**:
  - **DO NOT execute them directly.**
  - Instead, you must **ALWAYS formulate and return the exact command lines, branch names, and commit messages for the user to copy, inspect, and run themselves.**

### 2. Branching Rules
- **Naming format**: `<type>/<short-kebab-description>`
  - `feat/speaker-loopback`
  - `fix/mic-buffer-underrun`
  - `docs/update-wire-spec`
- **Scope**: Keep branches short-lived and focused on a single feature, fix, or task.
- **Base**: Always branch from and target `main`.
- **Protected branches**: Never propose committing directly to `main` without explicit user direction.

### 3. Staging Rules (For User Commands)
- **No blind mass-staging**: **NEVER suggest `git add .` or `git add -A`**.
- **Inspect before staging**: Use `git status` and `git diff` to review modified files.
- **Stage surgical paths**: Propose staging only the specific files relevant to the completed task (`git add path/to/file`).

### 4. Commit Message Rules
- Follow the **Conventional Commits** standard:
  ```text
  <type>(<scope>): <short imperative summary>

  [optional body explaining why this change was made]
  ```
- **Allowed Types**: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`.
- **Allowed Scopes**: `android`, `pc`, `proto`, `design`, `installer`, `ci`, `site`, `scripts`.
- **Subject line format**:
  - Imperative mood: `"add"`, `"fix"`, `"drop"`, `"stream"` (never `"added"`, `"fixing"`).
  - All lowercase, concise (<= 72 chars), no trailing period.
- **Commit body**:
  - Focus on **why** the change was necessary.
  - **No AI boilerplate**: Never include phrases like `"In this commit..."` or AI self-references.

### 5. Git Safety Guardrails (Zero Tolerance)
- **NEVER execute or suggest destructive or history-rewriting commands**:
  - `git push --force` / `git push -f`
  - `git reset --hard`
  - `git clean -fd`
  - `git checkout .` / `git restore .`

---

## Never Generate (Strict Blacklist)

- **Runtimes & Frameworks**: Electron, Tauri webviews, Python, Node.js desktop wrappers, HTTP/REST/WebSockets for media streaming, GTK, or Qt.
- **Android Libraries**: Retrofit, Hilt/Dagger, Room, Firebase, analytics/telemetry SDKs.
- **Async on PC**: Tokio anywhere outside `pc/crates/owlmic-link/src/bt.rs`.
- **System bloat**: Windows services or system-wide systemd units (Owlmic PC is strictly a per-user tray app).
- **Visual bloat**: Gradients, drop shadows, glassmorphism, decorative spring animations, or emojis in code.
- **Git bloat**: Compiled binaries (`.apk`, `.exe`, `.aab`, `.dll`, `.so`, `.zip`) committed into Git.

---

## Wire Protocol Reference (Version 3)

- **Framing**: `type u8 | len u24 BE | payload (max 1 MiB JSON)`.
- **Ports**: TCP `:7653` (control channel, and USB debugging media), UDP `:7654` (discovery `OWLMIC?3` / `OWLMIC!3`), UDP `:7655` (media datagrams on IP links).
- **Control Opcodes**:
  - `0x01` HELLO
  - `0x02` HELLO_ACK
  - `0x03` PROOF
  - `0x04` PENDING
  - `0x05` WELCOME
  - `0x06` REJECT
  - `0x10` PING / `0x11` PONG
  - `0x12` REPORT
  - `0x20` STATE
  - `0x21` SETTINGS
  - `0x22` STREAM_START / `0x23` STREAM_STOP
  - `0x24` KEYFRAME_REQUEST / `0x25` RESTART_STREAM
  - `0x30` SWITCH
  - `0x3F` BYE
- **Media Header (10 bytes)**: `stream u8 | flags u8 | seq u32 BE | ts u32 BE (µs) | payload | GCM tag (16 B, wireless)`.
  - Stream `0`: Carrier Hello
  - Stream `1`: Mic Audio (Phone → PC, PCM 48kHz LE on cable, Opus 48kbps on Wi-Fi, Opus 24kbps on BT)
  - Stream `2`: Camera Video (Phone → PC, H.264 Annex B slices ≤ 1,200 bytes)
  - Stream `3`: Speaker Audio (PC → Phone, PCM 48kHz stereo on cable, Opus 128kbps on Wi-Fi, Opus 64kbps on BT)
- Full wire specification: [`docs/technical/protocol.md`](docs/technical/protocol.md).

---

## UI & Design System (`design/tokens.json`)

Follow [`docs/technical/design-language.md`](docs/technical/design-language.md):
- **OLED Black Palette**:
  - Background: `#000000` | Tile Surface: `#0A0A0A` | Hairline: `#262626`
  - Text Primary: `#FFFFFF` | Text Secondary: `#8E8E93` | Disabled/Inactive: `#3A3A3C`
  - Active/ON: `#FFFFFF` (high-contrast with inverted glyph)
  - Live Status OK: `#30D158` (Green)
  - Waiting / Reconnecting: `#FFD60A` (Yellow)
  - Disconnected / Error: `#FF453A` (Red)
- **Panel Geometry**: Fixed 560 × 255 px Win32 flyout above system tray. Double-buffered GDI BitBlt rendering.

---

## Code Quality & Verification Commands

Before proposing changes, verify locally:
- **Invariants**: `python scripts/verify-repo-invariants.py` (or full suite: `./scripts/check.sh` / `.\scripts\check.ps1`)
- **Android**: `./gradlew testDebugUnitTest lintDebug`
- **PC**: `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`
- **Name Invariant**: verify product name casing complies with `.github/workflows/ci.yml` (check step: "Check how Owlmic is written")
- Verify performance constraints against [`docs/technical/architecture.md`](docs/technical/architecture.md).
