#!/usr/bin/env python3
"""Owlmic Unified Pre-Flight Verification Runner.

Runs repository invariant validation, Rust workspace checks (fmt, clippy, test),
and Android client tests across all supported developer platforms.
"""

import argparse
import os
import platform
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# ANSI color codes with Windows terminal compatibility
GREEN = "\033[92m"
CYAN = "\033[96m"
YELLOW = "\033[93m"
RED = "\033[91m"
RESET = "\033[0m"


def log_step(step: str, title: str):
    print(f"\n{CYAN}=== [{step}] {title} ==={RESET}")


def log_success(msg: str):
    print(f"{GREEN}✅ {msg}{RESET}")


def log_warn(msg: str):
    print(f"{YELLOW}⚠️ {msg}{RESET}")


def log_error(msg: str):
    print(f"{RED}❌ {msg}{RESET}")


def run_cmd(cmd: list[str], cwd: Path | None = None) -> int:
    display_cmd = " ".join(cmd)
    print(f"  -> {display_cmd}")
    res = subprocess.run(cmd, cwd=cwd or ROOT)
    return res.returncode


def check_invariants() -> bool:
    log_step("1/4", "Verifying Owlmic Repo Invariants")
    verifier = ROOT / "scripts" / "verify-repo-invariants.py"
    if not verifier.exists():
        log_error(f"Invariant verifier not found: {verifier}")
        return False
    code = run_cmd([sys.executable, str(verifier)], cwd=ROOT)
    return code == 0


def check_rust(fix: bool = False) -> bool:
    log_step("2/4", "Checking Rust Workspace (desktop/)")
    desktop_dir = ROOT / "desktop"
    if not desktop_dir.exists():
        log_warn("desktop/ directory not found, skipping Rust checks.")
        return True

    # 1. Formatting
    if fix:
        print("  Running cargo fmt (fixing formatting)...")
        if run_cmd(["cargo", "fmt", "--all"], cwd=desktop_dir) != 0:
            return False
    else:
        if run_cmd(["cargo", "fmt", "--all", "--check"], cwd=desktop_dir) != 0:
            log_error("Rust formatting check failed. Run 'python scripts/check.py --fix' to format.")
            return False

    # 2. Clippy
    if run_cmd(["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], cwd=desktop_dir) != 0:
        log_error("Cargo clippy linting failed.")
        return False

    # 3. Tests
    if run_cmd(["cargo", "test", "--workspace"], cwd=desktop_dir) != 0:
        log_error("Cargo tests failed.")
        return False

    log_success("Rust workspace verification passed.")
    return True


def check_android() -> bool:
    log_step("3/4", "Checking Android Client (android/)")
    android_dir = ROOT / "android"
    if not android_dir.exists():
        log_warn("android/ directory not found, skipping Android checks.")
        return True

    is_windows = platform.system() == "Windows"
    gradlew_name = "gradlew.bat" if is_windows else "gradlew"
    gradlew = android_dir / gradlew_name

    if not gradlew.exists():
        log_warn(f"Gradle wrapper ({gradlew_name}) not found, skipping Android checks.")
        return True

    cmd = [str(gradlew), "testDebugUnitTest", "lintDebug"]
    code = run_cmd(cmd, cwd=android_dir)
    if code != 0:
        log_error("Android tests / lint checks failed.")
        return False

    log_success("Android client verification passed.")
    return True


def main():
    parser = argparse.ArgumentParser(description="Owlmic Master Pre-Flight Checker")
    parser.add_argument("--skip-invariants", action="store_true", help="Skip repository invariant checks")
    parser.add_argument("--skip-rust", action="store_true", help="Skip Rust checks")
    parser.add_argument("--skip-android", action="store_true", help="Skip Android checks")
    parser.add_argument("--fix", action="store_true", help="Automatically format code where possible")
    args = parser.parse_args()

    # Step 1: Invariants
    if not args.skip_invariants:
        if not check_invariants():
            log_error("Repo invariant check failed.")
            sys.exit(1)

    # Step 2: Rust
    if not args.skip_rust:
        if not check_rust(fix=args.fix):
            log_error("Rust verification failed.")
            sys.exit(1)

    # Step 3: Android
    if not args.skip_android:
        if not check_android():
            log_error("Android verification failed.")
            sys.exit(1)

    # Step 4: Done
    log_step("4/4", "Verification Complete")
    log_success("All Owlmic Checks Passed!\n")


if __name__ == "__main__":
    main()
