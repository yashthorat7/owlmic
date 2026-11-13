#!/usr/bin/env python3
"""Owlmic Git Hooks Setup.

Configures git core.hooksPath to scripts/hooks and sets executable permissions on POSIX systems.
"""

import os
import platform
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    try:
        subprocess.run(["git", "config", "core.hooksPath", "scripts/hooks"], cwd=ROOT, check=True)
    except Exception as e:
        print(f"❌ Failed to configure git hooks path: {e}")
        sys.exit(1)

    # Set executable bit on POSIX systems (Linux, macOS, WSL)
    if platform.system() != "Windows":
        hooks_dir = ROOT / "scripts" / "hooks"
        if hooks_dir.exists():
            for hook in hooks_dir.iterdir():
                if hook.is_file():
                    try:
                        hook.chmod(hook.stat().st_mode | 0o755)
                    except Exception:
                        pass

    print("✅ Owlmic Git hooks activated (core.hooksPath -> scripts/hooks).")


if __name__ == "__main__":
    main()
