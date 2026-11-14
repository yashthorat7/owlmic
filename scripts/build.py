#!/usr/bin/env python3
"""Owlmic Unified Release Builder.

Builds, packages, and verifies production-ready Owlmic distributables for
Android, Windows, and Linux, routing output artifacts to the centralized `dist/` directory.
"""

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"
DIST_RELEASE = DIST / "release"
DIST_ANDROID = DIST / "android"
DIST_WINDOWS = DIST / "windows"
DIST_LINUX = DIST / "linux"
PACKAGING_MANIFEST = ROOT / "scripts" / "packaging" / "windows" / "dependencies.json"

SOFTCAM_HASH = "9D635E0AF682A883C3C7D407513D47E59B0883771701B101216820DFCF997F0B"
SOFTCAM_URL = "https://raw.githubusercontent.com/diveshpatil9104/owlmic/3f78b5767f64f4542b4307c586da4b609d54d38e/desktop/softcam.dll"

DRIVER_HASH = "B950E39F01AF1D04EA623C8F6D8EB9B6EA5C477C637295FABF20631C85116BFB"
DRIVER_URLS = [
    "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip",
    "https://web.archive.org/web/20240901000000id_/https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip",
]


def load_dependencies() -> dict:
    """Load Windows packaging dependencies manifest from dependencies.json."""
    if PACKAGING_MANIFEST.exists():
        try:
            return json.loads(PACKAGING_MANIFEST.read_text(encoding="utf-8"))
        except Exception as e:
            print(f"  [WARN] Failed to parse dependencies.json ({e}), using default fallback metadata.")
    return {
        "dependencies": {
            "vbcable": {
                "name": "VB-CABLE Virtual Audio Device",
                "role": "Microphone virtual audio cable driver (Owlmic Mic)",
                "version": "Pack45",
                "upstream_url": "https://vb-audio.com/Cable/",
                "primary_url": DRIVER_URLS[0],
                "mirror_urls": DRIVER_URLS[1:],
                "sha256": DRIVER_HASH,
                "type": "zip",
                "local_cache": "scripts/packaging/cache/vbcable.zip",
                "local_dir": "scripts/packaging/windows/driver",
                "primary_file": "VBCABLE_Setup_x64.exe",
            },
            "softcam": {
                "name": "Softcam DirectShow Virtual Camera Filter",
                "role": "DirectShow virtual camera fallback filter for Windows 10 (Owlmic Cam)",
                "version": "0.1.0",
                "upstream_url": "https://github.com/tshino/softcam",
                "primary_url": SOFTCAM_URL,
                "mirror_urls": [
                    "https://github.com/diveshpatil9104/owlmic/releases/download/v0.1.0/softcam.dll",
                ],
                "sha256": SOFTCAM_HASH,
                "type": "file",
                "local_cache": "scripts/packaging/cache/softcam.dll",
                "local_dir": "scripts/packaging/cache",
                "primary_file": "softcam.dll",
            },
        }
    }


