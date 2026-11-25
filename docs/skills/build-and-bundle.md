# Build, Packaging & Distribution - Domain Skill

> **Scope**: Developer menu, release artifacts, packaging, driver caching, and centralized `releases/` distribution.

## 1. Developer Menu (`scripts/menu.py`) & Release Builder (`scripts/core/build.py`)

All development tasks, pre-flight checks, and platform release packaging are unified under `scripts/menu.py`, powered by modular scripts in `scripts/core/`:

### Developer Menu (`scripts/menu.py`)
- **Interactive Menu**: `python scripts/menu.py`
- **CLI Subcommands**:
  ```bash
  python scripts/menu.py build [release|all|pc|android|windows|linux]
  python scripts/menu.py check [--fix]
  python scripts/menu.py verify
  python scripts/menu.py clean
  python scripts/menu.py deps [--check|--fetch]
  python scripts/menu.py hooks
  ```

### Release Builder Backend (`scripts/core/build.py`)
Direct builder access with zero third-party Python dependencies:
- **Interactive Builder**: `python scripts/core/build.py`
- **Non-Interactive Target Packaging**:
  ```bash
  python scripts/core/build.py --target release
  python scripts/core/build.py --target android
  python scripts/core/build.py --target windows
  python scripts/core/build.py --target linux
  python scripts/core/build.py --target all
  python scripts/core/build.py --clean
  ```
- **Packaging Dependency Management**:
  ```bash
  python scripts/core/build.py --check-deps
  python scripts/core/build.py --fetch-deps
  python scripts/core/build.py --update-dep <vbcable|softcam> --url <NEW_URL> [--sha256 <HASH>] [--dep-version <VER>]
  ```

---

## 2. Centralized Release Output (`releases/`)

All build outputs are strictly routed to the root `releases/` folder, which is completely excluded from Git:

```text
releases/
├── Owlmic-Setup-{version}.exe
├── Owlmic-v{version}.apk
├── Owlmic-v{version}-linux-x86_64.tar.gz
└── others/
    ├── Owlmic-v{version}-windows-x64.exe
    ├── owlmic_vcam.dll
    ├── softcam.dll
    └── SHA256SUMS.txt
```

---

## 3. Platform Prerequisites & Dependency Tracking

All Windows packaging dependencies are tracked in `scripts/packaging/windows/dependencies.json` with primary URLs, mirror fallbacks, and immutable SHA-256 digests.

### Windows Packaging Independence
- **Local Folder Check First**: When building the installer (`Owlmic-Setup-*.exe`), the builder checks local directories first (`scripts/packaging/windows/driver/` and `scripts/packaging/cache/softcam.dll`). If present and verified, builds are completely offline and instantaneous.
- **Automated Remote Fallback**: If local driver or camera files are missing or corrupted, the builder fetches them automatically from verified remote URLs with mirror fallbacks, extracts them to local folders, and validates checksums.
- **Microphone Driver (`vbcable`)**: VB-CABLE driver package unpacked to `scripts/packaging/windows/driver/` and verified against SHA-256 (`B950E39F01AF1D04EA623C8F6D8EB9B6EA5C477C637295FABF20631C85116BFB`).
- **DirectShow Camera Fallback (`softcam`)**: DirectShow filter for Windows 10 cached at `scripts/packaging/cache/softcam.dll` and verified against SHA-256 (`9D635E0AF682A883C3C7D407513D47E59B0883771701B101216820DFCF997F0B`).
- **Native Windows 11 Virtual Camera (`owlmic_vcam`)**: Compiled directly from internal crate `desktop/crates/owlmic-vcam/` into `desktop/target/release/owlmic_vcam.dll`.
- **Inno Setup 6 (`ISCC.exe`)**: Discovered automatically across PATH, `LOCALAPPDATA`, `ProgramFiles`, and local cache; if missing on Windows, can be auto-installed non-interactively via `winget`.
- **Updating Dependencies**: When upstream releases a new driver pack or camera binary, developers update it with `python scripts/core/build.py --update-dep <name> --url <url>`, which downloads, validates, and syncs both local folders and `dependencies.json`.

### Android Packaging
- Requires Android SDK (API 35) and Java 21 / JBR.
- Invokes Gradle wrapper (`gradlew assembleRelease` or `gradlew assembleDebug`).
- **On-Demand Only**: APK builds are strictly executed only when explicitly requested by the user or when invoking packaging commands (`scripts/menu.py build android` / `python scripts/core/build.py --target android`). Never compile APKs during routine development or code verification.

### Linux Packaging
- On Linux: requires Cargo (`owlmic-app` target) and generates a standalone binary + portable `.tar.gz` containing `owlmic.service` and `setup-linux-devices.sh` from `scripts/packaging/linux/`.
- On Windows: cross-compiles `x86_64-unknown-linux-gnu` automatically via Zig (auto-installed via `winget` if missing), WSL, or `cross`.
