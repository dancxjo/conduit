#!/bin/sh
# One existing climate-test profile on one existing Wi-Fi device. No credentials
# are read, no profile is edited, and installation does not activate a network.
set -eu
mode=${1:---print}
profile=${2:-Conduit-Climate-test}
interface=${3:-}
case "$mode" in --print|--facts|--install) ;; *) echo 'expected --print, --facts or --install [profile [interface]]' >&2; exit 2;; esac
nmcli=/usr/bin/nmcli
[ -x "$nmcli" ] || { echo 'NetworkManager nmcli is required on this Linux host' >&2; exit 2; }
user_name=${SUDO_USER:-$(id -un)}
case "$user_name" in ''|root|-*|*[!a-zA-Z0-9_-]*) echo 'invoke setup from the non-root host account' >&2; exit 2;; esac
id -u "$user_name" >/dev/null
uuid=$("$nmcli" -g connection.uuid connection show id "$profile")
case "$uuid" in ????????-????-????-????-????????????) ;; *) echo 'profile has no exact UUID' >&2; exit 2;; esac
case "$uuid" in *[!0-9a-fA-F-]*) echo 'invalid profile UUID' >&2; exit 2;; esac
[ "$("$nmcli" -g connection.type connection show uuid "$uuid")" = '802-11-wireless' ] || { echo 'profile must be Wi-Fi' >&2; exit 2; }
[ "$("$nmcli" -g 802-11-wireless.ssid connection show uuid "$uuid")" = 'Conduit-Climate' ] || { echo 'profile must target the Conduit-Climate SSID' >&2; exit 2; }
if [ -z "$interface" ]; then
    devices=$("$nmcli" -t -f DEVICE,TYPE device status | awk -F: '$2=="wifi" {print $1}')
    # Linux interface names cannot contain shell whitespace. Refuse ambiguity.
    set -- $devices
    [ "$#" -eq 1 ] || { echo 'select exactly one Wi-Fi interface explicitly' >&2; exit 2; }
    interface=$1
fi
case "$interface" in ''|-*|*[!a-zA-Z0-9_.-]*) echo 'invalid Wi-Fi interface' >&2; exit 2;; esac
[ "$("$nmcli" -g GENERAL.TYPE device show "$interface")" = wifi ] || { echo 'the selected device must be Wi-Fi' >&2; exit 2; }
if [ "$mode" = --facts ]; then
    printf '%s\n%s\n' "$uuid" "$interface"
    exit 0
fi
cookie='# Conduit climate-test reconnection grant v1'
rule="$user_name ALL=(root) NOPASSWD: /usr/bin/nmcli --wait 30 connection up uuid $uuid ifname $interface"
if [ "$mode" = --print ]; then
    printf '%s\n%s\n' "$cookie" "$rule"
    exit 0
fi
[ "$(id -u)" -eq 0 ] || { echo 'installation needs host sudo authentication; use cargo xtask setup environment-host-wifi' >&2; exit 2; }
[ -x /usr/sbin/visudo ] || { echo 'visudo is required' >&2; exit 2; }
mkdir -p /etc/sudoers.d
destination=/etc/sudoers.d/conduit-climate-test-$user_name
if [ -e "$destination" ] || [ -L "$destination" ]; then
    [ -f "$destination" ] && [ ! -L "$destination" ] || { echo 'existing grant path is not a regular file' >&2; exit 2; }
    IFS= read -r first_line < "$destination"
    [ "$first_line" = "$cookie" ] || { echo 'refusing to overwrite an unrelated sudoers file' >&2; exit 2; }
fi
umask 077
temporary=$(mktemp /etc/sudoers.d/.conduit-climate-test.XXXXXX)
trap 'rm -f "$temporary"' EXIT HUP INT TERM
printf '%s\n%s\n' "$cookie" "$rule" > "$temporary"
chmod 0440 "$temporary"
/usr/sbin/visudo -cf "$temporary"
mv "$temporary" "$destination"
trap - EXIT HUP INT TERM
printf 'Installed %s\n' "$destination"
