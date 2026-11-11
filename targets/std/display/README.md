# Linux direct display adapter

This optional adapter supports [#6325](https://github.com/dancxjo/conduit/issues/6325).
It uses SDL2 KMSDRM beneath an ordinary `GraphicsScene` Show. It reuses ConduitOS's
existing raster implementation without copying it. No Body, observation state,
formatting policy, recorder, scheduler or desktop session is introduced here.
A prepared scanout owns one texture and an at-most-8-MiB CPU surface. Loss is
terminal for that preparation. The installed Linux Host binds actual initialized display custody and an admitted
present operation before offering this Back. Checked configuration alone is not readiness.

Run the CPU proof and strict lint through:

```sh
cargo xtask check linux-display
```

The check uses the installed SDL2 runtime (Debian package `libsdl2-2.0-0`), creating
an owned linker alias without changing system libraries. Its explicitly selected
SDL dummy/software renderer verifies texture readback against the existing
raster. It does not establish DRM master access, HDMI output, Pi compatibility,
input, display reconnection or actual Mask/Body lifecycle integration.

The production preparation entrance requires `kmsdrm` and refuses fallback to
X11, Wayland or a diagnostic renderer. No new ready Host offer is advertised.
The installed Host can initialize this adapter from its checked scanout/Mask IMAGE
and render a joined/member Face through the ordinary native Mask and compositor.
Its current service path discards keys and does not present an owned Body. This
is read-only presentation, not the complete interactive ConduitOS GUI.

Optional boot-to-display session preparation is available through the **exact
installed executable** built with `native-display`:

```sh
/path/to/installed/conduit host service prepare-direct-display /path/to/release-manifest.json \
  --state-dir /path/to/private/host-state --output /path/to/new-review-directory \
  --user conduit --tty 7
```

The Host must be stopped and select initialized scanout and a native Mask in its
retained checked IMAGE. The command reverifies the exact installed release and
requires an unprivileged account that owns its state. It creates a private
systemd unit and `REVIEW.md` with prerequisites, explicit activation steps and
restoration instructions. It never installs/activates the unit, changes device
permissions, disables a desktop, or changes normal user-service startup.
The generated session uses PAM login, logind seat0 and an explicitly dedicated
VT; distribution session/DRM custody and SDL KMSDRM availability still need local
verification. It does not install dependencies or promise physical output.

Review the generated artifacts before activating the optional session. Physical
acknowledged Show, cold reboot, output loss and maintained maintenance access
remain acceptance gates under #6325. Shared interactive owner/member bootstrap
and input routing remain required to finish optional boot into the full GUI.
This adapter and preparation command alone are not a completed appliance image.

A member that must reconnect after finite encounters can explicitly select a
**standing contact**. On its running Body owner, use the ordinary `body
member-route` command with `--continuing`, the member's public credential, a
numeric `wss://IP:PORT` locator, and the reviewed TLS certificate/key. Redirect
its one contact document to a private file. On the stopped member install it:

```sh
conduit body member-contact --descriptor /private/member-contact.json \
  --state-dir /private/host-state --authorize-route
```

The owner listener must remain running. The contact is retained planning policy,
not an invitation, current presence, initialized Line or execution grant. Every
connection still performs the existing signed return/current-member proof and
exact native Mask admission. The native service retires a carrier before its
finite frame/deadline budget is exhausted, obtains fresh pinned evidence, and
authenticates the replacement without clearing historical pixels. Attempts and
I/O leases are bounded, including partial TLS/WebSocket traffic. Numeric locators
avoid unbounded DNS during that loop; an address change needs explicit contact
replacement. Owner Host/Boot/key changes also require explicit reselection.

After physical verification, boot-to-display uses the reviewed system unit.
For tty1, preserve existing states, disable the ordinary user Host service,
disable `getty@tty1.service`, and enable `conduit-direct-display.service` rather
than leaving competing boot units enabled. The account and DRM/session
prerequisites above still apply; these commands do not add keyboard interaction
or turn historical pixels into a current Show.
