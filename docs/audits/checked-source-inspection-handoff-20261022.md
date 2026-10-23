# Checked Source inspection: bounded implementation handoff

Refs #6257, #5519 and #5994. This records current code seams and required
corrections; it implements no new runtime and proves no native admission.
#6257 remains unbranched until #6255/#6252 integration handoff.

## Preserve the existing slot bound

`targets/conduitos/application-core/src/lib.rs` fixes `SLOTS = 4`.
`native_workset/application_delivery.rs::initial` allocates the same fixed target
array and deliberately skips the inspecting Patchbay slot. In the default
four-resident Body this yields three other resident graphs and one unused target
position. A single explicitly authorized checked-Source asset could occupy that
unused position without increasing SLOTS, array/bitmap sizes or the declared
prepared graph capacity. This is a candidate preparation layout, not proof of
heap/encoding/graph bounds. If that slot is occupied or any budget fails, refuse
preparation; do not enlarge a global limit or fabricate an extra resident.
The existing initial selected resident must remain unchanged.

`protected_application/canonical_selection.rs::selection_target` maps actual
occupied target positions through a compact offered-action index. It does not
infer target membership from labels. Retain this law when the inspecting
application's numerical slot holds a Source asset: numerical equality to the
application slot must not be mistaken for execution/focus of that application.

## Source is not Playing or Lulled

`application-core::data::Target` currently requires `plan: PlanId`; builder
preparation passes it to `PatchbayApplicationPort::open_graph` or
`append_prepared_graph`. The portable retained inventory stores
`PatchbayActivePlot` with a mandatory Plan and Playing/Lulled state.
`projected_open.rs::projected_entry` explicitly labels prepared entries Playing.
Changing only the wire Plan field to optional would therefore still manufacture
execution state for unplanned Source. That shortcut is invalid.

Introduce an explicit prepared inspection target distinction preserving the
existing genuinely planned/active entrances. Checked Source retains its exact
source/checked/expanded graph identity and provenance; inspected Plan/Play,
Body membership and Body focus remain absent. Do not use a placeholder Plan,
Playing, Lulled or selected-body-focus flag to represent that absence. Update
portable canonical/linear/native descriptions together and preserve the existing
planned selection, Edit and Mask paths. Inspect all Plan accessors, topology,
inspection, resident projections and typed selection commands before choosing
the narrow internal representation; no broad public rename is prescribed here.

## Root must retain authority independently

`protected_application::Owner::new` currently copies immutable graph tables,
selected positions and kinds. It does **not** retain target Plan/effect policy:
those fields are moved into guest preparation. Graph identity alone will not
let Root independently reject an unplanned target falsely advertising an owner
effect. Retain a fixed immutable target-scope descriptor beside each graph,
including planned-versus-checked-Source identity and actual owner routes.
This is admission/authority evidence, not a second mutable application model.

Extend the real Root return admission, not only guest validation. Selection may
change the inspected graph after exact current Show/revision/action checks;
it must not establish execution or Body membership. Edit and Mask/lifecycle
effects require their actual routes independently of visibility. Keep existing
legitimate Body-wide Mask effects if their real owner route exists; do not
silently equate them with target-specific execution authority. Refuse forged
Source/Plan/target/proposal combinations before Root state changes and quarantine
invalid service returns as the current protocol requires.

## Finite proof before product claims

1. Admit one explicit installed Source asset (Vision is the checked 14-internal-
   Cord candidate) with exact provenance before Play. No ambient files/network,
   model access, executable eligibility or Creche starting-Plot addition.
2. Check complete staged preparation, transferred encoding, prepared graph/Face
   bounds and real private-heap high water including selection, publication,
   current refusal and the existing applications. Four slots fitting does not
   imply bytes fit. Preserve existing limits or file a scoped admission blocker.
3. Exercise positive existing planned paths and explicit no-Plan Source paths,
   stale/wrong/foreign/forged selection and effect cases through actual Root
   broker code, plus cross-Mask semantics. No hidden clone/rebuild during Play.
4. After normal integration handoff, enact the real Patchbay Body/Plan in QEMU:
   source selection/Details and both directions of >12-link continuation with
   exact endpoints/labels, truthful absent target Plan/Play, current Show/input
   custody and resource observations. Host/backend/model execution is unproven
   merely by inspecting checked Source.
5. Complete relevant exact-head checks and normal merge. Original #5519/#5358
   requirements remain authoritative; this handoff waives none of them.
