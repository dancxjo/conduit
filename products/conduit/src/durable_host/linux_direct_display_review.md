# Boot Host `{host}` on tty{tty}

The prepared system service runs the installed Conduit executable as `{user}`
on seat0, with its existing private state. Use a dedicated local console with
systemd-logind, PAM login and SDL2 KMSDRM support. A desktop using the same
DRM device must be stopped before activation. Keep SSH access for recovery.

Record which services are active and enabled before switching. Run the user
commands in the session of `{user}`.

```sh
systemctl --user is-enabled conduit-host.service
systemctl --user is-active conduit-host.service
systemctl is-enabled getty@tty{tty}.service
systemctl is-active getty@tty{tty}.service
systemctl cat conduit-direct-display.service
```

Review the generated unit. If a direct-display unit already exists, preserve
it and its enabled/active state before replacing it. With the VT available,
enable the session and start it immediately:

```sh
systemd-analyze verify {unit}
systemctl --user disable --now conduit-host.service
sudo systemctl disable --now getty@tty{tty}.service
sudo install -m 0644 -- {unit} /etc/systemd/system/conduit-direct-display.service
sudo systemctl daemon-reload
sudo systemctl enable --now conduit-direct-display.service
```

Inspect the current process, including output that PAM routes to its own session:

```sh
systemctl status conduit-direct-display.service
journalctl -b "_PID=$(systemctl show -p MainPID --value conduit-direct-display.service)"
```

Service startup establishes the console session. Showing the Body's Face also
requires a current admitted Show and a release with the native Face consumer.

To undo a fresh installation:

```sh
sudo systemctl disable --now conduit-direct-display.service
sudo rm -- /etc/systemd/system/conduit-direct-display.service
sudo systemctl daemon-reload
```

Restore the old unit if one existed, then restore the individually recorded
enabled and active states of the console and Host services. Changing the release
or its selected Host configuration requires preparing the unit again.
