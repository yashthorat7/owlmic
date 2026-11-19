#!/usr/bin/env python3
"""Owlmic Unified Pre-Flight Verification Runner.

Runs repository invariant validation, Rust workspace checks (fmt, clippy, test),
and Android client tests across all supported developer platforms with compact,
legible, high-signal output.
"""

import argparse
import os
import platform
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent

# Enable ANSI virtual terminal processing on Windows if supported
if sys.platform == "win32":
    try:
        os.system("")
    except Exception:
        pass

# Check if color is disabled via standard NO_COLOR environment variable
if os.environ.get("NO_COLOR"):
    GREEN = CYAN = YELLOW = RED = BOLD = DIM = RESET = ""
else:
    GREEN = "\033[92m"
    CYAN = "\033[96m"
    YELLOW = "\033[93m"
    RED = "\033[91m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RESET = "\033[0m"


def print_banner(title: str = "Owlmic Pre-Flight Verification"):
    print(f"\n{CYAN}{BOLD}🦅 {title}{RESET}")
    print(f"{DIM}────────────────────────────────────────────────────────────{RESET}")


def format_row(label: str, status: str, detail: str = "", indent: int = 1, width: int = 44) -> str:
    prefix = " " * indent + label + " "
    dots_len = max(2, width - len(prefix))
    dots = f"{DIM}{'.' * dots_len}{RESET}"
    if status == "OK":
        badge = f"{GREEN}{BOLD}[ OK ]{RESET}"
    elif status == "FAIL":
        badge = f"{RED}{BOLD}[ FAIL ]{RESET}"
    elif status == "SKIP":
        badge = f"{YELLOW}{BOLD}[ SKIP ]{RESET}"
    elif status == "WARN":
        badge = f"{YELLOW}{BOLD}[ WARN ]{RESET}"
    elif status == "FIXED":
        badge = f"{GREEN}{BOLD}[ FIXED ]{RESET}"
    else:
        badge = f"[{status}]"

    det = f" {DIM}{detail}{RESET}" if detail else ""
    return f"{prefix}{dots} {badge}{det}"


def print_row(label: str, status: str, detail: str = "", indent: int = 1, width: int = 44):
    print(format_row(label, status, detail, indent, width), flush=True)


def print_error_box(title: str, content: str):
    print(f"\n{RED}{BOLD}❌ {title}{RESET}")
    print(f"{DIM}------------------------------------------------------------{RESET}")
    clean_content = content.strip() if content else ""
    if clean_content:
        print(clean_content)
    else:
        print("(No failure output recorded)")
    print(f"{DIM}------------------------------------------------------------{RESET}\n")


def check_invariants(verbose: bool = False) -> bool:
    verifier = ROOT / "scripts" / "core" / "verify-repo-invariants.py"
    if not verifier.exists():
        print_row("[1/3] Repository Invariants", "FAIL", "script not found", indent=1)
        print_error_box("Invariant Verifier Missing", f"Could not find: {verifier}")
        return False

    if verbose:
        print(f"  -> {sys.executable} {verifier}")
        code = subprocess.run([sys.executable, str(verifier)], cwd=ROOT).returncode
        return code == 0

    res = subprocess.run([sys.executable, str(verifier)], cwd=ROOT, capture_output=True, text=True)
    if res.returncode == 0:
        print_row("[1/3] Repository Invariants", "OK", "(casing, binary ban, tokio, JSON, versions)", indent=1)
        return True
    else:
        print_row("[1/3] Repository Invariants", "FAIL", "", indent=1)
        print_error_box("Repo Invariant Check FAILED", (res.stdout + "\n" + res.stderr))
        return False


def check_rust(fix: bool = False, verbose: bool = False) -> bool:
    print(" [2/3] Rust Workspace (desktop/)")
    desktop_dir = ROOT / "desktop"
    if not desktop_dir.exists():
        desktop_dir = ROOT / "pc"
    if not desktop_dir.exists():
        print_row("• Workspace Check", "SKIP", "desktop/ directory not found", indent=7)
        return True

    # 1. Formatting
    if fix:
        if verbose:
            print("  -> cargo fmt --all")
            code = subprocess.run(["cargo", "fmt", "--all"], cwd=desktop_dir).returncode
            if code != 0:
                return False
        else:
            res = subprocess.run(["cargo", "fmt", "--all"], cwd=desktop_dir, capture_output=True, text=True)
            if res.returncode != 0:
                print_row("• Formatting (cargo fmt)", "FAIL", "", indent=7)
                print_error_box("cargo fmt --all failed", res.stderr or res.stdout)
                return False
        print_row("• Formatting (cargo fmt)", "FIXED", "(auto-formatted)", indent=7)
    else:
        if verbose:
            print("  -> cargo fmt --all --check")
            code = subprocess.run(["cargo", "fmt", "--all", "--check"], cwd=desktop_dir).returncode
            if code != 0:
                return False
        else:
            res = subprocess.run(["cargo", "fmt", "--all", "--check"], cwd=desktop_dir, capture_output=True, text=True)
            if res.returncode != 0:
                print_row("• Formatting (cargo fmt)", "FAIL", "", indent=7)
                print_error_box("Rust formatting check failed. Run 'python scripts/menu.py check --fix' to format.", res.stderr or res.stdout)
                return False
            print_row("• Formatting (cargo fmt)", "OK", "", indent=7)

    # 2. Clippy
    if verbose:
        print("  -> cargo clippy --workspace --all-targets -- -D warnings")
        code = subprocess.run(["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], cwd=desktop_dir).returncode
        if code != 0:
            return False
    else:
        res = subprocess.run(["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"], cwd=desktop_dir, capture_output=True, text=True)
        if res.returncode != 0:
            print_row("• Linter (cargo clippy)", "FAIL", "", indent=7)
            print_error_box("Cargo Clippy Lints Failed", res.stderr or res.stdout)
            return False
        print_row("• Linter (cargo clippy)", "OK", "(workspace clean)", indent=7)

    # 3. Tests
    if verbose:
        print("  -> cargo test --workspace")
        code = subprocess.run(["cargo", "test", "--workspace"], cwd=desktop_dir).returncode
        return code == 0

    res = subprocess.run(["cargo", "test", "--workspace"], cwd=desktop_dir, capture_output=True, text=True)
    if res.returncode != 0:
        print_row("• Tests (cargo test)", "FAIL", "", indent=7)
        fail_out = res.stdout if "FAILED" in res.stdout else (res.stderr + "\n" + res.stdout)
        print_error_box("Cargo Tests Failed", fail_out)
        return False

    results = re.findall(r"test result:\s*(\w+)\.\s*(\d+)\s+passed;\s*(\d+)\s+failed;\s*(\d+)\s+ignored", res.stdout)
    total_passed = sum(int(r[1]) for r in results)
    total_ignored = sum(int(r[3]) for r in results)
    suites = sum(1 for r in results if int(r[1]) + int(r[2]) + int(r[3]) > 0)

    detail = f"({total_passed} passed"
    if total_ignored > 0:
        detail += f", {total_ignored} ignored"
    if suites > 0:
        detail += f" across {suites} test suites"
    detail += ")"

    print_row("• Tests (cargo test)", "OK", detail, indent=7)
    return True


def check_android(verbose: bool = False) -> bool:
    print(" [3/3] Android Client (android/)")
    android_dir = ROOT / "android"
    if not android_dir.exists():
        print_row("• Unit Tests & Lint", "SKIP", "android/ directory not found", indent=7)
        return True

    is_windows = platform.system() == "Windows"
    gradlew_name = "gradlew.bat" if is_windows else "gradlew"
    gradlew = android_dir / gradlew_name

    if not gradlew.exists():
        print_row("• Unit Tests & Lint", "SKIP", f"{gradlew_name} not found", indent=7)
        return True

    cmd = [str(gradlew), "testDebugUnitTest", "lintDebug"]

    if verbose:
        print(f"  -> {' '.join(cmd)}")
        code = subprocess.run(cmd, cwd=android_dir, shell=is_windows).returncode
        return code == 0

    res = subprocess.run(cmd, cwd=android_dir, capture_output=True, text=True, shell=is_windows)
    if res.returncode != 0:
        print_row("• Unit Tests & Lint (Debug)", "FAIL", "", indent=7)
        print_error_box("Android Tests / Lint Failed", res.stdout + "\n" + res.stderr)
        return False

    m_build = re.search(r"BUILD SUCCESSFUL in ([^\n\r]+)", res.stdout)
    m_tasks = re.search(r"(\d+)\s+actionable tasks", res.stdout)
    build_time = m_build.group(1).strip() if m_build else ""
    task_count = m_tasks.group(1).strip() if m_tasks else ""

    parts = []
    if task_count:
        parts.append(f"{task_count} tasks verified")
    else:
        parts.append("build successful")
    if build_time:
        parts.append(build_time)
    detail = f"({', '.join(parts)})"

    print_row("• Unit Tests & Lint (Debug)", "OK", detail, indent=7)
    return True


def main():
    start_time = time.time()
    parser = argparse.ArgumentParser(description="Owlmic Master Pre-Flight Checker")
    parser.add_argument("--skip-invariants", action="store_true", help="Skip repository invariant checks")
    parser.add_argument("--skip-rust", action="store_true", help="Skip Rust checks")
    parser.add_argument("--skip-android", action="store_true", help="Skip Android checks")
    parser.add_argument("--fix", action="store_true", help="Automatically format code where possible")
    parser.add_argument("--pre-push", action="store_true", help="Called from git pre-push hook")
    parser.add_argument("-v", "--verbose", action="store_true", help="Display verbose tool output")
    args = parser.parse_args()

    is_pre_push = args.pre_push or os.environ.get("OWLMIC_PRE_PUSH") == "1"
    banner_title = "Owlmic Pre-Push Verification" if is_pre_push else "Owlmic Pre-Flight Verification"
    print_banner(banner_title)

    has_skips = False

    # Step 1: Invariants
    if not args.skip_invariants:
        if not check_invariants(verbose=args.verbose):
            sys.exit(1)
    else:
        has_skips = True
        print_row("[1/3] Repository Invariants", "SKIP", "(skipped by flag)", indent=1)

    # Step 2: Rust
    if not args.skip_rust:
        if not check_rust(fix=args.fix, verbose=args.verbose):
            sys.exit(1)
    else:
        has_skips = True
        print(" [2/3] Rust Workspace (desktop/)")
        print_row("• Workspace Checks", "SKIP", "(skipped by flag)", indent=7)

    # Step 3: Android
    if not args.skip_android:
        if not check_android(verbose=args.verbose):
            sys.exit(1)
    else:
        has_skips = True
        print(" [3/3] Android Client (android/)")
        print_row("• Unit Tests & Lint", "SKIP", "(skipped by flag)", indent=7)

    elapsed = time.time() - start_time
    print(f"{DIM}────────────────────────────────────────────────────────────{RESET}")
    if has_skips:
        print(f"{GREEN}{BOLD}✨ Owlmic verification completed with skips.{RESET} {DIM}(Total: {elapsed:.1f}s){RESET}\n")
    else:
        print(f"{GREEN}{BOLD}✨ All Owlmic checks passed!{RESET} {DIM}(Total: {elapsed:.1f}s){RESET}\n")


if __name__ == "__main__":
    main()
