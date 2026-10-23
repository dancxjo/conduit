# Native prepared-frame resource observations

Refs #5608 and #5994. The existing `CONDUIT_CRECHE_FOCUS` observation now records
command count, total command-payload bytes and largest command payload for its
current prepared Mask frame, alongside the exact existing 48-command and
192-byte limits. UTF-8 is counted in bytes, not scalar or glyph count.

The focus observer already constructed this frame to resolve the current hit
bounds. It now borrows that same frame for both bounds and accounting: no second
render, new renderer/model, mutable metrics state, new observation record or
limit increase. The receipt remains scoped to the current acknowledged Face/Show.
The extra fold is bounded by the existing 48-command scene capacity.

These are costs of the **prepared frame reconstructed for focus**, not measured
compositor work or a retained scanout-command history. They do not count retained
compositor surfaces; the existing eight-surface admission stays independent.
No scalar count, encoded scene byte count, timing or pixel-work inference is
made from command-payload bytes.

The normal layout/search proof collector requires cost fields on every retained
focus observation, enforces exact limits/consistent totals and emits high-water
figures without increasing the existing 256-record or 27-capture limits. It
refuses missing, nonnumeric, overbound or misclassified measurements. Older
Source-bound packets remain unchanged; these fields are not backfilled into
historic observations.

The native CPU boundary check passes at 48 commands × 192 UTF-8 bytes, with
atomic over-capacity refusal leaving counts unchanged. The collector CPU check
passes missing/overbound/nonnumeric and incorrect-scope rejections. Workspace
formatting and whitespace checks pass. Fresh ordinary QEMU enactment is pending.
Scene-pressure/large-graph acceptance, combined-head integration and normal
merge remain separate requirements; this observation does not grant authority
or claim that pressure/fallback was exercised.

## Ordinary QEMU result and ledger

Clean Source `461770828` passes the supported ordinary command:

```
cargo +stable xtask make conduitos creche-controls-proof --viewport 640x480
```

The [original packet and independent ledger](evidence/native-prepared-frame-budget-qemu-20261022/)
retain all 247 focus observations, complete serial/QMP logs, exact product build
manifest, two focused CPU logs and 17 original captures. Every observation has
valid exact-limit cost fields. Independently recomputed peaks match the receipt:

| Prepared Mask-frame measurement | Observed peak | Existing limit |
| --- | ---: | ---: |
| Graphics commands | 32 | 48 |
| Largest command payload (bytes) | 192 | 192 |
| Sum of frame command payloads (bytes) | 680 | No separate observed-total limit claimed |

These independent maxima need not occur in the same frame. The ledger retains
exact observation indices, Face/revision/Show and page identities for each peak.
The existing eight-surface contract is linked as a static admission fact;
actual retained surface count remains explicitly unobserved.

All 17 PNG lengths, hashes, chunk CRCs, dimensions and independently decoded
RGBA hashes pass. Visual inspection confirms the exact accented name, focus
and four selected Plot controls remain readable at 640×480. Natural incomplete
input refuses visibly without owner effects; the same guest completes exact
Unicode naming, two-way traversal before/after editing, primary/Details page
traversal, naming choice, supported pointer focus and restoration. Outer command
exit is zero. No new observation record, frame render or bound increase was
needed for the accounting.

This reaches the actual 192-byte command-payload boundary in prepared native
frames. It is not deliberate over-capacity scene execution, scanout history,
retained-surface measurement, physical evidence or a performance benchmark.
Combined-head CI/normal merge and the remaining original #5608/#5994 acceptance
stay open. Inactive appliance compilation caches were cleaned separately,
recovering about 3.5 GiB net headroom; unique source/evidence/ELF/ISO were retained.
