# Linux direct display adapter

This held implementation for [#6325](https://github.com/dancxjo/conduit/issues/6325)
uses SDL2 KMSDRM beneath an ordinary `GraphicsScene` Show. It reuses ConduitOS's
existing raster implementation without copying it. No Body, observation state,
formatting policy, recorder, scheduler or desktop session is introduced here.
A prepared scanout owns one texture and an at-most-8-MiB CPU surface. Loss is
terminal for that preparation. The Linux Host still must bind actual display
custody and an admitted present operation before offering this Back.

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
