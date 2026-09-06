#!/usr/bin/env bash
set -euo pipefail

runtime_dir="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
credential_file="$runtime_dir/conduit-wifi.env"
completed=false
wifi_ssid=''
wifi_credential=''
active_ssid=''

cleanup() {
    wifi_ssid=''
    wifi_credential=''
    active_ssid=''
    if [[ "$completed" != true ]]; then
        rm -f -- "$credential_file"
    fi
}
trap cleanup EXIT HUP INT TERM

if [[ ! -d "$runtime_dir" || ! -O "$runtime_dir" ]]; then
    printf 'Unsafe or unavailable per-user runtime directory: %s\n' "$runtime_dir" >&2
    exit 1
fi
if [[ ! -r /dev/tty || ! -w /dev/tty ]]; then
    printf 'This script must be run from an interactive terminal.\n' >&2
    exit 1
fi

umask 077
active_ssid=''
if command -v nmcli >/dev/null 2>&1; then
    active_ssid=$(nmcli -t -f active,ssid dev wifi list 2>/dev/null | sed -n 's/^yes://p' | head -n 1)
fi
if [[ -n "$active_ssid" ]]; then
    read -r -p "Wi-Fi SSID [$active_ssid]: " wifi_ssid </dev/tty
    wifi_ssid="${wifi_ssid:-$active_ssid}"
else
    read -r -p 'Wi-Fi SSID: ' wifi_ssid </dev/tty
fi
if [[ -z "$wifi_ssid" ]]; then
    printf 'Wi-Fi SSID must not be empty.\n' >&2
    exit 1
fi
read -r -s -p 'Wi-Fi credential: ' wifi_credential </dev/tty
printf '\n' >/dev/tty

temporary_file=$(mktemp "$runtime_dir/conduit-wifi.env.XXXXXX")
printf 'export CONDUIT_WIFI_SSID=%q\n' "$wifi_ssid" >"$temporary_file"
printf 'export CONDUIT_WIFI_CREDENTIAL=%q\n' "$wifi_credential" >>"$temporary_file"
chmod 0600 "$temporary_file"
mv -f -- "$temporary_file" "$credential_file"
completed=true

printf 'Credentials are ready at %s (mode 0600).\n' "$credential_file"
