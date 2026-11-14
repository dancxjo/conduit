# Original profile refusal after installed-resource wire reduction

The actual Source76 CPU harness now authenticates and retains the complete original command: 30,314 bytes within the unchanged 32,548-byte configured workspace. Resource-reference metadata is 46 bytes resolving the exact installed 6,030-byte resource table. No limit increase or record/profile weakening was made.

Full factory preparation still fails before constructing the kernel with `Workset(Fragment(Assignment(MalformedRecord)))`. The direct original strict decoder in the helper independently reproduces the same error under the actual generated profile.

The cause is **before record iteration**, not a malformed record payload. `architecture/assigned-plan/src/validation.rs` initializes `resources = [false; 32]` and immediately refuses `requirements.resources.len() > resources.len()`. The exact emitted `layout::RESOURCE_ORDINALS` contains **49** resource requirements (all local ordinal zero, one per applicable node). Native Host Call requirements are two; remote requirements zero. Therefore 49 > 32 reaches this guard before any assignment record is inspected.

A likely reusable repair is the same allocation-free representation change already used for computation inventory: retain the same 32 bytes but track one requirement per bit across the existing one-byte record-count domain. Multiplicity matters: 49 equal ordinal-zero requirements must still consume 49 distinct inventory positions; duplicate, excess, missing and changed records must retain their refusals. Do not deduplicate the required list or widen Host grants. This child made **no production change**.

Authoritative log: `outputs/positive-installed-resource-profile-diagnostic-20261114.log`, focused original Source76 test terminal failure after 15.11 seconds. Previous failure log `outputs/positive-installed-resource-preparation-test-20261114.log` is preserved. Child CPU cache/source lease is released. Only test helper diagnostics changed in this investigation; admission, Start, native calls and target heap fit remain unproved.
