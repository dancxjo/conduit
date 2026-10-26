# Preserve the current Show when refusing stale submission

Refs #5608 and #5994. The direct native Mask submission path previously took
its current interaction session before checking an incoming occurrence.
A genuine retained input for an older Show could therefore retire the valid
newer session even though no source action was accepted.

The correction validates against the actual retained renderer Face/Show by
borrowing the session first. A valid occurrence still consumes it once through
the admitted Mask Fore. The display-aware submission route paints the existing
`Stale: refresh` notice only after an actual typed `StaleShow` or `StaleFace`
refusal, preserving that original cause. A failed repaint retires Show/input;
it cannot claim an acknowledged notice. Normal keyboard/pointer submissions
use this route. No second source truth, new error kind or storage budget.

The existing semantic-recovery appliance is extended to copy through the actual
boot framebuffer, with its deliberate `DisplayError::Lost` wrapper retained.
After genuine S1→S2 recovery on an exactly equal canonical Face and unchanged
draft, it submits the genuine held S1 occurrence. The actual production route
must return `StaleShow`, preserve S2/publication/Face/draft and keyboard routing,
and finish copying the existing notice. Only then does it emit a source/Show
checkpoint and wait on the normal PS/2 adapter. The collector captures once
through QMP before sending a real Enter press/release pair in one QMP request.
The guest consumes and validates both transitions before its acknowledgement,
after which the original
fresh S2 interaction must still be accepted by the canonical owner.

This is an explicit freestanding emulator appliance with actual framebuffer
pixels, not the ordinary product binary, physical scanout or human observation.
The prior counting-target capacity/recovery packet remains unchanged. The
60-second emulator envelope and all scene/surface/arena limits stay unchanged.
No screenshot delay, retry, forced private state, injected scene error or
synthetic notice is used to manufacture the case.

Implementation is prepared. Both new submission cases, all 19 related native
Mask cases and all four collector/profile/capacity/recovery cases pass. The CPU packet reported under
`evidence/native-stale-show-preparation-20261023/` (not present in this Git
checkout) also preserves
the initial featureless invocation, which failed to compile existing tests
requiring `native-compositor`; the corrected invocation supplies that feature.
The fresh QEMU result is recorded below; relevant merged checks remain pending.

The first supported build on `48570ecc7` stopped before QEMU because `/tmp`'s
user quota refused a C compiler temporary file. An inactive Cargo-only cache
was checked for active processes/file handles and cleaned through Cargo,
reclaiming about 14 GiB; active work and unique proof packets were preserved.
The resumed build was deliberately stopped after review found that the guest
could exit after Enter press while the collector still sent a separate release
request. No emulator result was claimed. The corrected protocol batches both
transitions and requires the guest to consume release before continuing.

## Actual framebuffer result

Clean Source `4d7c3283a2a13810cd59a4bed25f899bbdb226af` passes the same supported
`cargo +stable xtask make conduitos compositor-semantic-recovery-proof`
entrance with outer exit zero. The collector requires guest exit 33, exact
Make/profile/recovery/capacity observations, one complete matching stale-Show
checkpoint and its actual guest press/release acknowledgement. The
[original packet](evidence/native-stale-show-qemu-20261023/) retains full serial
and QMP transcripts, build/image records, original PNG, compressed full build
log and independent verifiers/results. It uses evidence schema @4; previous
packets and their proof classes remain unchanged.

The real 1280×800 screenshot visibly contains `Stale: refresh` after 1,024,000
framebuffer pixel writes. The exact Face remains
`36b10b134d428664c71243ea52ba3dcd0cc666669aa3a104fbab978aaf0a195e`, revision 1.
The genuine old Show is distinct from the retained fresh Show; the typed refusal
is specifically `StaleShow`. The current Show, publication, source draft and
keyboard route remain intact with zero source-owner actions at that boundary.
The receipt retains the actual current Mask Plan and Play identities. Only
after capture and the validated PS/2 press/release pair does the existing fresh
typed interaction correlate and advance the canonical source draft.

The capacity cases still pass with the actual framebuffer wrapper: 48 valid
192-byte commands rasterize, the 49th/193-byte cases refuse atomically, and
seven uploaded invisible fixture surfaces coexist with the real Mask surface.
The ninth admission leaves full service cost unchanged, including guest entries
2,153→2,153; cleanup and canonical recomposition preserve semantic custody.
The Root-copy-loss recovery still passes its original independent conditions.

Independent verification binds the clean measured Source, actual ELF/ISO bytes,
unique complete serial receipts, exact QMP transcript and completed capture to
the actual native Make's unchanged 268,435,456-byte arena/ceiling. An independent
stdlib PNG reader verifies all chunk CRCs, decompressed extent, filters, RGBA
pixels and receipt hashes. PNG SHA256 is
`06bb657b272033294582ace97c287601e395ec1470d811919ecad9681faf8435`;
decoded RGBA SHA256 is
`7a57a886aac6e863ffc9da71be6c94a8042be4e9acc7db17fd3ac14df17b6e58`.
The original image was visually inspected. Strict owning-package native
all-target Clippy also passes with warnings denied; its full log is retained
in the packet. ISO SHA256 is
`c9d7921c1eba9d8a182da7b4321cbcaf610fd035319b5bad45d6bf176e836fad`.

This earns the explicit appliance's visible stale-Show case, not ordinary-product
provider disconnection, physical/human evidence, or the wide two-panel/four-choice
layout requirement. The screenshot's primary page shows three choices with
explicit continuation; it does not establish that all four are visible together.
#5608/#5994 and normal merged acceptance retain those separate obligations.

## Refreshed onto merged Source inspection

After #6274 merged, this branch merged `dev` b5fc3ae759cf9351fc807901fd48bb16c4d24569 without conflicts. Both actual submission regressions pass on source 132e114e2 after a fresh owning-package compile:

```sh
cargo +stable test -p conduitos --features native-compositor native_face_mask::submission_tests
```

The compressed `post-source-submission-tests.log.gz` retains this local result. This is compile/CPU regression evidence for the combined source; the earlier measured QEMU packet retains its original source identity and was not relabelled or rerun. Current Candidate and merge acceptance remain separate gates.

## Candidate evidence packaging correction

Candidate 39939155013 stopped at patch whitespace validation: original emulator serial CRLF bytes were interpreted as trailing whitespace. Both original capacity/stale serial transcripts are now retained losslessly as gzip archives, with independently checked decompression and original-byte hashes in `serial-compression.json`. No serial byte, source, image or proof result changed. The retained verifier scripts still target the original product output paths. The failed quick-gate log is retained beside the stale packet.