def save_dependencies(data: dict):
    """Save updated dependencies manifest."""
    PACKAGING_MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    PACKAGING_MANIFEST.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def sha256_file(path: Path) -> str:
    """Calculate the SHA-256 digest of a file in lowercase."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest().lower()


def get_version() -> str:
    """Read canonical version from desktop/Cargo.toml."""
    cargo_toml = ROOT / "desktop" / "Cargo.toml"
    if not cargo_toml.exists():
        cargo_toml = ROOT / "pc" / "Cargo.toml"
    if cargo_toml.exists():
        m = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml.read_text(encoding="utf-8"), re.M)
        if m:
            return m.group(1)
    return "0.1.0"


def ensure_dirs():
    """Ensure release directories and driver cache exist."""
    DIST_RELEASE.mkdir(parents=True, exist_ok=True)
    DIST_ANDROID.mkdir(parents=True, exist_ok=True)
    DIST_WINDOWS.mkdir(parents=True, exist_ok=True)
    DIST_LINUX.mkdir(parents=True, exist_ok=True)
    (ROOT / "scripts" / "packaging" / "cache").mkdir(parents=True, exist_ok=True)


def download_file(url: str, dest: Path, expected_hash: str = None) -> bool:
    """Download a file over HTTPS with hash verification."""
    print(f"  Downloading {url} ...")
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) OwlmicBuilder/1.0"},
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as response, open(dest, "wb") as out_file:
            shutil.copyfileobj(response, out_file)
        if expected_hash:
            actual = sha256_file(dest).upper()
            if actual != expected_hash.upper():
                print(f"  [WARN] Checksum mismatch for {url}: got {actual}, expected {expected_hash}")
                dest.unlink(missing_ok=True)
                return False
        return True
    except Exception as e:
        print(f"  [WARN] Download error: {e}")
        dest.unlink(missing_ok=True)
        return False


def find_iscc() -> Path | None:
    """Locate the Inno Setup 6 compiler executable (ISCC.exe).

    Searches PATH, user programs, Program Files, local packaging cache,
    and attempts non-interactive winget setup if unavailable on Windows.
    """
    iscc = shutil.which("ISCC.exe") or shutil.which("iscc")
    if iscc:
        return Path(iscc)
    candidates = [
        Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "Inno Setup 6" / "ISCC.exe",
        Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Inno Setup 6" / "ISCC.exe",
        Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Inno Setup 6" / "ISCC.exe",
        ROOT / "scripts" / "packaging" / "cache" / "innosetup" / "ISCC.exe",
    ]
    for c in candidates:
        if c.exists():
            return c

    # Attempt automatic winget installation on Windows if missing
    if platform.system() == "Windows":
        winget = shutil.which("winget.exe") or shutil.which("winget")
        if winget:
            print("  ISCC.exe not found. Attempting non-interactive setup via winget...")
            cmd = [
                winget, "install", "--id", "JRSoftware.InnoSetup",
                "-e", "--silent", "--accept-source-agreements", "--accept-package-agreements"
            ]
            try:
                res = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
                if res.returncode == 0:
                    for c in candidates:
                        if c.exists():
                            print(f"  [OK] Inno Setup installed via winget: {c}")
                            return c
            except Exception as e:
                print(f"  [WARN] winget installation attempt failed: {e}")

    return None


def ensure_windows_prerequisites(force_refresh: bool = False) -> bool:
    """Verify or fetch softcam.dll and VB-CABLE driver pack into local folders.

    Checks local folders first. If files are missing, corrupted, or force_refresh is requested,
    reaches out to primary and mirror URLs defined in dependencies.json and verifies SHA-256 digests.
    """
    manifest = load_dependencies()
    deps = manifest.get("dependencies", {})

    # 1. Softcam (Windows 10 DirectShow fallback virtual camera)
    softcam_spec = deps.get("softcam", {})
    softcam_hash = softcam_spec.get("sha256", SOFTCAM_HASH)
    softcam_urls = [softcam_spec.get("primary_url")] + softcam_spec.get("mirror_urls", [])
    softcam_urls = [u for u in softcam_urls if u]
    softcam_path = ROOT / softcam_spec.get("local_cache", "scripts/packaging/cache/softcam.dll")

    if force_refresh or not softcam_path.exists() or sha256_file(softcam_path).upper() != softcam_hash.upper():
        print(f"  softcam.dll missing or outdated in {softcam_path.relative_to(ROOT)}. Downloading ...")
        softcam_path.parent.mkdir(parents=True, exist_ok=True)
        downloaded = False
        for url in softcam_urls:
            if download_file(url, softcam_path, softcam_hash):
                downloaded = True
                break
        if not downloaded:
            print("  [ERROR] Failed to obtain verified softcam.dll from remote URLs.")
            return False
        print(f"  [OK] softcam.dll verified and saved to local folder: {softcam_path.relative_to(ROOT)}")
    else:
        print(f"  [OK] softcam.dll is present in local folder and verified ({softcam_path.relative_to(ROOT)}).")

    # 2. VB-CABLE driver pack (Windows microphone virtual audio endpoint)
    driver_spec = deps.get("vbcable", {})
    driver_hash = driver_spec.get("sha256", DRIVER_HASH)
    driver_urls = [driver_spec.get("primary_url")] + driver_spec.get("mirror_urls", [])
    driver_urls = [u for u in driver_urls if u]
    driver_dir = ROOT / driver_spec.get("local_dir", "scripts/packaging/windows/driver")
    driver_check_file = driver_dir / driver_spec.get("primary_file", "VBCABLE_Setup_x64.exe")

    if force_refresh or not driver_check_file.exists():
        print(f"  VB-CABLE driver files missing in {driver_dir.relative_to(ROOT)}. Downloading driver pack ...")
        temp_zip = ROOT / driver_spec.get("local_cache", "scripts/packaging/cache/vbcable.zip")
        temp_zip.parent.mkdir(parents=True, exist_ok=True)
        downloaded = False
        for url in driver_urls:
            if download_file(url, temp_zip, driver_hash):
                downloaded = True
                break
        if not downloaded:
            print("  [ERROR] Failed to download verified VB-CABLE driver pack from remote URLs.")
            return False
        driver_dir.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(temp_zip, "r") as zf:
            zf.extractall(driver_dir)
        temp_zip.unlink(missing_ok=True)
        print(f"  [OK] Driver pack unpacked to local folder: {driver_dir.relative_to(ROOT)}")
    else:
        print(f"  [OK] Microphone driver pack is present in local folder ({driver_dir.relative_to(ROOT)}).")

    return True


def check_dependencies() -> bool:
    """Audit and report state of all Windows packaging dependencies."""
    manifest = load_dependencies()
    deps = manifest.get("dependencies", {})
    all_ok = True

    print("\n======================================================================")
    print("            OWLMIC WINDOWS PACKAGING DEPENDENCIES AUDIT")
    print("======================================================================")

    # 1. VB-CABLE
    vbc = deps.get("vbcable", {})
    driver_dir = ROOT / vbc.get("local_dir", "scripts/packaging/windows/driver")
    vbc_file = driver_dir / vbc.get("primary_file", "VBCABLE_Setup_x64.exe")
    vbc_present = vbc_file.exists()
    all_ok = all_ok and vbc_present
    status_vbc = "PRESENT" if vbc_present else "MISSING"
    print(f"\n[1] {vbc.get('name', 'VB-CABLE Audio Driver')} ({vbc.get('version', 'N/A')})")
    print(f"    Role         : {vbc.get('role', 'N/A')}")
    print(f"    Local Status : {status_vbc} ({vbc_file.relative_to(ROOT) if vbc_present else vbc_file})")
    print(f"    Primary URL  : {vbc.get('primary_url', 'N/A')}")
    print(f"    Expected SHA : {vbc.get('sha256', 'N/A')}")

    # 2. Softcam
    sc = deps.get("softcam", {})
    sc_file = ROOT / sc.get("local_cache", "scripts/packaging/cache/softcam.dll")
    sc_present = sc_file.exists()
    sc_hash_ok = False
    if sc_present:
        actual_hash = sha256_file(sc_file).upper()
        sc_hash_ok = (actual_hash == sc.get("sha256", "").upper())
    all_ok = all_ok and sc_hash_ok
    status_sc = "PRESENT & VERIFIED" if sc_hash_ok else ("CHECKSUM MISMATCH" if sc_present else "MISSING")
    print(f"\n[2] {sc.get('name', 'Softcam DirectShow Filter')} ({sc.get('version', 'N/A')})")
    print(f"    Role         : {sc.get('role', 'N/A')}")
    print(f"    Local Status : {status_sc} ({sc_file.relative_to(ROOT) if sc_present else sc_file})")
    print(f"    Primary URL  : {sc.get('primary_url', 'N/A')}")
    print(f"    Expected SHA : {sc.get('sha256', 'N/A')}")

    # 3. Owlmic Media Foundation VCAM
    vcam_crate = ROOT / "desktop" / "crates" / "owlmic-vcam"
    vcam_bin = ROOT / "desktop" / "target" / "release" / "owlmic_vcam.dll"
    print("\n[3] Owlmic Media Foundation Virtual Camera (Windows 11)")
    print("    Role         : Native Media Foundation virtual camera driver (IMFVirtualCamera)")
    print(f"    Source Crate : {'PRESENT' if vcam_crate.exists() else 'MISSING'} ({vcam_crate.relative_to(ROOT)})")
    print(f"    Release DLL  : {'BUILT' if vcam_bin.exists() else 'NOT YET BUILT'} ({vcam_bin.relative_to(ROOT) if vcam_bin.exists() else 'run build to compile'})")

    # 4. Inno Setup Compiler
    iscc = find_iscc()
    iscc_ok = iscc is not None
    all_ok = all_ok and iscc_ok
    print("\n[4] Inno Setup Compiler (ISCC.exe)")
    print(f"    Status       : {'DETECTED (' + str(iscc) + ')' if iscc else 'NOT FOUND'}")

    print("\n----------------------------------------------------------------------")
    if all_ok:
        print("  [SUCCESS] All Windows packaging prerequisites are satisfied locally.")
    else:
        print("  [NOTICE] Run 'python scripts/build.py --fetch-deps' to download missing local components.")
    print("======================================================================\n")
    return all_ok


def update_dependency_link(component: str, url: str, expected_hash: str = None, version: str = None) -> bool:
    """Update remote link and checksum for a packaging dependency, installing it locally."""
    manifest = load_dependencies()
    deps = manifest.setdefault("dependencies", {})
    key = component.lower().strip()
    if key not in deps:
        print(f"  [ERROR] Unknown dependency: '{component}'. Supported: {list(deps.keys())}")
        return False

    spec = deps[key]
    print(f"\nUpdating dependency '{key}' ({spec.get('name')}) ...")
    print(f"  New Primary URL: {url}")

    cache_dir = ROOT / "scripts" / "packaging" / "cache"
    cache_dir.mkdir(parents=True, exist_ok=True)
    temp_download = cache_dir / f"update_{key}_{Path(url.split('?')[0]).name}"

    print(f"  Downloading from new URL to verify ...")
    if not download_file(url, temp_download):
        print(f"  [ERROR] Failed to download from {url}")
        return False

    computed_hash = sha256_file(temp_download).upper()
    print(f"  Computed SHA-256: {computed_hash}")

    if expected_hash:
        if computed_hash != expected_hash.upper():
            print(f"  [ERROR] Hash mismatch! Expected {expected_hash}, got {computed_hash}")
            temp_download.unlink(missing_ok=True)
            return False
        print("  [OK] Hash matches specified checksum.")

    # Archive or raw file installation into local project folders
    if spec.get("type") == "zip":
        target_dir = ROOT / spec.get("local_dir", "scripts/packaging/windows/driver")
        target_dir.mkdir(parents=True, exist_ok=True)
        print(f"  Unpacking updated archive to local folder {target_dir.relative_to(ROOT)} ...")
        with zipfile.ZipFile(temp_download, "r") as zf:
            zf.extractall(target_dir)
        temp_download.unlink(missing_ok=True)
    else:
        target_file = ROOT / spec.get("local_cache", "scripts/packaging/cache/softcam.dll")
        shutil.move(str(temp_download), str(target_file))
        print(f"  Installed updated binary to local folder: {target_file.relative_to(ROOT)}")

    # Update manifest data
    spec["primary_url"] = url
    spec["sha256"] = computed_hash
    if version:
        spec["version"] = version
    manifest["updated_at"] = "2026-11-14"
    save_dependencies(manifest)

    print(f"  [SUCCESS] Updated '{key}' in scripts/packaging/windows/dependencies.json.")
    print("  Local folders and manifest are in sync.\n")
    return True


# =============================================================================
# PLATFORM BUILD IMPLEMENTATIONS
# =============================================================================

def build_android(version: str, out_dir: Path = DIST_ANDROID) -> Path | None:
    """Build production Android Release APK."""
    print("\n========================================================")
    print("  BUILDING ANDROID CLIENT (RELEASE APK)")
    print("========================================================")
    out_dir.mkdir(parents=True, exist_ok=True)
    android_dir = ROOT / "android"
    gradlew = android_dir / ("gradlew.bat" if platform.system() == "Windows" else "gradlew")
    if not gradlew.exists():
        print("  [ERROR] Gradle wrapper not found at:", gradlew)
        return None

    cmd = [str(gradlew), "assembleRelease"]
    print(f"  Running: {' '.join(cmd)} ...")
    res = subprocess.run(cmd, cwd=android_dir)

    apk_source = android_dir / "app" / "build" / "outputs" / "apk" / "release" / "app-release.apk"
    if not apk_source.exists():
        unsigned = android_dir / "app" / "build" / "outputs" / "apk" / "release" / "app-release-unsigned.apk"
        if unsigned.exists():
            apk_source = unsigned

    if res.returncode != 0 or not apk_source.exists():
        print("  [ERROR] Android release build failed.")
        return None

    dest_apk = out_dir / f"Owlmic-v{version}.apk"
    shutil.copy2(apk_source, dest_apk)
    size_mb = dest_apk.stat().st_size / (1024 * 1024)
    sha = sha256_file(dest_apk)

    print(f"\n  [SUCCESS] Android APK created: {dest_apk}")
    print(f"  Size:    {size_mb:.2f} MB")
    print(f"  SHA-256: {sha}")
    return dest_apk


def build_windows(version: str, out_dir: Path = DIST_WINDOWS, mode: str = "all") -> list[Path]:
    """Build Windows packages (installer, portable standalone exe, or both)."""
    print("\n========================================================")
    print(f"  BUILDING WINDOWS DESKTOP ({mode.upper()})")
    print("========================================================")
    out_dir.mkdir(parents=True, exist_ok=True)
    artifacts = []

    if platform.system() != "Windows":
        print("  [ERROR] Windows desktop build must run on a Windows host.")
        return artifacts

    if not ensure_windows_prerequisites():
        return artifacts

    desktop_dir = ROOT / "desktop"
    manifest = "desktop/Cargo.toml"
    print("  Compiling release binaries (cargo build --release) ...")
    env = os.environ.copy()
    env["RUSTFLAGS"] = "-C target-feature=+crt-static"
    res = subprocess.run(["cargo", "build", "--release", "--manifest-path", manifest], cwd=ROOT, env=env)
    if res.returncode != 0:
        print("  [WARN] Static CRT build failed, retrying standard release build ...")
        env.pop("RUSTFLAGS", None)
        res = subprocess.run(["cargo", "build", "--release", "--manifest-path", manifest], cwd=ROOT, env=env)
        if res.returncode != 0:
            print("  [ERROR] Rust build failed.")
            return artifacts

    exe = desktop_dir / "target" / "release" / "owlmic.exe"
    vcam = desktop_dir / "target" / "release" / "owlmic_vcam.dll"
    if not exe.exists() or not vcam.exists():
        print("  [ERROR] Expected binaries missing from target/release.")
        return artifacts

    # 1. Portable Standalone Binary
    if mode in ("all", "portable"):
        standalone = out_dir / f"Owlmic-v{version}-windows-x64.exe"
        shutil.copy2(exe, standalone)
        size_mb = standalone.stat().st_size / (1024 * 1024)
        sha = sha256_file(standalone)
        print(f"\n  [SUCCESS] Windows Portable Binary: {standalone}")
        print(f"  Size:    {size_mb:.2f} MB")
        print(f"  SHA-256: {sha}")
        artifacts.append(standalone)

    # 2. Inno Setup Installer
    if mode in ("all", "installer"):
        iscc = find_iscc()
        if not iscc:
            print("  [WARN] Inno Setup compiler (ISCC.exe) not found.")
            print("  Please install Inno Setup 6: https://jrsoftware.org/isdl.php")
        else:
            iss_file = ROOT / "scripts" / "packaging" / "windows" / "owlmic.iss"
            print(f"  Compiling installer with Inno Setup ({iscc}) ...")
            inno_cmd = [
                str(iscc),
                f"/DMyAppVersion={version}",
                f"/O{str(out_dir)}",
                str(iss_file),
            ]
            res_inno = subprocess.run(inno_cmd, cwd=iss_file.parent)
            if res_inno.returncode == 0:
                setup_exe = out_dir / f"Owlmic-Setup-{version}.exe"
                if setup_exe.exists():
                    size_mb = setup_exe.stat().st_size / (1024 * 1024)
                    sha = sha256_file(setup_exe)
                    print(f"\n  [SUCCESS] Windows Installer created: {setup_exe}")
                    print(f"  Size:    {size_mb:.2f} MB")
                    print(f"  SHA-256: {sha}")
                    if size_mb > 5.0:
                        print(f"  [WARN] Installer size ({size_mb:.2f} MB) exceeds 5.0 MB budget.")
                    artifacts.append(setup_exe)
                else:
                    print(f"  [ERROR] Setup executable not found at: {setup_exe}")
            else:
                print("  [ERROR] Inno Setup compilation failed.")

    return artifacts


def build_linux(version: str, out_dir: Path = DIST_LINUX) -> Path | None:
    """Build Linux portable package (.tar.gz with systemd unit and setup scripts)."""
    print("\n========================================================")
    print("  BUILDING LINUX PACKAGE")
    print("========================================================")
    out_dir.mkdir(parents=True, exist_ok=True)
    archive_name = f"Owlmic-v{version}-linux-x86_64.tar.gz"
    dest_archive = out_dir / archive_name
    staging_dir = ROOT / "scripts" / "packaging" / "linux" / "staging"
    staging_dir.mkdir(parents=True, exist_ok=True)

    svc = ROOT / "scripts" / "packaging" / "linux" / "owlmic.service"
    setup = ROOT / "scripts" / "packaging" / "linux" / "setup-linux-devices.sh"
    if svc.exists():
        shutil.copy2(svc, staging_dir / "owlmic.service")
    if setup.exists():
        shutil.copy2(setup, staging_dir / "setup-linux-devices.sh")

    desktop_dir = ROOT / "desktop"
    manifest = "desktop/Cargo.toml"
    if platform.system() == "Linux":
        print("  Compiling release binaries (cargo build --release) ...")
        res = subprocess.run(["cargo", "build", "--release", "--manifest-path", manifest], cwd=ROOT)
        if res.returncode != 0:
            print("  [ERROR] Linux build failed.")
            shutil.rmtree(staging_dir, ignore_errors=True)
            return None
        bin_path = desktop_dir / "target" / "release" / "owlmic"
        if bin_path.exists():
            shutil.copy2(bin_path, staging_dir / "owlmic")
    else:
        print(f"  [INFO] Host OS is {platform.system()}. Packaging Linux deployment configuration...")
        print("  Notice: Native Linux binary is compiled automatically via GitHub Actions CI on Ubuntu runners.")
        # Check if cross-compiled binary or pre-built binary exists
        cross_bin = desktop_dir / "target" / "x86_64-unknown-linux-gnu" / "release" / "owlmic"
        if cross_bin.exists():
            shutil.copy2(cross_bin, staging_dir / "owlmic")

    import tarfile
    with tarfile.open(dest_archive, "w:gz") as tar:
        for f in staging_dir.iterdir():
            tar.add(f, arcname=f.name)
    shutil.rmtree(staging_dir, ignore_errors=True)

    size_mb = dest_archive.stat().st_size / (1024 * 1024)
    sha = sha256_file(dest_archive)
    print(f"\n  [SUCCESS] Linux package created: {dest_archive}")
    print(f"  Size:    {size_mb:.2f} MB")
    print(f"  SHA-256: {sha}")
    return dest_archive


# =============================================================================
# COMPOSITE SUITES & RELEASE BUNDLE
# =============================================================================

def generate_checksums(directory: Path = DIST) -> Path | None:
    """Generate SHA256SUMS.txt for all built release artifacts in target directory."""
    directory.mkdir(parents=True, exist_ok=True)
    valid_exts = {".exe", ".apk", ".tar.gz", ".zip"}
    artifacts = []
    for f in directory.rglob("*"):
        if f.is_file() and f.suffix in valid_exts and f.name != "SHA256SUMS.txt":
            rel = f.relative_to(directory).as_posix()
            sha = sha256_file(f)
            artifacts.append((sha, rel, f.stat().st_size / (1024 * 1024)))

    if not artifacts:
        print(f"  [INFO] No release artifacts found in {directory.relative_to(ROOT)}.")
        return None

    manifest = directory / "SHA256SUMS.txt"
    lines = [f"{sha}  {rel}" for sha, rel, _ in sorted(artifacts, key=lambda x: x[1])]
    manifest.write_text("\n".join(lines) + "\n", encoding="utf-8")

    print("\n========================================================")
    print(f"  SHA-256 MANIFEST: {manifest.relative_to(ROOT)}")
    print("========================================================")
    for sha, rel, size_mb in sorted(artifacts, key=lambda x: x[1]):
        print(f"  {sha}  {rel:<42} ({size_mb:.2f} MB)")
    print(f"\n  [OK] Saved manifest to: {manifest}")
    return manifest


def build_release(version: str) -> bool:
    """Build everything directly into dist/release/ for GitHub release publishing."""
    print("\n########################################################")
    print(f"   STARTING FULL RELEASE BUILD (v{version})")
    print("   Output Destination: dist/release/")
    print("########################################################")
    ensure_dirs()
    ok = True

    # 1. Android Client Release APK
    apk = build_android(version, out_dir=DIST_RELEASE)
    ok = (apk is not None) and ok

    # 2. Windows Installer & Standalone Executable
    win_files = build_windows(version, out_dir=DIST_RELEASE, mode="all")
    ok = (len(win_files) > 0) and ok

    # 3. Linux Package
    linux_pkg = build_linux(version, out_dir=DIST_RELEASE)
    ok = (linux_pkg is not None) and ok

    # 4. Generate SHA256SUMS.txt inside dist/release/
    generate_checksums(directory=DIST_RELEASE)

    print("\n========================================================")
    print("  [COMPLETE] FULL RELEASE BUNDLE READY IN dist/release/")
    print("========================================================")
    print("  You can now drag-and-drop the contents of dist/release/ directly to GitHub Releases:")
    for f in sorted(DIST_RELEASE.iterdir()):
        if f.is_file():
            print(f"    - {f.name:<40} ({f.stat().st_size / (1024 * 1024):.2f} MB)")
    return ok


def build_all(version: str) -> bool:
    """Build all platform targets into their respective platform folders."""
    ensure_dirs()
    ok = True
    ok = (build_android(version, out_dir=DIST_ANDROID) is not None) and ok
    ok = (len(build_windows(version, out_dir=DIST_WINDOWS, mode="all")) > 0) and ok
    ok = (build_linux(version, out_dir=DIST_LINUX) is not None) and ok
    generate_checksums(directory=DIST)
    return ok


def build_pc(version: str) -> bool:
    """Build PC Desktop targets only (Windows + Linux)."""
    ensure_dirs()
    ok = True
    ok = (len(build_windows(version, out_dir=DIST_WINDOWS, mode="all")) > 0) and ok
    ok = (build_linux(version, out_dir=DIST_LINUX) is not None) and ok
    generate_checksums(directory=DIST)
    return ok


def clean_dist():
    """Wipe build outputs in dist/ for a fresh clean state."""
    print("  Cleaning dist/ directory ...")
    if DIST.exists():
        for item in DIST.iterdir():
            if item.is_file():
                item.unlink(missing_ok=True)
            elif item.is_dir():
                shutil.rmtree(item, ignore_errors=True)
    ensure_dirs()
    print("  [OK] dist/ cleaned.")


def verify_repository() -> bool:
    """Run repository invariant checker."""
    print("\n========================================================")
    print("  RUNNING REPOSITORY INVARIANTS CHECK")
    print("========================================================")
    res = subprocess.run([sys.executable, str(ROOT / "scripts" / "verify-repo-invariants.py")], cwd=ROOT)
    return res.returncode == 0


# =============================================================================
# INTERACTIVE MENU & CLI
# =============================================================================

def dependencies_menu():
    """Interactive management for packaging dependencies."""
    while True:
        print("""
