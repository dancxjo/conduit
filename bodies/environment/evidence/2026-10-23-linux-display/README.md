# Linux SDL preparation

The supported `cargo xtask check linux-display` command passed both CPU tests
and strict all-target library Clippy. The test uses the actual installed SDL2
dummy/software renderer and compares its pixels with the ordinary Conduit
GraphicsScene raster. It covers finite extents, frame clearing, opaque pixel
format, terminal display loss and zeroization. Strict dispatch-package Clippy
also passed. These are CPU/library proofs; no DRM master, Pi HDMI, input or
Body/Mask admission is claimed. The native graphical client and boot service
remain unfinished. Source file digests bind the held code beside this evidence.

The supported Pi bootstrap command produced a verified official Lite-based
Ethernet/SSH image with cloud-init boot-file readback. It includes only a
maintenance public key, no private key or Wi-Fi credentials, and explicitly
selects Ethernet DHCP before first-boot package installation. A second invocation
refused the existing output and its SHA-256 stayed unchanged. No SD card was
written. The retained receipt explicitly reports no initialized Conduit Host,
installed graphics client, physical boot or complete appliance image.
