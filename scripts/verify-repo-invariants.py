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
        if rel.startswith("desktop/") and path.suffix == ".rs" and not rel.endswith("bt.rs"):
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
    if iss.exists():
        m = re.search(r'#define\s+MyAppVersion\s+"([^"]+)"', iss.read_text(encoding="utf-8"))
        if m:
            versions["owlmic.iss"] = m.group(1)

    if len(set(versions.values())) > 1:
        ERRORS.append(f"[VERSION LOCKSTEP] Version mismatch across manifests: {versions}")

def check_workflow_paths():
    workflows_dir = ROOT / ".github" / "workflows"
    if not workflows_dir.exists():
        return
    for yml in workflows_dir.glob("*.yml"):
        content = yml.read_text(encoding="utf-8")
        lines = content.splitlines()
        in_paths = False
        for idx, line in enumerate(lines, 1):
            stripped = line.strip()
            if stripped.startswith("paths:"):
                in_paths = True
                continue
            if in_paths:
                if stripped.startswith("- "):
                    raw_path = stripped[2:].strip().strip("'\"")
                    # Remove trailing glob stars (e.g. dir/** or dir/*)
                    clean_path = re.sub(r"/\*+.*$", "", raw_path)
                    target = ROOT / clean_path
                    if not target.exists():
                        ERRORS.append(
                            f"[WORKFLOW PATH] {yml.relative_to(ROOT)}:{idx} references missing path: '{raw_path}'"
                        )
                elif stripped and not stripped.startswith("#"):
                    in_paths = False

            # Check working-directory (ignore build output directories created during jobs)
            m_wd = re.search(r"working-directory:\s*['\"]?([^'\"\s#]+)", line)
            if m_wd:
                wd = m_wd.group(1)
                if wd not in {"dist", "target", "build", "out"} and not (ROOT / wd).exists():
                    ERRORS.append(
                        f"[WORKFLOW PATH] {yml.relative_to(ROOT)}:{idx} references missing working-directory: '{wd}'"
                    )

            # Check --manifest-path
            m_mp = re.search(r"--manifest-path\s+['\"]?([^'\"\s]+)", line)
            if m_mp:
                mp = m_mp.group(1)
                if not (ROOT / mp).exists():
                    ERRORS.append(
                        f"[WORKFLOW PATH] {yml.relative_to(ROOT)}:{idx} references missing --manifest-path: '{mp}'"
                    )

            # Check cache-dependency-path
            m_cd = re.search(r"cache-dependency-path:\s*['\"]?([^'\"\s#]+)", line)
            if m_cd:
                cd = m_cd.group(1)
                if not (ROOT / cd).exists():
                    ERRORS.append(
                        f"[WORKFLOW PATH] {yml.relative_to(ROOT)}:{idx} references missing cache-dependency-path: '{cd}'"
                    )

            # Check local workflow uses
            m_uses = re.search(r"uses:\s*\./([^\s#]+)", line)
            if m_uses:
                wf = m_uses.group(1)
                if not (ROOT / wf).exists():
                    ERRORS.append(
                        f"[WORKFLOW PATH] {yml.relative_to(ROOT)}:{idx} references missing local workflow: '{wf}'"
                    )

def main():
    files = git_tracked_files()
    check_binaries_and_casing(files)
    check_tokio_restriction(files)
    check_json_sources()
    check_version_lockstep()
    check_workflow_paths()

    if ERRORS:
        print("\n❌ Owlmic Repository Invariant Checks FAILED:")
        for err in ERRORS:
            print(f"  - {err}")
        sys.exit(1)
    print("✅ All Owlmic repository invariants passed (casing, binary ban, concurrency, JSON, versions, workflows).")

if __name__ == "__main__":
    main()
