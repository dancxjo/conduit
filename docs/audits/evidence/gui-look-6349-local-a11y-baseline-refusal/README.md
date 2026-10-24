# Retained local accessibility baseline-check refusal

Clean Source 5517bd1d5 built and booted the normal product at 640×480.
The controls harness refused its first F6 baseline selection because its
expected identity was still conduit.face/spacious-phosphor@1, while the
actual checked Source and guest record use conduit.look/phosphor-spacious@1.
No accessibility control or name edit was exercised. Exit 1, complete logs,
actual bootstrap/Show records and failure PNG remain retained.

The correction updates the two explicit expected Look identities to the
canonical resources. It does not accept arbitrary identities or remove the
configuration, exact Face, Source/Show/Boot, action and fallback guards.
Five focused private-guard cases now pass, including replay of the actual saved
guest selection, retired-identity negatives and the existing stale/foreign/
configuration/structure guards. The probe extracts the actual helper Source
and imports its actual unit cases, linking the current Face/JSON libraries; it
is not a full xtask unit-suite or CLI receipt. Source/probe hashes and the
archived work-context probe are retained. This failed QEMU run is still not
accepted accessibility proof.
