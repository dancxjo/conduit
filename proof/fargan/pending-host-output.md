# Public pending Host output under storage pressure

The ConduitOS `PendingHostOutput<BYTES>` component retains one actual computed
output plus original node/request in a fixed physical buffer. A failed store
keeps all bytes. A successful store retains the same lease across retries;
completion refusal does not discard it. Only actual accepted completion clears
the pending output. Explicit post-cancellation retirement releases any unowned
lease through the selected scheduler before clearing the retained material.
The buffer must be charged before computation. This component neither invokes
an owner nor steps a scheduler and grants no Source or clock authority.

Two tests and standalone Clippy passed against the pinned cached Kernel SDK.
The actual three-node scheduler test runs two original epochs uninterrupted and
with every output store forced to fail by filling the real fixed store. It
releases only its own pressure leases, then resumes the same pending Host call
and live scheduler without invoking its owner again. Outputs, decisions and
invocation counts match; terminal pending calls and storage are zero. The other
test checks capacity/occupied refusals and retention after completion/release
refusals. Project evidence is `outputs/fargan-public-pending-output/`.

This is a reusable public component, not yet the concrete trained FARGAN driver.
The general public AI custody/session adapter and complete working/preparation
inventory admission remain separate integration work. No full heap, pressure
resume equivalence for the trained model, target-runtime or listening claim is
made by these scheduler tests.
