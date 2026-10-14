# Todo-owned typed facts for spoken slots

For #5657/#5658, the validated Todo projection now exposes remaining-count and completed-count as Count properties on todo/list, beside its domain revision. Counts derive from the entire actual state before disclosure or pagination; no consumer parses status prose or recounts a displayed page.

TodoFaceFacts privately borrows one validated immutable Face generation and its original list/count property references. It checks exact typed fields, bounds, explicit disclosure and original Source identity. Remaining item names must belong to that list, be incomplete and disclosed; pages contain at most three distinct items in source-owned order. A requested complete action must remain available, argument-free, disclosed and targeted at that exact item; foreign/stale/unsupported requests refuse. Values cannot be injected into a public slot struct.

Projection and binding fixtures cover 0/1/3/20 totals, a 3-remaining/17-completed list, Unicode names, read-only actions, missing/wrong typed/overflowed fields, corrupted identity, foreign items, stale action generation, duplicate/oversized/reversed pages and denied disclosure. These are policy/projection tests, not a fabricated current product or completed speech journey.

The checked spoken-template consumer, explicit English/locale policy and actual Source-pinned product matrix remain pending. This preparation does not close either issue and makes no Plan/Play/WAV/Show claim.


The preparation now includes separate checked Todo brief/expanded Source producers with explicit bounded Count fields, English locale, version and Source/Plan identities. Their finite balanced count tables cover every admitted 0..20 value and preserve singular/plural wording; the Source returns an explicit count-admission result. Exact forwarded contracts are retained rather than flattened. Opaque bindings supply original names, counts and available complete-action IDs; either style retains disclosed essential warnings or refuses the bound. Unsupported locales and stale/hidden slots refuse. These are ordinary reproduced source/Plan preparation and portable-expression evaluations, not a Play/Show or TTS claim.

The complete Todo Face suite passes 15 tests and strict all-target Clippy. Actual updated product Face evidence and integration with the ordinary spoken Mask remain pending; neither issue is closed by these fixtures.
