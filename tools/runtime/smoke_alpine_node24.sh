#!/usr/bin/env bash
# Executes Node from the packaged rootfs, including cross-architecture ARM64 roots.
set -euo pipefail
archive="$(realpath "$1")"
architecture="$2"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(mktemp -d)"
trap 'sudo umount "$root/dev" 2>/dev/null || true; sudo rm -rf "$root"' EXIT
sudo tar -xzf "$archive" -C "$root"
python3 "$script_dir/verify_alpine_node24.py" "$root/lib/apk/db/installed"
sudo mkdir -p "$root/tmp" "$root/dev"
sudo mount --bind /dev "$root/dev"
sudo cp "$script_dir/node24_smoke.cjs" "$root/tmp/node24_smoke.cjs"
case "$architecture" in
    x86)
        sudo chroot "$root" /usr/bin/node /tmp/node24_smoke.cjs
        ;;
    aarch64)
        sudo cp /usr/bin/qemu-aarch64-static "$root/tmp/qemu-aarch64-static"
        sudo chroot "$root" /tmp/qemu-aarch64-static /usr/bin/node /tmp/node24_smoke.cjs
        ;;
    *) echo "Unsupported runtime architecture: $architecture" >&2; exit 1 ;;
esac
