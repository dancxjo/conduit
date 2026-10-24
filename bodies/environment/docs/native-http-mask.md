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

The attended finite run on source `76f125afc` passed three physical DHT
publications and independent producer Face replay. Supplemental HTTP observations
matched all three native Face identities/revisions across 59 successful responses,
with 503 observed before publication and after the last publication. Root peaked
at 82,828 bytes and retained 13,988 after retirement; the link reserved 36,988 stack
bytes, with no high-water claim. The protected operational image was restored and
verified. See [retained evidence](../evidence/2026-10-24-native-http-face/README.md).
The initial stack failure is retained separately; the correction separates Mask
scratch from the deeper semantic publication validation call chain.
