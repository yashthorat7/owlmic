## Summary

<!-- What does this change, and why? If it fixes an issue: Fixes #123 -->

## Type

- [ ] Bug fix
- [ ] Feature
- [ ] Performance
- [ ] Refactor
- [ ] Documentation
- [ ] Build / CI / installer

## Area

- [ ] Android app (`android/`)
- [ ] Windows app (`desktop/`)
- [ ] Installer
- [ ] CI

## How it was tested

<!--
UI: attach a screenshot or a short recording.
Audio or video: which phone, which Windows version, which link (USB debugging, USB tethering, Wi-Fi, Bluetooth), and for how long.
Logic: the tests you added or ran.
-->

## Checklist

- [ ] Android: `./gradlew testDebugUnitTest lintDebug` passes
- [ ] Windows: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass
- [ ] No new dependency without a maintainer's OK
- [ ] Comments only where the why is not obvious
