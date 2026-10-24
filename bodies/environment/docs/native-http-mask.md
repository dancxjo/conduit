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
capacity refuses service. Requests do not allocate response backing. Reading,
setup pages and submission acknowledgements share this backing; rendering scratch
ends before socket awaits. The native profile excludes the legacy inline-buffer
future from its task type. Releasing duplicate response state recovers 4,080 static
bytes in the keyed image. This exact capacity moves into Root's internal heap,
raising it from 114,688 to 118,768 bytes. The SDK keeps 73,728 bytes and the linked
CPU stack keeps 36,988 bytes against its unchanged 36 KiB floor. Earlier captures
retain their original heap capacities; the attended run below verifies this new partition.

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

The later native run on source `6a3852565` verifies the shared page/ack response
backing and enlarged Root partition. Three physical publications matched 59 HTTP
responses. A generated nonexistent-network request was acknowledged during
sampling, stayed deferred until retirement, then made one failed join and restored
AP services in the same Boot. Root peaked at 79,692 of 118,768 bytes. The ordinary
slot change saved 3,136 actual heap bytes and static response reuse supplied 4,080
additional Root capacity, leaving 39,076 bytes unused during this finite sampling
case. All allocated RAM section addresses/extents match the preceding image; the
stack floor is unchanged. The protected operational image was restored and
verified. See [ordered native setup proof](../evidence/2026-10-24-http-response-lifetimes/README.md).
This does not admit joining while sampling or prove successful station attachment.
