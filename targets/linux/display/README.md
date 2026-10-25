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
not independently observed physical scanout. Loss is terminal for that preparation. `Scanout::initialized_display` returns a nonserialized lifetime token only after
actual SDL preparation. It reports the observed backend, renderer, window ID,
finite geometry and process-local instance nonce. All clones become lost when
the scanout loses custody or drops; repreparation creates a fresh instance.
Dummy/software tokens describe that explicit diagnostic backend, not production
DRM output. This token is a prerequisite for Host attachment and grants no Ready
Base or Show authority by itself.

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
raster. It does not establish DRM master access, HDMI output, Pi compatibility,
input, display reconnection or actual Mask/Body lifecycle integration.

The production preparation entrance requires `kmsdrm` and refuses fallback to
X11, Wayland or a diagnostic renderer. No new ready Host offer is advertised.
The native graphical client, systemd boot service and SD payload remain work;
this library alone is not the completed appliance image.
