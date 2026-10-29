# Linux read-only Face display

This framebuffer adapter presents an exact, caller-supplied portable `Face` with
ConduitOS's existing `NativeFaceScene` and typography raster. It does not read
sensors, reconstruct an environmental Face from numbers, select a Body source,
or grant membership. The same Face identity, revision, and producing basis are
retained; viewport, pagination, and pixel encoding belong to this realization.
Actions are unavailable. It is not yet an installed Host-selected display Back
or a hardware Show acknowledgement.

Build off the Pi through `cargo xtask make host release --platform
linux-armv6-face --output <directory> --source-identity <source>`. The pinned
ARM1176 toolchain also runs on a Pi 3B+ with 32-bit Raspberry Pi OS. No compiler
is needed on the Pi. The checked Linux framebuffer must expose packed RGB565,
240–1920 by 240–1200, with valid memory, offsets, and row stride.

The upstream producer must validate its source and own semantic freshness. It
atomically replaces a private mode-0600 regular file in an owner-controlled
private directory, bounded to 600 KiB, with:

```json
{
  "schema": "conduit.local-face/leased@1",
  "boot_id": "exact /proc/sys/kernel/random/boot_id on this host",
  "valid_until_boot_ms": 123456,
  "face": {}
}
```

`face` is the full serde representation of the existing valid Face, not a JSON
quantity summary. Lease time uses Linux `CLOCK_BOOTTIME` milliseconds; the
producer should refresh each second with a five-second lease. Leases more
than 30 seconds in the future are refused. This lease only protects against a
dead producer/reboot; it does not decide sensor freshness or alter the Face.
An unavailable, expired, or refused input shows a separate Host-owned local
status with no Body or execution provenance and no cached sensor values.

The included user service is an explicit deployment adapter. Install its binary
in `~/.local/lib/conduit-face-display/`, create
`~/.local/state/conduit-face-display/`, and install the unit in the user's
systemd directory. The user needs framebuffer access and lingering for boot.
The environmental producer and installed Host's Body admission remain separate
requirements; displaying a Face is not evidence of either.
