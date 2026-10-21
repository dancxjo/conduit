# Native owner independent appearance selection — development progress

This extends #5628's owner control path to the same four finite Source profiles
already available locally. Shift+F6 toggles density; Shift+F7 toggles palette.
Both preserve the checked recipe. Existing F6/F7/F8 shortcuts and the text-size
and forced-colors controls retain their meanings. Graphics retain the accepted
4×4 coverage path and enlarged glyphs retain the bounded filtered coverage path;
this change does not alter their rasterization or assets.

The native sender, return-route decoder, and durable owner request now share
`ComponentMaskLook`'s four closed serialized names. Existing `compact` and
`spacious` strings retain their meanings. This is selection among checked Source
roots, not an extension of the visual declaration language. Browser selection
remains unchanged. The guest prepares a replacement, preserves only an exact
same-Face/same-producer unsubmitted encounter, retires the old Show, and
acknowledges the fresh Show before further input. No Face action or Body command
is added. The transport refresh limit, lifetime and action budget are unchanged.

The owner selection operation now requires its actual current Show acknowledgement,
using the existing full validation method. Previously it checked route/Show
shape without requiring this acknowledgement. A stale or merely available Show
cannot authorize another replacement.

## Validation and limits

- Closed wire names round-trip, retain the two existing names, cover both axes,
  and reject arbitrary names/objects: one focused Face test passed.
- Actual owner admission/seal/selection test passed: four profiles × two recipes,
  unchanged complete Face after each replacement, fresh Show identity, stale
  predecessor refusal, and refusal until the replacement is acknowledged.
  This uses the existing owner fixture; it is not a live Todo application capture.
- Native arrival tests: five passed, including both independent axis cycles,
  recipe preservation, local draft/focus transfer, owner read-only arrival,
  route loss and retirement behavior.
- Maximum Todo scene projection test covers four profiles × two recipes at
  320×240, forced colors and 200% text. It traverses all 40 available item
  controls, retains all item text, rejects duplicate traversal and preserves
  the complete Face. Result recorded in the accompanying log.
- Full native scene regression filter: 97 passed, zero failed or ignored,
  including the expanded maximum Todo profile matrix.
- Focused formatting and diff checks passed.

The retained logs are under `evidence/native-owner-look-selection-20261020/`.
These are hosted behavioral checks. They do not claim live owner QMP appearance
acceptance, attended human use, stable publication, full #5628 acceptance or
completion of #5994. Prior source-pinned local Crèche captures remain documented
in `native-look-selection-20261020.md`; they are not relabelled as this source or
as owner Todo evidence. Live owner appearance and semantic icon placement remain
open work.
