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
