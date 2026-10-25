#!/bin/bash
# SPDX-License-Identifier: GPL-2.0-only
# Builds the community AnyPS5-ARM Rosetta port (main branch) on Apple Silicon so we can
# see how a title you legally own behaves today. UNTESTED by the ArmStation authors:
# steps follow that project's docs/dev/MACOS_PORT.md. Writes a log you can paste back.
# Usage: scripts/try-anyps5-mac.sh [workdir]   (default: ~/armstation-work)
set -uo pipefail
W="${1:-$HOME/armstation-work}"; mkdir -p "$W"; LOG="$W/try-anyps5.log"
exec > >(tee "$LOG") 2>&1
echo "== $(date) =="; uname -m; sw_vers
need() { command -v "$1" >/dev/null || { echo "MISSING: $1  ->  $2"; MISSING=1; }; }
MISSING=0
need git "xcode-select --install"; need cmake "brew install cmake"; need ninja "brew install ninja"; need python3 "xcode-select --install"
/usr/bin/pgrep -q oahd || echo "NOTE: Rosetta not running yet; if the build fails with 'bad CPU type', run: softwareupdate --install-rosetta --agree-to-license"
VK=$(ls -d "$HOME"/VulkanSDK/*/macOS 2>/dev/null | tail -1)
[ -n "$VK" ] && echo "Vulkan SDK: $VK" || { echo "MISSING: Vulkan SDK -> install the macOS SDK from https://vulkan.lunarg.com/sdk/home#mac (GUI installer), then rerun"; MISSING=1; }
[ "$MISSING" = 1 ] && { echo "Fix the MISSING items above and rerun."; exit 1; }
cd "$W"
[ -d AnyPS5-ARM ] || git clone --depth 1 --recurse-submodules --shallow-submodules https://github.com/utkuhalis/AnyPS5-ARM || exit 1
cd AnyPS5-ARM && git rev-parse HEAD
cmake -S . -B build-mac -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES=x86_64 -DBUILD_TESTING=OFF || exit 1
time ninja -C build-mac all libs || { echo "BUILD FAILED"; exit 1; }
ls -la build-mac/core/relinker/relinker build-mac/core/libs/libs | head
echo "Build OK. Next (with your own decrypted ELF at ~/ps5/source/eboot.bin):"
echo "  build-mac/core/relinker/relinker --macos --to-rosetta ~/ps5/source/eboot.bin ~/ps5/out/eboot"
echo "  python3 tools/package_macos_app.py --relinked ~/ps5/out --game ~/ps5/source --libs build-mac/core/libs/libs --vulkan $VK Title.app"
