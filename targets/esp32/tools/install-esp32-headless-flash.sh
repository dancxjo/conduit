#!/bin/sh
set -eu

fail() {
    printf 'install-esp32-headless-flash: %s\n' "$*" >&2
    exit 1
}

[ "$(id -u)" -eq 0 ] || fail "run this installer with sudo"

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
source_udev_rule="$script_dir/70-conduit-esp32.rules"
target_udev_rule=/etc/udev/rules.d/70-conduit-esp32.rules

[ -f "$source_udev_rule" ] || fail "missing udev rule at $source_udev_rule"
command -v udevadm >/dev/null || fail "udevadm is required"
getent group plugdev >/dev/null || fail "the plugdev group is required"

install -o root -g root -m 0644 "$source_udev_rule" "$target_udev_rule"
udevadm control --reload-rules

printf 'Installed persistent plugdev access for the inspected Conduit ESP32 adapter.\n'
printf 'Reconnect the ESP32 once to apply the serial access rule.\n'
