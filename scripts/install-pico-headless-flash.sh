#!/bin/sh
set -eu

fail() {
    printf 'install-pico-headless-flash: %s\n' "$*" >&2
    exit 1
}

[ "$(id -u)" -eq 0 ] || fail "run this installer with sudo"

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
source_helper="$script_dir/conduit-pico-headless-mount"
source_udev_rule="$script_dir/70-conduit-pico.rules"
target_helper=/usr/local/libexec/conduit-pico-headless-mount
target_rule=/etc/sudoers.d/conduit-pico-headless-flash
target_udev_rule=/etc/udev/rules.d/70-conduit-pico.rules

[ -f "$source_helper" ] || fail "missing helper at $source_helper"
[ -f "$source_udev_rule" ] || fail "missing udev rule at $source_udev_rule"
command -v lsblk >/dev/null || fail "lsblk is required"
command -v findmnt >/dev/null || fail "findmnt is required"
command -v mount >/dev/null || fail "mount is required"
command -v umount >/dev/null || fail "umount is required"
command -v udevadm >/dev/null || fail "udevadm is required"
command -v visudo >/dev/null || fail "visudo is required"
getent group plugdev >/dev/null || fail "the plugdev group is required"

install -d -o root -g root -m 0755 /usr/local/libexec
install -o root -g root -m 0755 "$source_helper" "$target_helper"

temporary_rule=$(mktemp)
trap 'rm -f "$temporary_rule"' EXIT HUP INT TERM
printf '%%plugdev ALL=(root) NOPASSWD: %s "", %s --unmount\n' "$target_helper" "$target_helper" > "$temporary_rule"
chmod 0440 "$temporary_rule"
visudo -cf "$temporary_rule" >/dev/null
install -o root -g root -m 0440 "$temporary_rule" "$target_rule"
visudo -cf "$target_rule" >/dev/null
install -o root -g root -m 0644 "$source_udev_rule" "$target_udev_rule"
udevadm control --reload-rules

printf 'Installed narrow headless Pico BOOTSEL mount and exact-device reset support.\n'
printf 'Installed persistent plugdev access for the matching Conduit Pico USB device and CDC interfaces.\n'
printf 'Members of plugdev may run only the fixed mount helpers and access that exact USB identity.\n'
printf 'Reconnect the Pico once to apply USB access and modem-probe exclusion.\n'
