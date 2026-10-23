# Native checked Crèche search proof

Refs #5608 and #5994. The existing canonical live-image proof entrance now has
an independent finite search scenario:

```
cargo +stable xtask make conduitos creche-controls-proof --viewport 640x480 --scenario search
```

The default `layout` scenario preserves the existing invocation. Search uses a
fresh actual guest so its observations fit the existing 256-record and
27-capture bounds. It changes no product state machine, image catalog or limit.
The actual initial catalog must contain more than four offered Plot choices;
the proof records all choices and refuses a smaller inventory rather than
manufacturing additional ones.

Actual keyboard drafts must preserve the complete source Face and acknowledged
Show before Enter. Each Enter must produce one exact checked `creche.search`
interaction and a fresh source publication/acknowledged Show. The source's
Patchbay filter must retain its exact offered action, selection, label and
availability. An unmatched query must remove editable choices while retaining
all selected Plot review text at primary disclosure. Name and naming tradition
remain exact. Explicit Backspace stages an empty replacement query; committing
it must restore the full original inventory and control order. Keyboard focus
traverses all remaining controls forward/backward under the filter and again
after restoration. Every primary page and clamp is observed; sampled original
PNGs retain the existing custody. Exactly three search actions and no selection
changes are allowed.

Harness compilation passes. QEMU enactment and original capture verification
are pending for the clean committed Source. This scenario does not establish
provider-loss fallback, arbitrary query Unicode/IME, physical/human enactment,
combined-head CI or merge. Those remain separately tracked.

## Real QEMU result

Clean Source `bc3ff641a` passes the supported command above. The
[original packet](evidence/native-creche-search-qemu-20261022/) retains 11 PNGs,
full serial/QMP observations, exact native build manifest and typed receipts.
The actual inventory contains five choices, four selected. Three and only three
source changes commit `Patchbay`, `zzzzzz`, and empty queries. Every draft
keystroke preserves the exact source Face/Show; name, tradition and all four
selected-Plot review clauses remain exact. The matching inventory contains
only the original offered Patchbay action with unchanged selection and
availability; the empty-result inventory has no Plot actions. Clearing restores
all five source choices and the original control order. Complete forward and
reverse keyboard cycles pass both under the match and after restoration.

All 11 original PNG lengths, SHA256, chunk CRCs, dimensions and independently
decoded RGBA hashes pass. Visual inspection shows the exact committed query,
accurate `Selected: 4` despite filtering, and all four selected Plot names in
the primary review on page two. Both query states have four primary pages;
every page/clamped endpoint has an acknowledged focus receipt. The retained
first/middle/last screenshots do not include page three: no pixel claim is made
for its matching checkbox row, whose typed Face and actual focus are retained.

The first run `d08759af4` failed a harness assertion that incorrectly required
explicit Primary disclosure entries. Canonical Face omission means Primary;
[negative evidence](evidence/native-creche-search-disclosure-20261022/) remains
retained separately. Correcting that assertion changed no product behavior or
limit. The successful run is emulator evidence, not physical/human enactment.
Provider-loss/resource-pressure acceptance and combined-head CI/merge remain
open. Harness compilation and workspace formatting pass; no new Actions run
was queued for this checkpoint.
