# Native shell / Face correspondence audit (#5603)

Review source: `7989c14248d479715b5bd6e5379da11eb617fa87`.
The existing #5355 Face conformance contract remains the semantic authority.
This audit compares the native entrance's current semantic projection with its
shown scenes; it does not introduce another Body store or rewrite layouts.

| Surface / scene branch | Current semantic owner | Correspondence or explicit unsupported scope |
| --- | --- | --- |
| Pending join | `PendingJoinView::face` and the production Face-arrival route | `FrontDoor::scene` has no dedicated joining branch. Its generic scene is not a joining correspondence proof; the product Face-arrival route forwards the actual pending/owner-admitted Face. |
| Zero Body / World | `FrontDoor::face`, host and entrance Plot subjects, `semantic_actions` | Host/Plot identity, opened property and Open/Birth availability exist. Scene uses local selection/details labels and key hints; F2/F9 and selection are renderer/context controls rather than extra lifecycle actions. |
| Open Plot | Same Face; checked/source identities and Open/Birth availability | Scene prints embedded provenance and uses the actual Birth action's local key binding/availability label. Inspection creates no Body. |
| Exact details | Host properties plus current journey identities | Profile/build/image are disclosed only while details are open. Detail pages additionally show current lifecycle identities; formatting belongs to the scene. |
| Home | Current journey Body plus `conduitos/shell/home` Region, view and selected index; shared `HomeModel` | Launcher destinations, command output and key hints are not individually represented as Face subjects/actions in `projection.rs`. They are currently shared Home/ApplicationView context, not evidence of complete Face action coverage. |
| Crèche | `Arrival::face` and shared draft/widget Face | Arrival scene consumes `draft.widget_face`; fields, availability and validation belong to the shared birth model. Width/height below 480 explicitly refuse scene layout. No Body exists before admitted birth. |
| Body workspace | Observed `JourneyProjection`, exact resident `WorkspaceProjection`, optional validated `ApplicationView` | Scene labels/status derive from current journey/foreground title. Application rows come from the ApplicationView. Scene-level navigation is context; lifecycle availability remains in semantic actions. The foreground resident title is not currently a named Plot subject in the entrance Face. |
| Connectivity | Current Body plus Line subject/status/value properties | Scene's compact connectivity branch shows the same observed line identity/status/value. |
| Patchbay | Current Body plus `graph_projection::append` exact graph subjects/relationships and Source/checked/expanded identities | Entrance workspace scene receives graph availability only; the graph-specific native Face compositor owns actual diagram layout. A generic workspace scene is not a diagram correspondence proof. |

`observe_product` obtains the authoritative journey/workspace, validates its
ApplicationView and updates graph projection from that same product observation.
`observe_body` checks membership, current host and revision before acceptance.
Home contains `HomeModel` and keymap state. These are context, not a second Body
lifecycle store. A renderer scene is not allowed to mint lifecycle identities.

The focused `front_door::shell_audit_tests` fixtures capture concrete Face revision, shown text,
intent/availability and unsupported context for zero-Body, Home, Crèche, Body
and Patchbay fixtures. Remediation belongs to #5604 and actual QEMU/cross-Mask interaction to #5605. All 39 front-door tests passed, including the four new audit tests covering
five named states. Their concrete Face identities/revisions, actions and shown
text are retained in [fixtures.json](../../proof/native-shell/fixtures.json).
The selected-graph fixture separately checks exact Gear/Port/Cord projection.
This run used the default feature profile; it does not claim native-compositor
diagram, QEMU, physical display or cross-Mask interaction acceptance. #5603
closure still waits for exact-head admission; #5356/#5604/#5605 remain open.
