# Invoked as the unprivileged Host account. stdin is the definition generated
# in the verified install process. Only distribution binaries run as root.
set -eu
PATH=/usr/bin:/bin
export PATH
unit_root=/etc/systemd/system
die() { echo "$*" >&2; exit 1; }
admin() { /usr/bin/sudo -- /usr/bin/systemctl "$@"; }
authorize() { /usr/bin/sudo -v; }
remove_unit() { /usr/bin/sudo -- /usr/bin/rm -- "$unit_root/$display"; }
verify_definition() { systemd-analyze verify "$unit"; }
console_exists() { [ -c "/dev/$vt" ] && [ -f /etc/pam.d/login ]; }

activate_display() {
review=$1
vt=tty$2
display=conduit-direct-display.service
getty=getty@$vt.service
host=conduit-host.service
unit=$review/$display

console_exists || die "Linux VT or PAM login unavailable"
[ "$(ps -o tty= -p $$ | tr -d ' ')" != "$vt" ] || die "Install from SSH or a different terminal"
systemctl is-active --quiet systemd-logind.service || die "systemd-logind unavailable"
systemctl --user show -p Version --value >/dev/null || die "install from the Host account's systemd session"
sessions=$(loginctl list-sessions --no-legend --no-pager)
for session in $(printf '%s\n' "$sessions" | awk '{print $1}'); do
    session_tty=$(loginctl show-session "$session" -p TTY --value)
    [ "$session_tty" != "$vt" ] || die "$vt is occupied by a login session"
done
systemctl is-active --quiet display-manager.service && die "release the active desktop before installing a dedicated display"
if [ "$(systemctl get-default)" = graphical.target ]; then
    manager=$(systemctl show -p LoadState --value display-manager.service) || [ "$manager" = not-found ] || exit 1
    [ "$manager" = not-found ] || [ "$manager" = masked ] || die "the configured desktop would compete for the display at boot"
fi
systemctl is-active --quiet --user "$host" && die "stop the running user Host before console installation"
verify_definition
systemctl --help | grep -q -- '--stdin' || die "console installation requires systemctl edit --stdin"

load=$(systemctl show -p LoadState --value "$display") || [ "$load" = not-found ] || exit 1
created=0
case $load in
    not-found) [ ! -e "$unit_root/$display" ] && [ ! -L "$unit_root/$display" ] || die "unrecognized existing display unit" ;;
    loaded)
        [ "$(systemctl show -p FragmentPath --value "$display")" = "$unit_root/$display" ] || die "existing display service has a different origin"
        cmp -s "$unit" "$unit_root/$display" || die "existing display service differs; preserve and review it before replacement"
        [ -z "$(systemctl show -p DropInPaths --value "$display")" ] || die "existing service has local overrides; preserve and review it before replacement"
        ;;
    *) die "cannot take over display service ($load)" ;;
esac
# Refuse to silently change masks, aliases, temporary or otherwise unexpected states.
enabled_state() {
    value=$(systemctl "$@" 2>/dev/null) || :
    case $value in enabled|disabled|not-found|static|linked) echo "$value" ;; *) die "unsupported enablement: $* ($value)" ;; esac
}
active_state() {
    value=$(systemctl "$@" 2>/dev/null) || :
    case $value in active|inactive) echo "$value" ;; *) die "unsupported service state: $* ($value)" ;; esac
}
host_enabled=$(enabled_state --user is-enabled "$host")
getty_enabled=$(enabled_state is-enabled "$getty")
getty_active=$(active_state is-active "$getty")
display_enabled=$(enabled_state is-enabled "$display")
display_active=$(active_state is-active "$display")
if [ "$host_enabled" = enabled ]; then
    fragment=$(systemctl --user show "$host" -p FragmentPath --value)
    [ "$(realpath "$fragment")" = "$(realpath "$review/../$host")" ] ||
        die "the user service belongs to a different installation"
fi
printf 'user Host %s; %s %s/%s; %s %s/%s\n' "$host_enabled" "$getty" "$getty_enabled" "$getty_active" "$display" "$display_enabled" "$display_active" > "$review/previous-state.txt"

rollback() {
    result=$?
    trap - EXIT HUP INT TERM
    [ "$result" = 0 ] && exit 0
    set +e
    [ "$display_active" = active ] || admin stop "$display"
    [ "$display_enabled" = enabled ] || admin disable "$display"
    if [ "$created" = 1 ] && cmp -s "$unit" "$unit_root/$display"; then
        remove_unit
        admin daemon-reload
    fi
    [ "$getty_enabled" != enabled ] || admin enable "$getty"
    [ "$getty_active" != active ] || admin start "$getty"
    [ "$host_enabled" != enabled ] || systemctl --user enable "$(realpath "$review/../$host")"
    echo "Activation failed. Check restored services against $review/previous-state.txt" >&2
    exit "$result"
}

authorize
if [ "$load" = not-found ]; then
    admin edit --full --force --stdin "$display"
    created=1
fi
trap rollback EXIT
trap 'exit 1' HUP INT TERM
[ "$host_enabled" != enabled ] || systemctl --user disable "$host"
[ "$getty_enabled" != enabled ] || admin disable "$getty"
[ "$getty_active" != active ] || admin stop "$getty"
admin enable --now "$display"
systemctl is-active --quiet "$display"
}
