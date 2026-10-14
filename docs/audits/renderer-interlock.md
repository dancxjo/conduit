# Face to renderer correspondence audit

For #5642, the authoritative action reference is the exact Face content identity
and revision plus action identity and target. `FaceInteraction` transports that
reference with the exact Show identity and named typed arguments. Current code
has no separate renderer-owned Body action catalogue or standalone `ActionRef`
struct; renderer controls must resolve these existing fields.

## Ownership and checks

| Fact or boundary | Existing owner/check | Remaining proof |
|---|---|---|
| Subject, role, label, action target and availability | `semantics/face/src/face.rs`; `Face::resolve_action` | Product-specific Face construction must retain actual owner facts |
| Typed complete argument set | `interaction.rs::FaceInteraction::new`; human_interaction tests | Real adapter event must enter this admission path |
| Mask Source and checked/expanded identity | `mask_plot.rs::MaskPlot::admit`; common checked browser fixture | Two actual visual recipes with one unchanged Face |
| Mask Plan and renderer placement | `mask_show.rs::MaskShow::prepared` and planned Mask | Installed realization for each example |
| Acknowledged Show and output lifecycle | Show/Manifestation transitions; portable_renderer tests | Actual renderer acknowledgement rather than fixture transition |
| Reverse event correlation | `MaskShow::correlate_interaction`; exact Face/revision/Show fields | Actual DOM correspondence remains #5643 |
| Pressure, duplicate sequence, cancellation and failure | `interaction_ledger.rs`; human_interaction tests | Installed adapter pressure/cancellation evidence |
| Browser-local element IDs, focus and coordinates | Selected renderer adapter | These cannot create subject/action meaning or authority |

Paths above are under `semantics/face/src` unless a test is named. The table maps
code ownership; it does not certify an installed browser/native renderer run.

## Two presentation examples to prove

For one zero-argument `message/refresh` action targeting `patchbay/plot`, a compact
Mask can show a single **Refresh** button; a spacious Mask can place the same
button under a **Messages** heading with a current-state description. Their
geometry and wording around the control differ. Both must resolve the same
Face/action/target fields and return `face/interaction@2` with no invented empty
argument. Their Show identities and Mask Plans are distinct realization facts.

The shipped `human_interaction::zero_argument_action_needs_no_counterfeit_empty_input`
checks the action contract and refuses a counterfeit empty argument. These
presentation examples still need paired actual rendering evidence before #5642
can close. They are a proof target, not screenshots or implemented Look recipes.

## Exact correlation

`MaskShow` retains Mask Plot identity, optional producing Face Plot/Plan identity,
Face identity/revision, planned Mask and Show. Validation compares the original
Mask Source identity with the planned Mask, validates Show against the exact Face
and Plan, and recomputes Show identity. A resting Body Face may have no current
Plan; its Mask Plan is still exact. Reverse correlation refuses an interaction
whose Face, revision or Show differs.

Interaction admission requires current available action and Show, exact target,
complete typed argument set and finite sequence. The current shared ceilings are
64 arguments, 4096 bytes per argument and 8192 aggregate interaction bytes;
individual Face argument contracts can be stricter. Ledger evidence retains
lengths and identities without copying plaintext into evidence.

Changing style or local control IDs cannot repair a stale Face/Show reference.
A renderer may propose an event; the ordinary interaction and owner paths admit
and execute meaning. This audit does not move scheduling, authority or Body state
into inspection or the UI.
