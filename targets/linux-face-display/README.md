# Linux read-only Face display

This framebuffer adapter presents an exact, caller-supplied portable `Face` with
ConduitOS's existing `NativeFaceScene` and typography raster. It does not read
sensors, reconstruct an environmental Face from numbers, select a Body source,
or grant membership. The same Face identity, revision, and producing basis are
retained; viewport, pagination, and pixel encoding belong to this realization.
A property-focused Look brings Info-subject typed values and status forward;
quantity decoding, precision, and units reuse Face formatting. This creates no
replacement Face. Larger unsupported layouts fall back to the ordinary scene.
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

Set `CONDUIT_DISPLAY_TEMPERATURE_UNIT=fahrenheit` or `celsius` to choose the local Mask temperature unit. Conversion uses the shared exact Quantity formatter; incompatible values retain their canonical wording. The input Face and its measurement units remain unchanged.

Large property pages precede the ordinary native Face pages in one rotation. Mixed sensor and transcript content therefore remains perceptible; the property Look never replaces the complete Face view. This renderer does not admit workers, synthesize contributions, or renew a producer lease.

The display retains its last successfully validated same-Boot Face when its live
feed expires or becomes unavailable. A separate Host notice marks that view
`STALE`; the original Face, source times and execution identity are unchanged,
and actions remain disabled. A privately published, semantically valid same-Linux-Boot file may be recovered
as explicitly historical at startup even if its live lease has expired. A
foreign Boot, malformed document or unbounded future lease still refuses; this
recovery does not rebase the original observation age.

`CONDUIT_RETAINED_HISTORY_FILE` selects an optional private, current-owner 0600
regular document (at most8192 bytes) using
`conduit.environment/retained-observation-series@1`, custody
`retained-signed-evidence`. The admitted recorder/exporter must verify original
Root signatures and exact Body/Boot/Play scope before creating this projection.
The renderer checks scope hashes, current observation Forms and at most16
original21-byte records; it does not itself establish membership, verify the
original batch signatures or claim selected recording State commitment.

That document displays a separate `RETAINED HISTORY / SIGNED EVIDENCE` line
chart, alternating with the ordinary full Face pages. Producer Boot milliseconds
form its horizontal axis, never fabricated wall time. Failure samples and sequence
gaps break lines. Temperature uses the selected Celsius/Fahrenheit display
preference through canonical Quantity conversion. Historical evidence remains
historical when the live Face transport lease expires; keeping it visible does
not renew the live Play, its actions or its lease.

A reserved 32-pixel footer shows the local Host's `HH:MM:SS UTC` wall clock and
an alternating `DISPLAY +` / `DISPLAY o` heartbeat once per second. This is
presentation liveness only, including while a retained or unavailable view is
shown. `host clock` does not claim synchronized BodyTime, accurate sensor time,
or producer freshness. A failed wall-clock read shows `CLOCK UNAVAILABLE`
without stopping the heartbeat. Narrow displays use the compact `DISPLAY` label
and `NO CLOCK` refusal wording. Content and charts use the remaining viewport;
the footer never covers their labels or replaces their freshness notices.
