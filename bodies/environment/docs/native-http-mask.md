# Native Face HTTP Mask integration

The experimental ESP32 `native-face-root` profile projects the actual retained
producer Face into bounded HTTP Show material after each committed publication.
The synchronous reader borrow ends before any socket await. This does not add a
second observation store, browser Body membership, a Line, or radio authority.

Root prepares a finite publication quota and leases using its admitted source
cadence. Fresh material expires at the original observed-time freshness boundary;
a failed later capture cannot extend it. The HTTP timestamp is explicitly an
observed-at-boot coordinate, not an acquisition timestamp. Missing, expired or
withdrawn custody returns HTTP 503. Source cancellation withdraws the Show before
quiescence; dropping its publisher closes the mailbox permanently.

Two fixed response buffers are allocated once before the service task accepts
commands and retained across service mode switches. Insufficient preparation
capacity refuses service. Requests do not allocate response backing. The native
profile excludes the legacy inline-buffer future from its task type. Root keeps
112 KiB and the SDK 72 KiB; 512 additional bytes of existing Root backing move to
the reclaimed RAM bank to preserve the 36 KiB stack floor.

CPU tests cover actual target projection, Face identity, observed coordinates,
failure, expiry, withdrawal and zero allocations during projection/rendering.
The buffered exchange test checks transport bytes and clearing a previous response
when the next reading is unavailable. A keyed release link proves only static
placement; it does not prove stack high-water or complete concurrent admission.

Run the finite attended native test through
`cargo xtask prove environment owner-face-run-esp32`; preserve captures and restore
and verify the protected operational image afterward. Live HTTP/native correlation
and resource measurements must be recorded separately before claiming deployment.
The ongoing sampling/display/network/recorder combination remains unadmitted.