----------------------------------------------------------------------
             WINDOWS PACKAGING DEPENDENCIES MANAGER
----------------------------------------------------------------------
  [1] Audit Dependencies  -> Check local files, hashes, and URLs
  [2] Prefetch to Local   -> Download and verify all dependencies to local folders
  [3] Update Remote Link  -> Update URL and checksum in dependencies.json
  [0] Return to Main Menu
----------------------------------------------------------------------""")
        try:
            choice = input("Select an option [0-3]: ").strip()
        except (KeyboardInterrupt, EOFError):
            break

        if choice == "1":
            check_dependencies()
        elif choice == "2":
            ensure_windows_prerequisites(force_refresh=True)
        elif choice == "3":
            comp = input("Dependency identifier (vbcable or softcam): ").strip()
            url = input("New download URL: ").strip()
            h = input("Expected SHA-256 (press Enter to auto-calculate): ").strip() or None
            v = input("Version string (optional, press Enter to keep): ").strip() or None
            if comp and url:
                update_dependency_link(comp, url, h, v)
        elif choice == "0":
            break


def interactive_menu():
    """Clean, focused release management console."""
    version = get_version()
    ensure_dirs()
    host_os = platform.system()

    while True:
        menu = f"""
======================================================================
                     OWLMIC RELEASE BUILDER (v{version})
