# Recoverable native input refusal

Refs #5608, #5629 and #5994; delivery PR #6255.

An actual unedited text-field Enter is incomplete: its source-owned action
requires an explicit argument, and the Mask must not infer a replacement from
displayed prose. NativeFaceMask previously rendered only stale-input notices.
Other expected input errors returned without visible feedback; Crèche then
propagated them out of its interactive loop, causing a terminal kernel refusal.

The existing Mask refusal presentation now gives short bounded notices for
stale, unknown, unavailable, unsupported, over-bound and incomplete input.
It returns the original typed error. The same Face/Show, source revision,
producer publication, focus and authority remain unchanged. A single fixed
optional enum records only successful completion of that input's notice
scanout, and is reset on input and semantic publication replacement. It owns
no application state or retry. Modifier-only reports do not clear the notice
or create a misleading redraw acknowledgement before a text key.

Crèche continues only for an error matching that completed current-Show local
refusal. It emits a bounded `CONDUIT_CRECHE_INPUT_REFUSAL` observation, calls
no source owner, and accepts later genuine input. Display loss, producer or
layout failure cannot manufacture that acknowledgement and still propagates.
The old unexplained protected Crèche service refusal remains a separate gap.

CPU proof covers natural incomplete Enter followed by actual replacement and
source commit, six distinct notice/error pairs without a new publication, and
failed refusal scanout retiring Show/input while retaining immutable Face
meaning. All 17 native Mask cases pass. The final focused three-case run additionally
checks Shift-only notice preservation and an actual capital-letter replacement
with release transitions. The xtask harness compiles and formatting passes.
No 48-command, 192-byte payload or eight-surface bound is increased.

The control journey additionally performs one actual Enter on the focused
unedited name field, checks the precise nonfatal refusal/unchanged Face and
Show/no owner change, captures its original pixels, and then uses the existing
checked rename to prove the guest is still interactive. It adds one capture
within the existing 27 limit. QEMU enactment is pending until a fresh clean
Source build completes; compilation and CPU evidence do not substitute for it.

## Real QEMU enactment

Clean Source `75e7b3965f25c8117f6d34499bc2e5f8212b218d` passes the supported
`cargo +stable xtask make conduitos creche-controls-proof --viewport 640x480`
build-and-proof entrance. The [retained packet](evidence/native-input-refusal-qemu-20261022/)
contains 14 original captures, complete serial/QMP observations, exact build
manifest, typed refusal receipt and post-refusal checked rename/restoration.
All 14 PNG lengths, hashes, chunk CRCs, dimensions and independently decoded
RGBA hashes pass. The incomplete-field capture was visually inspected: the
focused name control remains visible and the footer says `Enter value`.

The actual Enter returns one `native-face-mask-input-incomplete` local refusal
with acknowledged scanout, unchanged Face/revision/Show and zero source-owner
calls. No accepted source change or new publication is emitted. Subsequent
typing, actual source commit, naming choice and restoration finish on the same
live guest. This is emulator enactment, not physical or human evidence.
Original observed name is retained as evidence; the final restored name is
the explicitly committed ASCII probe, not a claim to have retyped the original.

The earlier protected Crèche service refusal remains separately unresolved.
Provider-loss/platform fallback, larger-graph Source admission and required
combined-head integration/normal merge are still open. #6252 advanced to
71e8e571e2101b74b8c65993a4ab8a650ea77d39 while this proof ran; cancelled
prior-source Candidate shards are not acceptance. Captures retain Source `75e7b`
rather than being relabelled as that integration head or later documentation.
