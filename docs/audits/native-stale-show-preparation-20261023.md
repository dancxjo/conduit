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
through QMP before sending a real Enter acknowledgement, after which the original
fresh S2 interaction must still be accepted by the canonical owner.

This is an explicit freestanding emulator appliance with actual framebuffer
pixels, not the ordinary product binary, physical scanout or human observation.
The prior counting-target capacity/recovery packet remains unchanged. The
60-second emulator envelope and all scene/surface/arena limits stay unchanged.
No screenshot delay, retry, forced private state, injected scene error or
synthetic notice is used to manufacture the case.

Implementation is prepared. Both new submission cases, all 19 related native
Mask cases and all four collector/profile/capacity/recovery cases pass. The
[CPU packet](evidence/native-stale-show-preparation-20261023/) also preserves
the initial featureless invocation, which failed to compile existing tests
requiring `native-compositor`; the corrected invocation supplies that feature.
Fresh QEMU capture and relevant merged checks remain pending.
No visible stale-Show acceptance is claimed until the exact-source packet passes.
