#!/bin/sh
# Run by cloud-init as root on the actual Pi; create the Host as its Linux user.
set -eu
umask 077
[ "$(id -u)" = 0 ] || { echo 'first-boot installation requires root' >&2; exit 1; }
user=${1:-dancxjo}
case "$user" in root|''|*[!a-z0-9_-]*) echo 'invalid non-root user' >&2; exit 1;; esac
uid=$(id -u "$user")
gid=$(id -g "$user")
[ "$uid" -ne 0 ] || exit 1
payload=/boot/firmware/conduit-bplus
[ -d "$payload" ] && [ ! -L "$payload" ]
cd "$payload"
sha256sum --strict --check SHA256SUMS
# Read only the fixed manifest, never paths or shell fragments from provenance.
bundle=$(python3 - <<'PY'
import json,re
from pathlib import Path
m=json.loads(Path('raspios-model-b-plus-v1.2.json').read_text())
assert m['schema']=='conduit.release/host-bundle@1'
assert m['target_id']=='std/armv6/raspberry-pi-model-b-plus-v1.2'
assert re.fullmatch(r'sha256:[0-9a-f]{64}',m['bundle_sha256'])
assert {f['path'] for f in m['files']}=={'conduit-linux-armv6','linux-armv6-build.json'}
print(m['bundle_sha256'][7:])
PY
)
for directory in /opt/conduit /opt/conduit/releases; do
    [ ! -L "$directory" ] || { echo 'refusing symlink release directory' >&2; exit 1; }
    install -d -o root -g root -m 0755 "$directory"
done
release=/opt/conduit/releases/$bundle
if [ -e "$release" ] || [ -L "$release" ]; then
    [ -d "$release" ] && [ ! -L "$release" ]
    for name in conduit-linux-armv6 linux-armv6-build.json raspios-model-b-plus-v1.2.json; do
        [ -f "$release/$name" ] && [ ! -L "$release/$name" ]
        cmp "$payload/$name" "$release/$name"
    done
else
    stage=$(mktemp -d /opt/conduit/releases/.bplus-stage.XXXXXXXX)
    trap 'rm -rf -- "$stage"' EXIT HUP INT TERM
    install -o root -g root -m 0555 conduit-linux-armv6 "$stage/conduit-linux-armv6"
    for name in linux-armv6-build.json raspios-model-b-plus-v1.2.json; do
        install -o root -g root -m 0444 "$name" "$stage/$name"
    done
    chmod 0755 "$stage"
    mv -T "$stage" "$release"
    trap - EXIT HUP INT TERM
fi
install -d -m 0755 /usr/local/bin
# Refuse an unrelated existing command instead of silently replacing it.
if [ -e /usr/local/bin/conduit ] || [ -L /usr/local/bin/conduit ]; then
    if [ ! -L /usr/local/bin/conduit ] || [ "$(readlink /usr/local/bin/conduit)" != "$release/conduit-linux-armv6" ]; then
        echo 'refusing to replace an unrelated conduit command' >&2
        exit 1
    fi
else
    ln -s "$release/conduit-linux-armv6" /usr/local/bin/conduit
fi
[ ! -L /var/lib/conduit ] || { echo 'refusing symlink Host state' >&2; exit 1; }
if [ -e /var/lib/conduit ] && [ "$(stat -c %u /var/lib/conduit)" != "$uid" ]; then
    echo 'refusing to take over Host state owned by another user' >&2
    exit 1
fi
install -d -o "$uid" -g "$gid" -m 0700 /var/lib/conduit
user_home=$(getent passwd "$user" | cut -d: -f6)
[ -d "$user_home" ] && [ ! -L "$user_home" ]
install -o "$uid" -g "$gid" -m 0644 "$payload/README-BPLUS.md" "$user_home/CONDUIT-BPLUS.md"
loginctl enable-linger "$user"
systemctl start "user@$uid.service"
# The stock installer owns identity creation, verified release installation and
# the user-systemd service. Never create a root Host or copy one from the builder.
runuser -u "$user" -- env \
    XDG_RUNTIME_DIR="/run/user/$uid" \
    DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" \
    "$release/conduit-linux-armv6" host service install \
    "$release/raspios-model-b-plus-v1.2.json" --state-dir /var/lib/conduit
runuser -u "$user" -- env XDG_RUNTIME_DIR="/run/user/$uid" \
    DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" \
    systemctl --user is-active conduit-host.service
printf '%s\n' 'Conduit B+ Host installed. Membership requires a fresh explicit Body invitation.'
