#!/usr/bin/env python3
"""Owlmic Release Version Bumper & AI Prompt Generator.

Updates the lockstep version manifests (desktop/Cargo.toml,
android/app/build.gradle.kts, scripts/packaging/windows/owlmic.iss),
verifies repository invariants, and prints a ready-to-copy AI prompt
covering all remaining release update stages (changelog, docs, website,
and verification).
"""

import argparse
from datetime import date
import platform
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
CARGO_TOML = ROOT / "desktop" / "Cargo.toml"
GRADLE_KTS = ROOT / "android" / "app" / "build.gradle.kts"
OWLMIC_ISS = ROOT / "scripts" / "packaging" / "windows" / "owlmic.iss"
VERIFIER = ROOT / "scripts" / "core" / "verify-repo-invariants.py"

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+$")


def get_current_version() -> str:
    """Read canonical version from desktop/Cargo.toml."""
    text = CARGO_TOML.read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        raise RuntimeError(f"Could not find version in {CARGO_TOML}")
    return m.group(1)


def bump_manifests(new_version: str) -> tuple[int, int]:
    """Update Cargo.toml, build.gradle.kts, and owlmic.iss in lockstep."""
    cargo_text = CARGO_TOML.read_text(encoding="utf-8")
    cargo_updated, n = re.subn(
        r'^(version\s*=\s*")[^"]+(")',
        rf"\g<1>{new_version}\g<2>",
        cargo_text,
        count=1,
        flags=re.M,
    )
    if n != 1:
        raise RuntimeError(f"Failed to update version in {CARGO_TOML}")
    CARGO_TOML.write_text(cargo_updated, encoding="utf-8")

    gradle_text = GRADLE_KTS.read_text(encoding="utf-8")
    vc_match = re.search(r"(versionCode\s*=\s*)(\d+)", gradle_text)
    if not vc_match:
        raise RuntimeError(f"Could not find versionCode in {GRADLE_KTS}")
    old_vc = int(vc_match.group(2))
    new_vc = old_vc + 1

    gradle_updated = re.sub(
        r'(versionCode\s*=\s*)\d+',
        rf"\g<1>{new_vc}",
        gradle_text,
        count=1,
    )
    gradle_updated, n = re.subn(
        r'(versionName\s*=\s*")[^"]+(")',
        rf"\g<1>{new_version}\g<2>",
        gradle_updated,
        count=1,
    )
    if n != 1:
        raise RuntimeError(f"Failed to update versionName in {GRADLE_KTS}")
    GRADLE_KTS.write_text(gradle_updated, encoding="utf-8")

    iss_text = OWLMIC_ISS.read_text(encoding="utf-8")
    iss_updated, n = re.subn(
        r'(#define\s+MyAppVersion\s+")[^"]+(")',
        rf"\g<1>{new_version}\g<2>",
        iss_text,
        count=1,
    )
    if n != 1:
        raise RuntimeError(f"Failed to update MyAppVersion in {OWLMIC_ISS}")
    OWLMIC_ISS.write_text(iss_updated, encoding="utf-8")

    return old_vc, new_vc


