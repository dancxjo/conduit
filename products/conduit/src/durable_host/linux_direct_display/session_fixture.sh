# Substitute display service commands in the test process.
unit_root=$1/system
mode=$2
review_dir=$1/review
mkdir -p "$unit_root" "$review_dir"
printf 'exact unit\n' > "$review_dir/conduit-direct-display.service"
console_exists() { return 0; }
remove_unit() { rm -- "$unit_root/$display"; }
loginctl() { return 0; }
verify_definition() { return 0; }
systemctl() {
    case "$*" in
        'is-active --quiet systemd-logind.service'|'--user show -p Version --value') return 0 ;;
        'is-active --quiet display-manager.service') [ "$mode" = desktop ] ;;
        'is-active --quiet --user conduit-host.service') return 3 ;;
        'get-default') echo multi-user.target ;;
        '--help') echo '--stdin' ;;
        'show -p LoadState --value conduit-direct-display.service') echo not-found ;;
        '--user is-enabled conduit-host.service'|'is-enabled conduit-direct-display.service') echo not-found; return 4 ;;
        'is-enabled getty@tty1.service') echo enabled ;;
        'is-active getty@tty1.service') echo active ;;
        'is-active conduit-direct-display.service') echo inactive; return 3 ;;
        'is-active --quiet conduit-direct-display.service') return 0 ;;
        *) echo "unexpected systemctl: $*" >&2; exit 9 ;;
    esac
}
log=$1/commands
authorize() { echo authorize >> "$log"; }
admin() {
    echo "$*" >> "$log"
    case "$*" in
        'edit --full --force --stdin conduit-direct-display.service') cat > "$unit_root/conduit-direct-display.service" ;;
        'enable --now conduit-direct-display.service') [ "$mode" != failure ] ;;
        *) return 0 ;;
    esac
}
activate_display "$review_dir" 1
