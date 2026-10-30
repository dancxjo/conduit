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

This package is a development adapter. Build and qualify it off the Pi through
`cargo xtask make linux-face-display --target linux-aarch64 --output NEW_DIRECTORY
--source-identity FULL_GIT_SHA --linker ABSOLUTE_AARCH64_GCC_PATH
--linker-sha256 SHA256` (one command). The source checkout must be clean and match
the full commit identity. The explicit linker executable must match its digest
and report `aarch64-linux-gnu`. Rust 1.98.1 and the locked dependency resolution
are used with two build jobs. `CARGO_TARGET_DIR` may select an existing cache.
`--dry-run --json` validates inputs without building or creating the output.

A successful build creates the executable and `qualification.json` in a new
output directory, recording source, compiler, target and exact executable SHA256.
Qualification checks ELF64 little-endian AArch64 and the canonical Linux dynamic
interpreter. It does not qualify the complete SDK/sysroot, target glibc compatibility,
physical scanout, membership, a Host-selected Back or a release bundle. Those
remain separate proofs. This command does not install or deploy anything.
The checked Linux framebuffer must expose packed RGB565, 240–1920 by 240–1200,
with valid memory, offsets, and row stride.

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

Local presentation controls occupy a separate touch bar above the Host clock.
Tap Theme to cycle the built-in Navy, Editorial and Engineering Look palettes,
and Units to switch the existing Celsius/Fahrenheit quantity formatter. Zoom
in/out selects a retained producer-time window (one hour, fifteen minutes,
five minutes or one minute); Range resets to all retained samples. These are
view windows, not sampler frequency or aggregation requests. No missing history
is synthesized. Pinching inside the graph invokes the same bounded zoom actions.

Drag the visible divider to resize the report and graph columns. Both retain
at least 320 pixels; dragging to either outer edge collapses the split and
restores timed full-page cycling. Drag the remaining edge handle inward to
restore the split. Sensor properties are grouped under their subject heading,
with individual field labels below it; narrow reports may paginate complete rows.

Linux input is acquired from an absolute-position evdev device, independently
of the semantic preferences/actions in `conduit_face::presentation_preferences`.
The adapter supports at most five touch contacts and consumes at most 128 kernel
events per poll, without grabbing the device. Queue overrun/disconnection resets
gestures and requires a fresh touch after release. It scans at most 32 event nodes;
`CONDUIT_DISPLAY_INPUT_DEVICE` may select an exact event device instead. The
service user needs input-device read access. Input changes only local presentation;
it never enables or submits Body actions. Palette selection reuses the native
Look-token paint resolver; it does not claim all Look layout/font facets.

Preferences persist atomically in the display user's private `preferences.json`
under `~/.local/state/conduit-face-display/` (or the explicit
`CONDUIT_DISPLAY_PREFERENCES_FILE` path). The file is bounded to 2048 bytes and
must be a current-user regular 0600 file. Saved preferences take precedence over
the initial `CONDUIT_DISPLAY_TEMPERATURE_UNIT` default. Input is polled every
10ms between paints; redraws are limited to ten per second during interaction
and once per second while idle. These are development-adapter scheduling limits,
not an admitted Host Back or a real-time input-latency guarantee.
