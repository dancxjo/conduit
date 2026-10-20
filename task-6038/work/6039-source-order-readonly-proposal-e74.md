# C1 Birth collection order: read-only cause/proposal

Inspected held collection-source e74d6f060 and retained native inventory result; no Source edits, tests, builds, guest, or GitHub changes.

## Actual defect

`outputs/6039-fore-consumer-e74/native-face-inventory.log:150-155` is an independent ordinary navigation failure: one Tab after name/value reaches creche.plot.2 rather than creche.plot.0. It is not a capacity failure and should not be waived under #6156.

- `plots/birth/src/face/projection/collections.rs:74-83` emits unique Count(order) facts from the owner's draft.choices order for projected inventory items. Review items likewise carry that Source ordinal.
- `semantics/face/src/collections.rs:153-168` admits the scoped order and sorts checked collection items by it.
- `semantics/face/src/aural.rs:249-287` starts with lexically sorted subject identities; only existing relationship/composition precedence changes that ordering. It does not interpret a collection's order property.
- `targets/conduitos/src/native_face_scene/document.rs:80-118` consumes canonical aural Subject provenance, rather than reconstructing application-specific order.
- Opaque full Source/check-pair item IDs are correct identities, but their lexical order is unrelated to source collection order. Previously ordinal-looking identities accidentally gave the desired fallback order. That coincidence has disappeared.

## Smallest Source-only correction

Reuse the existing `FaceRelationshipKind::Semantic(kind_id("document/precedes"))`, already authored for navigation→article in `plots/birth/src/face/projection.rs:62-72`. In the collection projection's same owner-order loop, remember the previous *actually projected* inventory member and emit previous→current for each subsequent member. Derive Count(order) and the relational fallback from that one enumeration. Do the same for the informational reviewed-plots collection, using its existing selected-member loop. Keep IDs, membership, source action mapping, values, selection and available actions unchanged.

No new order property, semantic Kind, native application sort, identity prefix, DOM-order convention or generic reader redesign is needed. A chain among N distinct members needs max(N−1,0) edges, not an all-pairs order. For two collections bounded by M owner choices, at most2max(M−1,0) additional relationships. These edges express Source reading meaning across unfamiliar Masks; they contain no host or geometry facts.

## Invariants and risks

- Scope each chain to one collection's actual Contains members. Filtered-out members must not appear as dangling endpoints. Carry the prior visible member across gaps: visible source orders0,3,5 require0→3→5, not missing0→1 edges.
- Empty/singleton collections emit no edges; review selection can have noncontiguous ordinals. Numeric order need only remain unique; do not renumber selection or change option identity to make it contiguous.
- Adjacent edges strictly increase the same owner ordinal, so no self-edge or cycle arises. Existing Contains edges point parent→member→details, not back to a sibling. Do not add member→collection or reverse edges. The generic aural cycle fallback is deterministic, not validation of a contradictory Source order; do not rely on it.
- Count(order) and precedes become redundant representations, but need not become two truths: both must be derived from the same immutable owner enumeration in one projection responsibility. Do not retain an independently editable permutation. A focused correspondence assertion should bind prepared collection order to the canonical aural Subject order, filtered to those members.
- This changes Face content/hash and adds relationship clauses/bytes. Existing #6156 admission/capacity work remains separate and must account for the real enlarged Face; do not shrink away exact facts or declare capacity failures fixed by this order correction.
- The chain orders collection members; it does not create a new global field-layout order. Preserve existing navigation/article and field identities. The existing ordinary name→first-choice regression should verify the relevant surrounding reading boundary without introducing a native-specific sorter.

## Focused proof to reuse/propose (not executed)

1. Reuse `ordinary_name_entry_reaches_first_choice_with_one_tab`: same actual generic Birth Face must reach creche.plot.0 and submit its one Boolean value.
2. Source projection test: canonical aural member Subject order equals prepare_face_collection member order for inventory and review, with opaque IDs deliberately out of lexical owner order.
3. Filtered/empty/singleton/selected-subset cases: exact member endpoints, N−1 edges, unique increasing source ordinals, unchanged identity/selection/action associations, no cross-collection edges.

No broad native layout work or new portable schema is justified by this failure. Root owns any edit and the exact proof/admission sequence.
