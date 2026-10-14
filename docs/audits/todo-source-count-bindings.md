# Todo-owned typed facts for spoken slots

For #5657/#5658, the validated Todo projection now exposes remaining-count and completed-count as Count properties on todo/list, beside its domain revision. Counts derive from the entire actual state before disclosure or pagination; no consumer parses status prose or recounts a displayed page.

TodoFaceFacts privately borrows one validated immutable Face generation and its original list/count property references. It checks exact typed fields, bounds, explicit disclosure and original Source identity. Remaining item names must belong to that list, be incomplete and disclosed; pages contain at most three distinct items in source-owned order. A requested complete action must remain available, argument-free, disclosed and targeted at that exact item; foreign/stale/unsupported requests refuse. Values cannot be injected into a public slot struct.

Projection and binding fixtures cover 0/1/3/20 totals, a 3-remaining/17-completed list, Unicode names, read-only actions, missing/wrong typed/overflowed fields, corrupted identity, foreign items, stale action generation, duplicate/oversized/reversed pages and denied disclosure. These are policy/projection tests, not a fabricated current product or completed speech journey.

The checked spoken-template consumer, explicit English/locale policy and actual Source-pinned product matrix remain pending. This preparation does not close either issue and makes no Plan/Play/WAV/Show claim.
