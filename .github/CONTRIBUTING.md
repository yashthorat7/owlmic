# Contributing to Owlmic

Thanks for contributing to Owlmic! Small, focused pull requests and precise bug reports keep the codebase lean and fast.

By participating, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## 1. 10-Minute Setup by Area

### 1.1 Desktop App (`desktop/`)
**Prerequisites**: Windows 10 (1903+) / Windows 11 or Linux, [Rust 1.80+](https://rustup.rs/) (stable), C++ Build Tools or GCC.

```powershell
# 1. Clone repo with submodules
git clone --recurse-submodules https://github.com/diveshpatil9104/owlmic.git
cd owlmic\desktop

# 2. Check compilation and run tests
cargo test --workspace

# 3. Verify format and clippy lints
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```
> [!NOTE]
> Tests that interact with system audio or device registry keys are marked `#[ignore]` and run only when explicitly targeted (`cargo test -- --ignored`).

### 1.2 Android App (`android/`)
**Prerequisites**: JDK 17 (Eclipse Temurin or OpenJDK), Android SDK (API 34+), Android NDK (r26+), CMake 3.22+.

```bash
# 1. Navigate to android directory
cd owlmic/android

# 2. Run unit tests and linting
./gradlew testDebugUnitTest lintDebug

# 3. Build debug APK
./gradlew assembleDebug
```
Output APK is located at `android/app/build/outputs/apk/debug/app-debug.apk`.

### 1.3 Website (`website/`)
**Prerequisites**: Node.js 20+ and npm.

```bash
# 1. Navigate to website directory
cd owlmic/website

# 2. Install dependencies & run dev server
npm install
npm run dev

# 3. Build & lint
npm run build
npm run lint
```

---

## 2. Shared Sources (`shared/`)

To prevent divergence between Windows, Linux, and Android:
- **`shared/design/tokens.json`**: Defines all colors, spacing, corner radii, typography, and geometry.
- **`shared/design/copy.json`**: Contains every user-facing string, message, and name.
- **`shared/design/settings.json`**: Schema for all shared and local settings.
- **`shared/protocol/`**: Wire specification and test vectors (`shared/protocol/vectors/*.json`).

Never hardcode colors, strings, or settings in platform code. Edit the shared JSON files, which are compiled into both apps.

---

## 3. How to Pick an Issue

1. Check the [Issues](https://github.com/diveshpatil9104/owlmic/issues) tab.
2. Filter by [`good first issue`](https://github.com/diveshpatil9104/owlmic/labels/good%20first%20issue) or [`help wanted`](https://github.com/diveshpatil9104/owlmic/labels/help%20wanted).
3. Comment on the issue to express your interest before starting work to avoid collisions.

---

## 4. Pull Request Rules & Workflow

1. **Branch Naming**: `<type>/<short-kebab-description>`
   - `feat/speaker-loopback`
   - `fix/mic-buffer-underrun`
   - `docs/clarify-wire-spec`
2. **Conventional Commits**:
   ```text
   <type>(<scope>): <short imperative summary>

   [optional body explaining why this change was made]
   ```
   - Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`, `chore`, `build`.
   - Scopes: `android`, `pc`, `win`, `linux`, `proto`, `design`, `installer`, `scripts`, `ci`, `site`.
   - Example: `fix(pc): prevent audio jitter buffer underflow on rapid link switch`
3. **Lean Engineering**:
   - Zero speculative code or premature abstractions.
   - Surgical diffs: do not reformat lines outside your change scope.
   - No chatty comments. Comments explain non-obvious *why* and hardware quirks only.
4. **Strict Binary Ban in Git**:
   - Never commit compiled binaries (`.apk`, `.exe`, `.dll`, `.so`, `.zip`, `.aab`). Releases are distributed via GitHub Releases and CI artifacts.
5. **Product Name Spelling**:
   - The product name is strictly **Owlmic** (or **owlmic** in lowercase contexts, as defined in `design/copy.json`). Capitalizing the second syllable or hyphenating is forbidden.
6. **Green CI Required**:
   - All automated GitHub Actions checks (PC tests/clippy, Android tests/lint, product name validation) must pass before merging.
