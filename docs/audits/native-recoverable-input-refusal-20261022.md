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
