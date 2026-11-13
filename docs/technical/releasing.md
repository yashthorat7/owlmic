# Release Process

This document details the step-by-step procedures for cutting a formal release of Owlmic, tagging Git milestones, compiling production artifacts, signing packages, and distributing updates.

---

## 1. Versioning Standard

Owlmic adheres to [Semantic Versioning 2.0.0](https://semver.org/).
- Version strings across `Cargo.toml`, `android/app/build.gradle.kts`, `pc/installer/windows/owlmic.iss`, and `protocol/README.md` must be bumped in lockstep before tagging.

---

## 2. Release Checklist

1. **Verify All Automated Tests**:
   - PC: `cargo test --workspace` (must pass 100% of unit & integration tests).
   - PC Linter: `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all --check`.
   - Android: `./gradlew testDebugUnitTest lintDebug`.
2. **Verify Protocol Vectors**:
   - `cargo test -p owlmic-proto` verifies wire encoders against `protocol/vectors/*.json`.
   - Android `./gradlew :core:test` validates Kotlin proto vectors against the same files.
3. **Verify Product Name Invariant**:
   - Run the CI name verification check (defined in `.github/workflows/ci.yml`) to ensure no incorrect casings exist across the codebase.
4. **Update Documentation & Changelog**:
   - Record user-facing highlights, fixes, and contributor credits in [`docs/CHANGELOG.md`](../CHANGELOG.md).
   - Ensure `README.md` download links and version badges match the target milestone.

---

## 3. Creating the Release Tag

When all checks are green:
```bash
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0
```

GitHub Actions triggers the release workflow (`.github/workflows/release.yml`) to generate:
- `Owlmic-Setup-1.0.0.exe`: Standalone Windows Installer with SHA-256 checksum.
- `owlmic-1.0.0.apk`: Sideloadable Android universal APK.
- `owlmic-1.0.0.aab`: Android App Bundle for Google Play Console submission.

---

## 4. Binary Signing & Integrity

- **Windows Installer**: Signed with Authenticode EV certificate via SignPath or local hardware token.
- **Android APK / AAB**: Signed with Google Play Upload Key using `apksigner`.
- **Release Checksums**: A `SHA256SUMS.txt` manifest containing SHA-256 digests is automatically attached to each GitHub Release.
