# Inspect logical execution beside its physical realization

Patchbay and Observatory keep the sealed logical Gear/port/Cord/source graph even
when a Host selects grouped dispatch. Physical fusion records refer to those
placements and Cords; they do not replace them. Installed failures use the exact
retained failing node or debug suspension to identify the logical Gear,
placement, authored Source document and span. A failure with no exact node keeps
Plan/fragment/Source context without attributing it to a nearby Gear. Unsupported
physical Back preparation also returns that sealed context.

The report now preserves the placement's capability limits, realization
characteristics, typed realization properties, pool references and terminal
transductions. Fragment rows retain the mandatory Sign storage budget, expected
Signs, shared pools, cancellation and terminal policies. The normal Patchbay
text also includes authority bindings alongside Host Calls and resources.
These are sealed bounds and selected facts, not fresh runtime measurements or
resource grants. In particular, the installed Near Back's existing typed
properties carry its retained scratch, input/output and numerical-work bounds;
inspection does not synthesize those values. Older serialized reports with no
admission facts decode as unknown rather than zero or an empty admission.

`PatchbayTopology::document_with_fusion_selection` borrows the planner's actual
`FusionSelection` beside the already inspected `OptimizedPlan`. It displays the
selected, admitted alternatives and rejected candidates, work/transfer byte
figures, supporting Sign identities and observation provenance. Work units are
explicitly planning evidence, not elapsed time. It verifies the selected Plan,
checked Source identity, groups, Gear/Host/capability/Boot and Line choices,
unique selected candidate and non-refused cost totals. It never replans, refreshes
a historical observation or stores another decision history. The existing
96 KiB report retention budget remains unchanged; borrowed decision text has a
separate 64 KiB formatting budget and shares the finite 256-line document limit.
Oversized text refuses while formatting rather than silently truncating facts.

The ordinary source-bound 32-Gear chain test checks both logical reports and
actual installed execution. Both realizations produce 33 from input 1; input
`u64::MAX - 2` fails exactly at the third authored addition with no external
output. A selected unavailable physical artifact refuses with exact Plan/Source
custody. The same test checks all original Gear ports/spans and Cord contracts,
selected/admitted/rejected explanations, wrong Source/Boot/cost refusal,
oversized explanation refusal, historical decoding and report serialization.
The separate configured Quantity/Near source test checks nonempty bounds from
the actual installed numerical offer through the normal Patchbay document.
Observation work values in the planning test are explicit assumptions, not a
new benchmark or hardware performance result.

```sh
cargo test -p conduit-patchbay-workbench --test pure_chain_fusion --test sealed_admission
cargo test -p conduit-patchbay-workbench --lib topology::tests
cargo test -p conduit-observatory --lib
cargo clippy -p conduit-observatory --all-targets \
  -p conduit-patchbay-workbench --lib --tests -- -D warnings
```

This is read-only Plan/Patchbay inspection and installed execution evidence.
It does not change authored arithmetic, typed pressure, logical kernel Signs,
authority admission, or the fusion eligibility boundary. Browser/native rendering
and shared dev/main publication remain separate checks; a local passing test is
not a claim of a published release.
