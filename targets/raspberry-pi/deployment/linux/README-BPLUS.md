# Original Raspberry Pi Model B+ v1.2 Linux Host

[#6528](https://github.com/dancxjo/conduit/issues/6528) owns this Ethernet/SSH
bootstrap. It uses Raspberry Pi OS Lite **32-bit** Trixie on the original
ARM1176/ARMv6 Model B+. The binary is compiled for ARMv6 hard-float; a Debian
ARMv7 `armhf` binary or AArch64 image is unsuitable for this board.

The image preloads the sealed Conduit release, build provenance and these
instructions. On the actual Pi, first boot installs a fresh Conduit Host under
user `dancxjo`, starts its systemd **user** service, and preserves its state at
`/var/lib/conduit`. No Host identity, control secret, SSH private key or Body
invitation is cloned from the builder. Joining a Body remains an explicit action.

## Prepare and transfer

The release builder requires cross 0.2.5, Docker and Rust 1.98.1 with the
`arm-unknown-linux-gnueabihf` standard library. Its adapter pins the ARM1176 CPU,
pinned compiler defaults for ARMv6 and VFP hard float, with an ARMv6-capable sysroot; do not substitute a Debian
ARMv7 sysroot.

```sh
cargo +stable xtask make host release --platform linux-armv6 \
  --output /path/to/linux-release
cargo +stable xtask make host raspberry-pi-bplus-linux \
  --base /path/to/2026-10-06-raspios-trixie-armhf-lite.img.xz \
  --release-dir /path/to/linux-release \
  --output /path/to/conduit-bplus.img \
  --ssh-public-key /path/to/maintenance.pub
```

The release directory must contain `conduit-linux-armv6`,
`linux-armv6-build.json` and `raspios-model-b-plus-v1.2.json` from the supported
ARMv6 release producer. `--mtools-dir /path/to/bin` selects an owned mtools
installation; `--hostname` and `--user` change their defaults. The preparer
checks the pinned official compressed-image digest, exact target manifest,
release bundle/file digests, safe file names and ARM ELF header before creating
image bytes. It checks `bcm2708-rpi-b-plus.dtb`, verifies FAT readback, and emits
`conduit-bplus.img.sha256` plus `.bootstrap/receipt.json`. It refuses overwrite
and never flashes a device.

Transfer the image and checksum to the computer with the SD writer. In their
common directory, run `sha256sum -c conduit-bplus.img.sha256` (on macOS,
`shasum -a 256 -c conduit-bplus.img.sha256`). Write the **whole image** to a
removable microSD card using Raspberry Pi Imager's custom-image option; retain
its embedded settings instead of applying Imager customisation. Verify the
writer's selected removable device carefully. Insert the card into the B+,
connect Ethernet with DHCP and power it on. No Internet download is required
for first-boot Conduit installation; all payload and required OS services are
already present. Allow several minutes on this small single-core machine.

## Connect and inspect

```sh
ssh -i /path/to/maintenance dancxjo@conduit-bplus.local
sudo cloud-init status --wait
conduit host service status --state-dir /var/lib/conduit --json
systemctl --user status conduit-host.service
journalctl --user -u conduit-host.service -b --no-pager
```

Use the DHCP address from your router if mDNS is unavailable. SSH password login
is disabled; keep the matching private key on your maintenance computer. The
Linux account has passwordless sudo for maintenance. Its linger-enabled user
service continues without an SSH session. The release snapshot resides in a
root-owned `/opt/conduit/releases/<bundle-digest>/`; `/usr/local/bin/conduit`
points to that exact release. The stock service installer also retains its
verified executable inside `/var/lib/conduit/releases/`.

These instructions are on the FAT partition at
`conduit-bplus/README-BPLUS.md`, and first boot copies them to
`~/CONDUIT-BPLUS.md`. If setup fails, inspect:

```sh
sudo tail -n 120 /var/log/cloud-init-output.log
sudo cloud-init status --long
sudo systemctl status cloud-final.service
```

The installer can be retried after fixing the reported problem:
`sudo sh /boot/firmware/conduit-bplus/bplus-first-boot.sh dancxjo`.
A retry preserves the device's existing Host identity. Preserve
`/var/lib/conduit` when maintaining that Host; create independent Hosts on
independent freshly written cards rather than copying an initialized filesystem.

## Join deliberately

Obtain a fresh routed invitation from the intended Body owner and transfer it
privately to this Pi. No invitation is embedded in the image. Then run:

```sh
conduit body join /path/to/invitation.json \
  --state-dir /var/lib/conduit --authorize-join
conduit body status --state-dir /var/lib/conduit --json
```

Membership, current reachability and actual equipment offers remain distinct.
The Mini PiTFT is expected to stay **blank** with this Host-only image: no Linux
display adapter is configured. Use Ethernet/SSH or HDMI for boot diagnostics.
This image does not prove Mini PiTFT display/button adapters, Camera V2.1 capture,
physical boot, graphics/Mask/Show or a completed Body join. Those require the
corresponding Linux adapters and physical observation on this board.

The base is pinned to the [official image directory](https://downloads.raspberrypi.com/raspios_lite_armhf/images/raspios_lite_armhf-2026-10-06/)
and its adjacent SHA-256 file. Raspberry Pi lists its [32-bit OS as compatible
with all Pi models](https://www.raspberrypi.com/software/operating-systems/).
The singular `user:` configuration follows the official
[Imager configuration generator](https://github.com/raspberrypi/rpi-imager/blob/main/src/customization_generator.cpp),
retaining the distribution's default group memberships.
