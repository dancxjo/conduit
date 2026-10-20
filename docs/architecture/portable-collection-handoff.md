# Portable collection handoff

This records the development contract for [C1 #6039](https://github.com/dancxjo/conduit/issues/6039),
consumed by [browser W4 #6041](https://github.com/dancxjo/conduit/issues/6041) and
[native N4 #6042](https://github.com/dancxjo/conduit/issues/6042). It describes the
implemented source/Face preparation profile, not accepted platform capability.
The [Face grammar](face-grammar-conformance.md) and
[Face/Mask/Show law](../presenter-hourglass.md) retain architectural ownership.

## Input and preparation

The source supplies one valid immutable Face, a collection subject identity and
a maximum item count from 1 through 64.
[`prepare_face_collection`](../../semantics/face/src/collections.rs) borrows that
Face and returns ordered source subjects, finite pages and proposal construction.
It does not create a domain selection or execute an action. A checked component
recipe may use `ComponentSubjectSelector::collection` with mode `information`,
`exclusive` or `independent` and its own `maximum_items` within the same bound.
The selected collection must satisfy that mode before the recipe is admitted.

The Crèche selection region binds the checked `independent` collection profile
with at most 16 items and one selected collection subject. Its existing region
groups and action-target field bindings remain unchanged. The exclusive naming
and informational review collections retain their ordinary complete generic
reading/Details path; they do not consume additional selection-region slots.

A collection has role `Collection`, unique direct `Contains` targets and exactly
one `item-count: Count` matching those targets. Each target is an existing source
subject, including an `Item` or domain `Plot`, with one distinct `order: Count`.
Order values may have gaps; vector position and label are not order or identity.
The view sorts by these facts. Duplicate relevant profile facts, repeated
members and ambiguous order refuse preparation. Empty informational and independent views are valid.

The Crèche producer uses a domain-separated, length-prefixed digest of the exact
Source-document/checked-Plot pair under the collection identity for Plot subjects, and a source option-ID digest for naming alternatives. Display
labels may coincide or change without becoming submitted values. These are
producer identity rules; a Mask must retain the supplied subject identities.

## Supported selection profiles

| Profile | Required source facts | Proposal rule |
| --- | --- | --- |
| Information | No `choice-multiplicity` | Collection selection refuses `NoSelectionAction`; other ordinary Face actions remain independently available. |
| Exclusive | `choice-multiplicity: Text("exclusive")`; minimum and maximum selected counts both 1; `selection-action` and `value-source` identities | Choosing one exact member proposes the source value-change action; deselecting it refuses `Selection`. |
| Independent | `choice-multiplicity: Text("independent")`; each item's `selected: Flag` and `boolean-change-action: Identity` | Propose the explicitly requested canonical Boolean replacement through that item's action. |

For exclusive choice, the value-source has `value-change-action` naming the
selection action and `current-value: TypedValue` with exactly its one argument's
checked contract. Each item has `choice-value: TypedValue` of the same value kind
and `selected: Flag` agreeing with the current canonical value. Exactly one item
is selected. Values are distinct and accepted by the full action contract.
A positive finite `CanonicalMembership` supplies the domain; every member allowed
by all intersecting constraints must be represented. An unannounced filtered
subset, unbounded domain or negative-only domain does not gain choice authority.
Always submit the action's actual argument name, kind and canonical bytes.

Independent choice requires one exact target-bound replacement argument with
kind `value/bool`, maximum one byte, admitting both `[0]` and `[1]`. The selected
fact and explicit action association must be unique. Availability is separate:
an unavailable action can retain the reversible contract without permitting
activation. A one-way action cannot become a checkbox. This profile refuses
minimum/maximum selected-count limits it cannot enforce.

Optional independent `inventory-count` describes the complete source inventory;
it must cover visible membership. Without it, inventory equals visible count.
Optional `selected-count` must lie between visible selected count and that count
plus hidden inventory. The bound subtracts visible items before adding selected
items, so valid `u64` inventory extremes do not overflow. Source query/filter
operations remain ordinary typed actions. The Crèche's `query-source` or
`source-query` describes its authored query, rather than a Mask rewriting
membership. Selected review remains an informational source collection.

This profile defines no additional generic grouping or collection-readonly
property. Structural grouping and other Face facts remain available through
the original Face. A renderer must not interpret an arbitrary property name as
new authority, erase unsupported meaning, or claim an unimplemented profile.

## View and interaction boundary

`page(first, maximum)` returns borrowed ordered items, the original Face identity
and revision, collection identity, total and optional next offset. `first` may
equal the length for an empty terminal page; maximum is from 1 through 64.
Paging, focus, hover, highlight and scrolling are local views and do not call
`interaction`. Native rows, browser popup space and input richness have their
own admitted limits; this source bound grants no Host capacity.

`interaction(current_face, current_show, item_id, choose, sequence)` constructs
a proposal using the retained source action and its actual typed argument. It
refuses a different valid Face identity/revision, unknown item, unsupported
selection or invalid current Show/action/value. Ordinary FaceInteraction and
source-owner validation still decide admission and commitment. Preserve exact
Face/revision/target/argument and Show identities across pending input; do not
resubmit a stale proposal against a replacement Show or resolve identity by
label or ordinal. Refusal and cancellation must leave domain state unchanged.

Preparation distinguishes `InvalidFace`, `MissingCollection`, `MissingFact`,
`AmbiguousFact`, `Type`, `Capacity`, `Membership`, `Order`, `Selection`, `Action`
and `UnsupportedProfile`. Proposal construction additionally distinguishes
`UnknownItem`, `NoSelectionAction`, `StaleFace` and wrapped interaction refusals.
Render these through the existing Face/refusal path or use truthful generic
fallback. Do not silently invent options, weaken membership or hide actions
because the selected Back cannot realize the richer view.

## Proof and downstream gates

The supported repository entrance is `cargo xtask check input-semantics`. Its
portable-collections step selects the Birth collection conformance modules.
The actual producer, independent test-only linear reader and separately observed
generic view live in
[`plots/birth/src/face`](../../plots/birth/src/face/collection_reconstruction_tests.rs).
They compare owner-derived identities, order, selection, exact values/contracts,
actions and availability. Variations include no selection, duplicate Unicode
labels, reversed storage and semantic order, and lost membership. Linear typed
properties retain their original checked contract and canonical byte vector
beside human wording; labels and an opaque Face digest cannot reconstruct them.

The retained reader is deliberately fixture-only: at most 64 members, canonical
values up to 128 bytes and JSON-compatible quoted Debug strings. It is neither
a production reverse parser nor an action execution engine. Source-derived
fixture Shows and checked proposals are not installed owner/Body Play evidence.

This held development branch still needs refresh onto accepted native stage05,
the final browser contract-source pin and producer specimen regeneration,
required current-tree checks, Candidate, Integration and stable acceptance.
A 16-item serialization fixture fitting 256 KiB does not establish native
storage, routed framing or observer-envelope admission. W4 and N4 must retain
one accepted source Face and demonstrate their actual selected Back/generic
fallback, exact acknowledged Show and Host/Plan/Play identities, bounded
resources and rejected actions with zero committed mutation. N4 follows
#5994's accepted finite native route; this contract does not reorder that queue.

The held canonical native five-candidate producer receipt at the pair-identity
fix measures 56,232 Face JSON bytes, exceeding the existing 49,152-byte snapshot
bound. [#6156](https://github.com/dancxjo/conduit/issues/6156) owns that separate
finite native capacity prerequisite after the existing #5994 route. The checked
collection selector resolves recipe membership, not storage or routed-envelope
admission; no native budget is raised by this preparation. C1 admission remains
held while the known native consumer capacity refusal is unresolved.

The Source producer lowers each collection's declared Count order into adjacent
`document/precedes` relationships from the same owner enumeration. The chains
connect only actual projected members within one collection; zero or one member
adds no edge. Generic aural traversal and checked collection Fore order now agree
across filtering, review, naming options and reordered storage. No native sorting
or identity-based geometry policy is introduced.

The refreshed canonical five-candidate CPU producer measures 64,530 Face JSON bytes.
The maximum 16-item selected long-Unicode Source fixture measures 201,468 bytes;
these are complete Face measurements, not observer or route envelopes. Earlier
56,232-byte and 174,810-byte receipts describe earlier Source versions. The
configured Crèche tests and the formerly failing one-Tab first-choice test pass;
the broader native consumer inventory retains ten failures involving capacity
or assertions on later branches. This remains held preparation under #6156,
not full native Host, emulator or stable acceptance.
