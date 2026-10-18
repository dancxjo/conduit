# Host completion Sign refusal preserves cancellation

Development proof slice of #5231, 17 October 2026.

Host completion admission reserves its mandatory Sign before consuming an
undispatched cancellation obligation or installing the completion. This ordering
is especially important after the indexed cancellation-count/search changes:
a rejected completion must leave its exact cancellation pollable.

The new production-kernel regression creates two dispatched requests and commits
cancellation Steps for both. It injects an exhausted Sign sequence counter, then
attempts the first completion twice. Both attempts refuse with SequenceOverflow;
the undispatched cancellation count stays two, all pending slot identities stay
unchanged, neither completion is installed, both cancellations remain requested
and undispatched, the waiting nodes stay unready, and value occupancy and Sign
entries remain unchanged. The Sign counter is restored after this fault injection.

Two continuations exercise cancellation-first and completion-first retry. The
accepted completion retires only its own obligation; the other node's exact
cancellation still dispatches once. Both completions resume and rearm request
one, then global cancellation empties pending/value storage. The test also checks
that a repeated cancellation poll produces no duplicate obligation.

A deliberate temporary mutation moves the cancellation-count decrement ahead of
Sign admission. The regression then fails at one remaining obligation versus
two expected. This is a test-sensitivity mutation, not a claimed historical bug.
Production source is restored byte-for-byte afterward.

The focused regression passes locally under both packed and linear ready
selectors; strict all-target kernel Clippy, formatting and diff checks pass.
No production behavior changes in this slice. No CI dispatched or awaited, and
no stable/physical/browser acceptance or parent completion is claimed.