def build_prompt(old_version: str, new_version: str, old_vc: int, new_vc: int) -> str:
    """Build the comprehensive AI prompt for all post-bump release stages."""
    today = date.today().isoformat()
    return f"""Prepare the Owlmic v{new_version} release (upgrading from v{old_version}, Android versionCode {old_vc} -> {new_vc}).
The lockstep manifests (`desktop/Cargo.toml`, `android/app/build.gradle.kts`, `scripts/packaging/windows/owlmic.iss`) have already been bumped to `{new_version}`.

Complete all remaining release stages below:

1. **Inspect Changes Since v{old_version}**
   - Run `git log v{old_version}..HEAD --oneline` (or `git log -n 30 --oneline` if tag `v{old_version}` does not exist) and `git status` to review what changed in this release.

2. **Update `docs/CHANGELOG.md`**
   - Add a new section `## [{new_version}] - {today}` at the top of the changelog following Keep a Changelog format (`### Added`, `### Changed`, `### Fixed` as applicable).
   - Summarize real user-facing changes, fixes, and packaging updates observed in git history—do not invent unverified features.
   - Update any artifact filename references (e.g. `Owlmic-v{new_version}-linux-x86_64.tar.gz`) if mentioned in the new entry.

3. **Update Version Strings & Download Links Across Docs and Website**
   Update references from `{old_version}` to `{new_version}` in:
   - `README.md` (download badge, installer/tarball/APK filenames & release links)
   - `docs/README.md` (changelog version range)
   - `docs/technical/building.md` (expected `releases/` output filenames)
   - `docs/technical/releasing.md` (tag example `v{new_version}` and artifact list)
   - `docs/user/install-android.md` (`Owlmic-v{new_version}.apk`)
   - `docs/user/install-windows.md` (`Owlmic-Setup-{new_version}.exe`)
   - `website/src/App.tsx` (version badge, release banner, Windows `.exe` and Android `.apk` download URLs)
   - `website/src/docsData.ts` (installer/APK filenames and tag instructions)
   Do NOT modify test/snapshot fixtures in `desktop/crates/owlmic-ui/src/view.rs` or `panel.rs`, and do NOT alter third-party dependency versions in `Cargo.lock` or `package-lock.json`.

4. **Run Full Verification**
   - Run `python scripts/core/verify-repo-invariants.py`
   - Run `python scripts/core/check.py`
   - Run `npm run lint && npm run build` inside `website/`
   - Fix any issues and re-verify before reporting completion.

5. **Provide Release Git Commands (Do Not Execute Mutating Git Commands)**
   - Output a single copy-pasteable command block (separate lines, no `&&`) to stage, commit (`chore(repo): bump release to v{new_version}`), tag (`git tag -a v{new_version} -m "Release v{new_version}"`), and push the branch and tag."""


def copy_to_clipboard(text: str) -> bool:
    """Best-effort clipboard copy using OS native tools."""
    system = platform.system()
    cmd = None
    if system == "Windows":
        cmd = ["clip"]
    elif system == "Darwin":
        cmd = ["pbcopy"]
    elif system == "Linux":
        cmd = ["xclip", "-selection", "clipboard"]
    if not cmd:
        return False
    try:
        subprocess.run(cmd, input=text.encode("utf-8"), check=True)
        return True
    except Exception:
        return False


def main() -> int:
    parser = argparse.ArgumentParser(description="Bump Owlmic release version and generate AI release prompt.")
    parser.add_argument("--version", "-v", help="Target SemVer version (X.Y.Z)")
    args = parser.parse_args()

    current = get_current_version()
    major, minor, _ = (int(p) for p in current.split("."))
    suggested = f"{major}.{minor + 1}.0"
    print(f"\nCurrent Owlmic version: {current}")

    target = args.version.strip() if args.version else ""
    while not target:
        try:
            raw = input(f"Enter new release version (e.g. {suggested}): ").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nCancelled.")
            return 1
        if not SEMVER_RE.match(raw):
            print(f"  Invalid format. Expected SemVer X.Y.Z (e.g. {suggested}).")
            continue
        if raw == current:
            print(f"  Version is already {current}. Enter a new version.")
            continue
        target = raw

    if not SEMVER_RE.match(target) or target == current:
        print(f"Invalid or unchanged version: {target}")
        return 1

    old_vc, new_vc = bump_manifests(target)
    print(f"\nUpdated manifests to {target} (Android versionCode: {old_vc} -> {new_vc}):")
    print(f"  - {CARGO_TOML.relative_to(ROOT)}")
    print(f"  - {GRADLE_KTS.relative_to(ROOT)}")
    print(f"  - {OWLMIC_ISS.relative_to(ROOT)}")

    print("\nRunning repository invariant check...")
    res = subprocess.run([sys.executable, str(VERIFIER)], cwd=ROOT)
    if res.returncode != 0:
        print("Invariant check failed. Review errors above.")
        return res.returncode

    prompt = build_prompt(current, target, old_vc, new_vc)
    copied = copy_to_clipboard(prompt)

    print("\n" + "=" * 70)
    print("COPY-PASTE AI PROMPT (ALL REMAINING RELEASE STAGES)")
    if copied:
        print("[Copied to clipboard]")
    print("=" * 70)
    print(prompt)
    print("=" * 70 + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
