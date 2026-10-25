# Linux direct display adapter

This held implementation for [#6325](https://github.com/dancxjo/conduit/issues/6325)
uses SDL2 KMSDRM beneath an ordinary `GraphicsScene` Show. It reuses ConduitOS's
existing raster implementation without copying it. No Body, observation state,
formatting policy, recorder, scheduler or desktop session is introduced here.
A prepared scanout owns one texture and an at-most-8-MiB CPU surface. Compositor
`PixelTarget` writes reuse that surface; `commit_frame` performs SDL texture
upload, copy and present before a caller may acknowledge a Mask Show. The
`present(GraphicsScene)` entrance uses the same commit boundary. SDL's present
call has no success return, so this proves completion of the selected SDL calls,
not independently observed physical scanout. Loss is terminal for that preparation.
`Scanout::initialized_display` returns a nonserialized lifetime token only after
actual SDL preparation. It reports the observed backend, renderer, window ID,
finite geometry and process-local instance nonce. All clones become lost when
the scanout loses custody or drops; repreparation creates a fresh instance.
Dummy/software tokens describe that explicit diagnostic backend, not production
DRM output. The token grants no Ready Base or Show authority by itself. `claim_host` atomically grants one noncloneable
`DisplayHostClaim` for this lifetime; cloned observations cannot attach a second
Host. Dropping that claim terminally revokes the shared preparation, including
actual scanout write/present/commit operations. Reattachment needs a fresh
preparation. Host admission should finish fallible staging before claiming.

The standard Host’s optional `linux-display` feature accepts a live KMSDRM token
through `attach_initialized_display`. While idle, it registers the exact
cooperative Ready Show Base and one finite Show resource, then commits the
current offer generation and rebuilt resource ledger together. It retains the
private lifetime observation. `ready_native_display` refuses lost or changed
bindings; after loss and ordinary effect drain, `refresh_native_display` retracts
only that display’s offers. A new preparation is required to attach again.
The public attachment refuses dummy/software tokens; the private test policy
exercises those exact transitions with an actual diagnostic SDL window.
Readiness alone does not issue NativeSurfaceProvider possession or a Mask Play.
The product’s `prepare_linux_creche_output` checks that the actual mutable
scanout has the exact same live instance as this Host attachment before bridging
it to the ordinary native Mask. The bridge performs no presentation or Show
acknowledgement. Scanout loss and Host-claim loss refuse further provider
construction; metadata equality cannot substitute for the same live instance.

`prepare_input` establishes a fresh input boundary before Play by draining old
queued keys in finite batches. It checks the actual queue after an empty poll
cycle; SDL may end a cycle while events still remain queued. The queue check owns one
stack slot and performs no Rust heap allocation. `poll_input` then inspects at most 256 events
without waiting or allocating, forwards canonical key transitions only for the
same live window, and suppresses repeat notifications. Invalid keys and absent
preparation are typed refusals. A semantic Face replacement withdraws input
readiness and requires another finite drain before key delivery. Window focus/output loss revokes the preparation;
reacquisition requires fresh output and input preparation. These observations
must still pass through the selected Mask's acknowledged Show/focus route.
The diagnostic test injects actual SDL queue events; it proves no physical
keyboard, user enactment or completed application event loop.

The Linux Host still must bind actual display
custody and an admitted present operation before offering this Back.

The package remains a standalone workspace at `targets/linux/display`; the
product’s `native-creche-sdl` feature uses this same scanout implementation.

Run the CPU proof and strict lint through:

```sh
cargo xtask check linux-display
```

The check uses the installed SDL2 runtime (Debian package `libsdl2-2.0-0`), creating
an owned linker alias without changing system libraries. Its explicitly selected
SDL dummy/software renderer verifies texture readback against the existing
raster. It also checks standard Host attachment, exact binding and loss
transitions using the private diagnostic policy, plus strict package lint.
It does not establish DRM master access, HDMI output, Pi compatibility,
input, display reconnection or actual Mask/Body lifecycle integration.

The production preparation entrance requires `kmsdrm` and refuses fallback to
X11, Wayland or a diagnostic renderer. Scanout preparation alone advertises no
Host offer; explicit Host attachment is required.
The native graphical client, systemd boot service and SD payload remain work;
this library alone is not the completed appliance image.