======================================================================
  Host Platform: {host_os} ({platform.machine()})

  [1] Release Build      -> Full Release Bundle (All Platforms -> dist/release/)
  [2] Build All          -> All Platforms into Platform Folders (dist/android, windows, linux)
  [3] Desktop Build        -> Windows + Linux Only (dist/windows/, dist/linux/)
  [4] Android Client Only-> APK -> dist/android/Owlmic-v{version}.apk
  [5] Windows Build Only -> Installer + Portable -> dist/windows/
  [6] Linux Build Only   -> Portable Archive -> dist/linux/

  --- Dependency & Packaging Maintenance ---
  [7] Manage Dependencies-> Audit, prefetch, or update VB-CABLE and softcam
  [8] Verify Invariants  -> Check repo casing, binary ban, and JSON schemas
  [9] Clean dist/        -> Reset all build outputs

  [0] Exit
----------------------------------------------------------------------
"""
        print(menu)
        try:
            choice = input("Select an option [0-9]: ").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nExiting.")
            sys.exit(0)

        if choice == "1":
            build_release(version)
        elif choice == "2":
            build_all(version)
        elif choice == "3":
            build_pc(version)
        elif choice == "4":
            build_android(version, out_dir=DIST_ANDROID)
        elif choice == "5":
            build_windows(version, out_dir=DIST_WINDOWS, mode="all")
        elif choice == "6":
            build_linux(version, out_dir=DIST_LINUX)
        elif choice == "7":
            dependencies_menu()
        elif choice == "8":
            verify_repository()
        elif choice == "9":
            clean_dist()
        elif choice == "0":
            print("Exiting.")
            sys.exit(0)
        else:
            print("Invalid selection, please try again.")


def main():
    parser = argparse.ArgumentParser(description="Owlmic release packaging tool")
    parser.add_argument(
        "--target",
        choices=["release", "all", "desktop", "pc", "android", "windows", "linux"],
        help="Target suite to build non-interactively",
    )
    parser.add_argument("--clean", action="store_true", help="Clean dist/ directory")
    parser.add_argument("--verify", action="store_true", help="Run repo invariants check")
    parser.add_argument("--check-deps", action="store_true", help="Audit local packaging dependencies, hashes and URLs")
    parser.add_argument("--fetch-deps", action="store_true", help="Download/refresh all dependencies into local folders")
    parser.add_argument("--update-dep", metavar="NAME", help="Update dependency link (vbcable or softcam)")
    parser.add_argument("--url", metavar="URL", help="New download URL for --update-dep")
    parser.add_argument("--sha256", metavar="HASH", help="Expected SHA-256 for --update-dep (optional)")
    parser.add_argument("--dep-version", metavar="VER", help="Version string for --update-dep (optional)")
    args = parser.parse_args()

    version = get_version()
    ensure_dirs()

    if args.clean:
        clean_dist()

    if args.verify:
        if not verify_repository():
            sys.exit(1)

    if args.check_deps:
        ok = check_dependencies()
        sys.exit(0 if ok else 1)

    if args.fetch_deps:
        ok = ensure_windows_prerequisites(force_refresh=True)
        sys.exit(0 if ok else 1)

    if args.update_dep:
        if not args.url:
            print("  [ERROR] --url is required when using --update-dep.")
            sys.exit(1)
        ok = update_dependency_link(args.update_dep, args.url, args.sha256, args.dep_version)
        sys.exit(0 if ok else 1)

    if args.target:
        ok = True
        if args.target == "release":
            ok = build_release(version)
        elif args.target == "all":
            ok = build_all(version)
        elif args.target in ("desktop", "pc"):
            ok = build_pc(version)
        elif args.target == "android":
            ok = (build_android(version, out_dir=DIST_ANDROID) is not None)
        elif args.target == "windows":
            ok = (len(build_windows(version, out_dir=DIST_WINDOWS, mode="all")) > 0)
        elif args.target == "linux":
            ok = (build_linux(version, out_dir=DIST_LINUX) is not None)
        sys.exit(0 if ok else 1)
    elif not (args.clean or args.verify):
        interactive_menu()


if __name__ == "__main__":
    main()
