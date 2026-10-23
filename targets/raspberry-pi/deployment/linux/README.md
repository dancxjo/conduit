# Pi 3B+ Linux bootstrap image

Held preparation for [#6325](https://github.com/dancxjo/conduit/issues/6325).
The pinned Raspberry Pi OS Lite 64-bit Trixie image supports the Pi 3B+.
This creates an Ethernet/SSH maintenance bootstrap, **not a complete appliance
image**. There is no initialized Conduit Host, Body, recorder or graphics client.
Those identities/resources must be installed and initialized on the actual Pi.

```sh
cargo xtask make host raspberry-pi-bootstrap \
  --base /path/to/2026-10-06-raspios-trixie-arm64-lite.img.xz \
  --output /path/to/climate-pi-bootstrap.img \
  --ssh-public-key /path/to/maintenance-key.pub
```

Use `--mtools-dir /path/to/bin` if `mcopy` and `mtype` are in an owned tool cache.
The base comes from the [official download directory](https://downloads.raspberrypi.com/raspios_lite_arm64/images/raspios_lite_arm64-2026-10-06/).
The command verifies its SHA-256, exclusively creates an ordinary image file,
checks the Pi 3B+ device tree, and verifies boot-file readback. It never writes
a block device or overwrites an existing image/sidecar directory. It needs
at least 4 GiB free and `xz` support in Python, `ssh-keygen`, and mtools.

Defaults are hostname `climate-pi`, user `dancxjo`, US keyboard and Los Angeles
timezone. SSH accepts the supplied public key and disables password login;
private keys and Wi-Fi credentials never enter the image. Ethernet uses DHCP,
with network readiness required for first-boot package installation. The
standard Raspberry Pi default-user integration is retained through singular
`user:` cloud-init configuration. SSH and mDNS are enabled explicitly. Linux
boots to `multi-user.target`; the display manager is masked. Lite carries no
desktop session. First boot installs the SDL2/GBM/EGL/Mesa runtime dependencies
from distribution repositories, so Ethernet Internet access is needed.

Only cloud-init's instance identifier is generated during image preparation.
It is not a Conduit Host or Body identity. Output sidecars retain configuration
and the image digest, with explicit false fields for physical boot, initialized
Host, installed graphics client and complete appliance acceptance. The next
step is actual Host/client installation and a systemd graphics service using
SDL KMSDRM, followed by physical HDMI and Body/recording lifecycle proof.

References: Raspberry Pi's [cloud-init format](https://github.com/raspberrypi/rpi-imager/blob/main/doc/os_customisation_formats.md)
and [configuration generator](https://github.com/raspberrypi/rpi-imager/blob/main/src/customization_generator.cpp).
