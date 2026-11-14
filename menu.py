#!/usr/bin/env python3
"""Owlmic Unified Developer & Release Menu.

Single entry-point for building, testing, linting, invariant verification,
and repository maintenance across Android, Windows, and Linux.
"""

import os
import platform
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SCRIPTS = ROOT / "scripts"

GREEN = "\033[92m"
CYAN = "\033[96m"
YELLOW = "\033[93m"
RED = "\033[91m"
RESET = "\033[0m"


def get_version() -> str:
    cargo_toml = ROOT / "desktop" / "Cargo.toml"
    if cargo_toml.exists():
        m = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml.read_text(encoding="utf-8"), re.M)
        if m:
            return m.group(1)
    return "0.1.0"


def run_script(script_name: str, args: list[str] | None = None) -> int:
    target = SCRIPTS / script_name
    if not target.exists():
        print(f"{RED}❌ Script not found: {target}{RESET}")
        return 1
    cmd = [sys.executable, str(target)] + (args or [])
    res = subprocess.run(cmd, cwd=ROOT)
    return res.returncode


def print_banner(version: str):
    host = f"{platform.system()} ({platform.machine()})"
    print(f"""
{CYAN}======================================================================
                         OWLMIC TOOLKIT MENU
======================================================================{RESET}
  Host Platform: {host}
  Version:       {version}

  {YELLOW}[ Build & Package ]{RESET}
  [1] Release Bundle       -> Full release bundle (all platforms -> dist/release/)
  [2] Android Client APK   -> APK (dist/android/Owlmic-v{version}.apk)
  [3] Windows Desktop      -> Installer + Portable (dist/windows/)
  [4] Linux Desktop        -> Portable Tarball (dist/linux/)
  [5] Clean dist/          -> Reset all build outputs

  {YELLOW}[ Verification & Quality ]{RESET}
  [6] Full Pre-Flight      -> Invariants + Rust + Android tests & lint
  [7] Repo Invariants Only -> Naming, binary ban, tokio, JSON, versions
  [8] Rust Workspace Only  -> fmt, clippy, tests (desktop/)
  [9] Android Client Only  -> unit tests + lint (android/)
  [10] Format Rust Code    -> cargo fmt --all (fixes formatting)

  {YELLOW}[ Repository Setup ]{RESET}
  [11] Setup Git Hooks     -> Activate core.hooksPath -> scripts/hooks

  {YELLOW}[ Packaging Dependencies & Setup ]{RESET}
  [12] Manage Dependencies -> Audit, prefetch, or update VB-CABLE and softcam

  [0] Exit
----------------------------------------------------------------------""")


def interactive_menu():
    version = get_version()
    while True:
        print_banner(version)
        try:
            choice = input("Select an option [0-12]: ").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nExiting.")
            sys.exit(0)

        if choice == "1":
            run_script("build.py", ["--target", "release"])
        elif choice == "2":
            run_script("build.py", ["--target", "android"])
        elif choice == "3":
            run_script("build.py", ["--target", "windows"])
        elif choice == "4":
            run_script("build.py", ["--target", "linux"])
        elif choice == "5":
            run_script("build.py", ["--clean"])
        elif choice == "6":
            run_script("check.py")
        elif choice == "7":
            run_script("verify-repo-invariants.py")
        elif choice == "8":
            run_script("check.py", ["--skip-invariants", "--skip-android"])
        elif choice == "9":
            run_script("check.py", ["--skip-invariants", "--skip-rust"])
        elif choice == "10":
            run_script("check.py", ["--skip-invariants", "--skip-android", "--fix"])
        elif choice == "11":
            run_script("setup-hooks.py")
        elif choice == "12":
            print("\n  [1] Check / Audit Dependencies Status")
            print("  [2] Prefetch / Refresh All Dependencies to Local Folders")
            print("  [3] Update Remote Link / Build URL")
            print("  [0] Back to Menu")
            try:
                sub = input("Select [0-3]: ").strip()
                if sub == "1":
                    run_script("build.py", ["--check-deps"])
                elif sub == "2":
                    run_script("build.py", ["--fetch-deps"])
                elif sub == "3":
                    comp = input("Dependency name (vbcable or softcam): ").strip()
                    url = input("New download URL: ").strip()
                    h = input("Expected SHA-256 (optional, press Enter to auto-calculate): ").strip() or None
                    args = ["--update-dep", comp, "--url", url]
                    if h:
                        args.extend(["--sha256", h])
                    run_script("build.py", args)
            except (KeyboardInterrupt, EOFError):
                pass
        elif choice == "0":
            print("Exiting.")
            sys.exit(0)
        else:
            print(f"{RED}Invalid selection. Please choose an option from 0 to 12.{RESET}")

        try:
            input("\nPress Enter to return to menu...")
        except (KeyboardInterrupt, EOFError):
            print("\nExiting.")
            sys.exit(0)


def print_help():
    print("""Owlmic Unified Developer CLI

Usage:
  python menu.py [command] [options]

Commands:
  build [release|all|desktop|android|windows|linux]  Build target suite
  clean                                         Clean dist/ directory
  check [--fix]                                 Run pre-flight checks
  verify                                        Verify repo invariants
  deps [--check|--fetch]                        Audit or prefetch packaging dependencies
  hooks                                         Configure git hooks
  help, -h, --help                              Show this help message

Run without arguments to launch the interactive menu:
  python menu.py
""")


def main():
    if len(sys.argv) > 1:
        cmd = sys.argv[1].lower()
        if cmd in {"-h", "--help", "help"}:
            print_help()
            sys.exit(0)
        elif cmd == "build":
            target = sys.argv[2] if len(sys.argv) > 2 else "release"
            sys.exit(run_script("build.py", ["--target", target]))
        elif cmd == "clean":
            sys.exit(run_script("build.py", ["--clean"]))
        elif cmd == "check":
            extra = sys.argv[2:]
            sys.exit(run_script("check.py", extra))
        elif cmd == "verify":
            sys.exit(run_script("verify-repo-invariants.py"))
        elif cmd == "deps":
            flag = sys.argv[2] if len(sys.argv) > 2 else "--check-deps"
            if flag == "--fetch":
                flag = "--fetch-deps"
            elif flag == "--check":
                flag = "--check-deps"
            sys.exit(run_script("build.py", [flag]))
        elif cmd == "hooks":
            sys.exit(run_script("setup-hooks.py"))
        else:
            print(f"Unknown command: {cmd}")
            print_help()
            sys.exit(1)

    interactive_menu()


if __name__ == "__main__":
    main()
