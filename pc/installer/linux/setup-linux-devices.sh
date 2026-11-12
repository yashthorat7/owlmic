#!/usr/bin/env bash
set -euo pipefail

# Setup script for Owlmic Linux virtual camera and audio prerequisites.

echo "=== Setting up Owlmic Linux Prerequisites ==="

# 1. Check PipeWire / PulseAudio
if command -v pw-cli >/dev/null 2>&1; then
    echo "✅ PipeWire detected."
elif command -v pactl >/dev/null 2>&1; then
    echo "✅ PulseAudio detected."
else
    echo "⚠️ Warning: Neither PipeWire nor PulseAudio found. Audio streaming requires a sound server."
fi

# 2. Check and configure v4l2loopback for virtual camera
if lsmod | grep -q v4l2loopback; then
    echo "✅ v4l2loopback kernel module already loaded."
else
    echo "→ Loading v4l2loopback kernel module for Owlmic Cam..."
    if command -v modprobe >/dev/null 2>&1; then
        sudo modprobe v4l2loopback devices=1 video_nr=10 card_label="Owlmic Cam" exclusive_caps=1 || {
            echo "⚠️ Could not load v4l2loopback. Install it with: sudo apt install v4l2loopback-dkms"
        }
    fi
fi

echo "=== Setup complete ==="
