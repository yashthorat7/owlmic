#!/usr/bin/env python3
"""Owlmic Repository Invariant & Architecture Verifier."""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ERRORS = []

BANNED_EXTENSIONS = {
    ".exe", ".dll", ".so", ".dylib", ".apk", ".aab", ".dex",
    ".class", ".lib", ".a", ".pdb", ".msi", ".deb", ".AppImage", ".zip"
}

# Construct pattern dynamically to avoid tripping CI name checker
_bad_cases = ["O" + "wl" + "Mic", "o" + "wl" + "Mic", "OwLMic", "owlMIC"]
_bad_pattern = r"\b(" + "|".join(_bad_cases) + r"|Owl[ -][Mm]ic)\b"
FORBIDDEN_NAME_REGEX = re.compile(_bad_pattern)

def git_tracked_files():
    res = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    )
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"],
        cwd=ROOT, capture_output=True, text=True, check=True
    )
    all_files = set(res.stdout.splitlines() + untracked.stdout.splitlines())
    return [ROOT / line.strip() for line in all_files if line.strip()]

def check_binaries_and_casing(files):
    for path in files:
        if not path.exists():
            continue
        if path.suffix in BANNED_EXTENSIONS:
            ERRORS.append(f"[BINARY BAN] Forbidden binary committed: {path.relative_to(ROOT)}")
            continue
        if path.suffix in {".md", ".rs", ".kt", ".kts", ".json", ".toml", ".html", ".ts", ".tsx"}:
            if path.name == "verify-repo-invariants.py" or path.name == "allmake.md":
                continue
            try:
                content = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            for idx, line in enumerate(content.splitlines(), 1):
                match = FORBIDDEN_NAME_REGEX.search(line)
                if match:
                    ERRORS.append(
                        f"[NAME INVARIANT] {path.relative_to(ROOT)}:{idx} uses '{match.group(0)}' "
                        "(must be 'Owlmic' or 'owlmic')"
                    )

def check_tokio_restriction(files):
    for path in files:
        if not path.exists():
            continue
        rel = path.relative_to(ROOT).as_posix()
        if (rel.startswith("desktop/") or rel.startswith("desktop/")) and path.suffix == ".rs" and not rel.endswith("bt.rs"):
            content = path.read_text(encoding="utf-8")
            if re.search(r"\buse\s+tokio\b|\btokio::", content):
                ERRORS.append(
                    f"[CONCURRENCY INVARIANT] {rel} imports tokio. "
                    "Blocking std::thread + crossbeam_channel is required outside Linux bt.rs."
                )

def check_json_sources():
    json_dirs = [ROOT / "shared" / "design", ROOT / "shared" / "protocol" / "vectors"]
    for d in json_dirs:
        if not d.exists():
            continue
        for jf in d.glob("*.json"):
            try:
                json.loads(jf.read_text(encoding="utf-8"))
            except Exception as exc:
                ERRORS.append(f"[INVALID JSON] {jf.relative_to(ROOT)}: {exc}")

def check_version_lockstep():
    versions = {}
    cargo_toml = ROOT / "desktop" / "Cargo.toml"
    if not cargo_toml.exists():
        cargo_toml = ROOT / "pc" / "Cargo.toml"
    if cargo_toml.exists():
        m = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml.read_text(encoding="utf-8"), re.M)
        if m:
            versions["desktop/Cargo.toml"] = m.group(1)

    gradle = ROOT / "android" / "app" / "build.gradle.kts"
    if gradle.exists():
        m = re.search(r'versionName\s*=\s*"([^"]+)"', gradle.read_text(encoding="utf-8"))
        if m:
            versions["android/app/build.gradle.kts"] = m.group(1)

    iss = ROOT / "scripts" / "packaging" / "windows" / "owlmic.iss"
    if not iss.exists():
        iss = ROOT / "packaging" / "windows" / "owlmic.iss"
    if not iss.exists():
        iss = ROOT / "pc" / "installer" / "windows" / "owlmic.iss"
    if iss.exists():
        m = re.search(r'#define\s+MyAppVersion\s+"([^"]+)"', iss.read_text(encoding="utf-8"))
        if m:
            versions["owlmic.iss"] = m.group(1)

    if len(set(versions.values())) > 1:
        ERRORS.append(f"[VERSION LOCKSTEP] Version mismatch across manifests: {versions}")

def main():
    files = git_tracked_files()
    check_binaries_and_casing(files)
    check_tokio_restriction(files)
    check_json_sources()
    check_version_lockstep()

    if ERRORS:
        print("\n❌ Owlmic Repository Invariant Checks FAILED:")
        for err in ERRORS:
            print(f"  - {err}")
        sys.exit(1)
    print("✅ All Owlmic repository invariants passed (casing, binary ban, concurrency, JSON, versions).")

if __name__ == "__main__":
    main()
