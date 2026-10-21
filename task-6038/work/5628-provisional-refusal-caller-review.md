# Provisional input refusal must survive its caller

Root inspected selected-Look checkpoint d402ec749 on 2026-10-21. Source-level integration defect, not a QEMU reproduction.

The new select_owner_component / preference guard returns owner-look-provisional-input before any owner request. But product_front_door.rs F6–F9 dispatch handles every Err by setting owner_route=None, calling retire_owner_route and showing an owner refusal. retire_owner_route presents a fresh read-only Scene, dropping the draft. Therefore lower-level guard/input tests do not establish end-to-end draft retention.

Approved correction: typed OwnerSelectionError distinguishes ProvisionalInput from uncertain Route errors. Extract the coherent key dispatch/result disposition into a named owner_selection module, reducing the 947-line front door. Only the proven local pre-request variant retains route, acknowledged Show, draft and named focus; it presents a truthful local input refusal. Route failures continue retirement. Do not recover by matching arbitrary strings or call a local refusal an owner response.

Regression: run the production disposition helper with an actual pending native Mask and an opaque Some route sentinel. This can prove retention/drop behavior without pretending the sentinel is authenticated transport. Assert unchanged draft/Show/focus and route retention on the local variant; retain the real route-failure retirement path. Actual owner-backed F7 refusal/cancel/success still needs QEMU proof.

Separate fixture failure: a pre-swap CPU test initially called FaceArrival::present_owner_face, which emits privileged x86 UART output and crashed in hosted execution. Preserve that negative log. Use the existing legitimate CPU fixture setup through mask presentation and owner_face assignment, leaving the actual replace_owner_mask call under test. Do not upload core dumps or process environment.
