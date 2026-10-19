# Portable Mask control reuse audit

Development preparation for [#6038](https://github.com/dancxjo/conduit/issues/6038),
starting with [C1 #6039](https://github.com/dancxjo/conduit/issues/6039).
This record does not establish component-Fore, browser Back or native acceptance.

| Existing source | Reusable contract | Limit or missing proof |
| --- | --- | --- |
| `plots/todo/face` | Collection/Item containment, stable item IDs, source order, completion flags, exact complete/reopen/remove actions, bounded add text | No exclusive-choice domain action; do not invent one for this specimen. |
| `plots/birth` | Source-owned naming tradition, finite checked membership, independent reversible Plot selection, names and search | Naming labels and selected value currently have prose projection. Search is an explicit source operation, not Mask-local filtering. |
| `semantics/catalog/src/saved_choice.rs` | Bounded option ID/label pairs, retained identity before emission | A saved-choice Host Back is not an automatically selected Face component. |
| `semantics/face/src/readable_choices.rs` | Conservative display hint for small plain UTF-8 memberships | Only one positive membership constraint, up to eight values/128 total bytes; refusal of a display hint does not narrow the source contract. |
| `targets/browser/handbook/owner-face-component.mjs` | Native inputs/selects, local drafts/composition, membership validation, closed-profile exclusive groups | Registration and a custom-element surface alone do not prove selected Back identity or the portable component Fore. |
| `face/editor`, documented in `docs/input-semantics.md` | Revisioned Unicode scalar replacements and provisional edits | Atomic named arguments remain distinct from document editing. Host IME/grapheme capabilities need separate proof. |
| `ApplicationComponent`, `semantic_ui` | Migration evidence for select/option and choice contracts | Do not maintain a second stable application schema or widget ontology. |

## First failed reconstruction: filtered Crèche item identity

The source `BirthPlotChoice` contains a checked Plot identity. The intermediate
`ChoiceOption` retains it independently of its label. Previously the canonical
Face projection replaced it with the option's *filtered position*. Filtering the
second inventory item to position zero changed its subject identity to the
first item's old identity. Duplicate labels could not recover the original
identity from the Face. Exact Show/revision validation still fenced old actions;
this was a projection identity defect, not evidence of an accepted stale effect.

The bounded correction derives each Plot subject ID from the checked Plot
identity, scoped to the encounter and collection. A SHA-256 suffix bounds the
identifier length. An existing `Identity` property named `checked-plot-id`
retains the original source identity for independent reconstruction. Existing
Contains, selected Flag, availability, action target and Boolean argument
contracts suffice; no Face grammar extension is needed. Source order stays in
the ordered subject/relationship encounter; it is not encoded in item identity.
Current source actions may have revision-scoped inventory indices; they must
always be resolved from the current Face and never reconstructed from labels,
subject suffixes or visible positions.

The focused corpus covers duplicate Unicode labels, source inventory reorder,
filtering the second item to the first visible position, empty results, disabled
items, exact source selection, forged target and stale Show refusal. Its Mask
acknowledgement is simulated deterministic conformance, not actual Host delivery.
The committed prebirth Face specimen is regenerated from the existing domain
producer and remains shared by browser/linear conformance.

## Handoff boundaries

C1 still needs reusable checked Fore consumption and the complete collection,
exclusive-choice and independent-choice corpus. Naming tradition is a real
exclusive source action; it must not be replaced by a new invented Todo action.
C2 owns atomic text/editor/suggestion composition. W4 consumes those portable
contracts after the selected Web Component foundations; it must retain current
trusted browser binding validation. N4 follows #5994's serial native acceptance,
using the same source specimen. This change touches no native scene or renderer.

A linear or spoken realization must preserve exact source IDs, action/value
contracts and refusal reasons without requiring popup or widget behavior.
Browser keyboard/IME/accessibility, actual QEMU, finite resource/latency receipts,
Candidate, Integration and stable publication remain separate required evidence.

## Local preparation evidence

On the unmodified development source, the focused filtering regression fails:
the same source action's target changes from `initial-plots/1` to
`initial-plots/0`. With this correction, all 31 Birth tests pass (five new
identity regressions), and Birth all-target Clippy passes with warnings denied.
This is deterministic software conformance. It is not Chromium, QEMU, accepted
component-Fore, Candidate, Integration, release or physical proof.

Following the user's explicit ownership handoff, this delta is also reconciled
locally with #6043's committed Crèche source and the #5624 required-slot
preparation. The original branches and proofs remain intact. Admission still
waits for the normal repair and exact Candidate/Integration gates; this is not
a competing native scene PR. #5651/#5652 remain separate assembly work. N4
follows #5994's serial native route. Parent #6038 and C1 #6039 remain open.
