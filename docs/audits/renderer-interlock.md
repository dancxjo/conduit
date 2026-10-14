# Face to renderer correspondence audit

For #5642, the authoritative action reference is the exact Face content identity
and revision plus action identity and target. `FaceInteraction` transports that
reference with the exact Show identity and named typed arguments. Current code
has no separate renderer-owned Body action catalogue or standalone `ActionRef`
struct; renderer controls must resolve these existing fields.

## Ownership and checks

| Fact or boundary | Existing owner/check | Evidence / remaining gap |
|---|---|---|
| Subject, role, label, action target and availability | `face.rs` and `semantics.rs::Face::resolve_action` | Product-specific Face construction must retain actual owner facts |
| Typed complete argument set | `interaction.rs::FaceInteraction::new`; human_interaction tests | Real adapter event must enter this admission path |
| Mask Source and checked/expanded identity | `mask_plot.rs::MaskPlot::admit`; common checked browser fixture | Paired compact/spacious terminal test below |
| Mask Plan and renderer placement | `mask_show.rs::MaskShow::prepared` and planned Mask | Paired test executes ordinary terminal planner/kernel realization |
| Acknowledged Show and output lifecycle | Show/Manifestation transitions; portable_renderer tests | Paired test checks Write/flush acknowledgement to named memory sinks |
| Reverse event correlation | `MaskShow::correlate_interaction`; exact Face/revision/Show fields | Actual DOM correspondence remains #5643 |
| Pressure, duplicate sequence, cancellation and failure | `interaction_ledger.rs`; human_interaction tests | Installed adapter pressure/cancellation evidence |
| Browser-local element IDs, focus and coordinates | Selected renderer adapter | These cannot create subject/action meaning or authority |

Paths above are under `semantics/face/src` unless a test is named. The table maps
code ownership; it does not certify an installed browser/native renderer run.

## Two native presentation examples

The paired proof uses one current `start` action targeting `clock`, with zero
arguments. A compact 40-column terminal Mask wraps the same description and
Start action over more rows; an 80-column Mask displays the action on one row.
Both retain the same Face/revision/action/target reference and return
`face/interaction@2` without a counterfeit empty argument. Their Show identities
are distinct per-Play realization facts. Their sealed Mask Source and Plan stay
unchanged while renderer-local viewport dimensions differ.

The shipped `human_interaction::zero_argument_action_needs_no_counterfeit_empty_input`
independently checks the zero-argument contract and rejects counterfeit material.
The new paired test exercises its actual terminal return path. The minimum-width footer now splits keyboard help
over two rows instead of
clipping inspect/apply/cancel instructions; content pagination reserves those
rows and keeps the admitted terminal extent. This does not certify all browser
or graphical accessibility.

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

## Paired native terminal proof

`targets/std/src/terminal_mask_execution/tests.rs::compact_and_spacious_presentations_return_one_unchanged_face_action`
prepares 40- and 80-column terminal views of one unchanged clock Face. It renders
both through `HostedTerminalMaskExecution`, flushes to explicitly named in-memory
sinks, and requires Available Shows only after acknowledgement. Actual output
bytes differ: the compact view wraps the description and action across more rows.
Both activate the existing `start` action targeting `clock` through the renderer's
ordinary typed input and the planned interaction Fore. Their Face/revision/action/
target references agree; their Show identities differ across the two Plays.
The sealed Mask Plan identity remains unchanged. No interaction remains pending.

Printed evidence retains producing Face Source, Mask Source, Mask Plan, Face
revision, both Show identities, return kind and both actual output buffers.
The producer Face basis is a checked fixture; execution and terminal flush use
real planner/kernel paths. This is native conformance with a memory output sink,
not a human terminal, graphical screenshot, packaged browser or physical proof.
Browser reverse-event correspondence remains #5643.
