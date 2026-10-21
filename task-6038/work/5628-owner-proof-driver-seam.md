# Owner-backed two-Look driver seam

Read-only preparation against `work/selected-typography-look`; no owner, credential, invitation, build, browser or guest was created. Execute only after the integration owner freezes the reviewed source and grants the relevant resource leases.

## Existing supported entrance suffices

Use the registered long-lived command:

```
cargo +stable xtask make conduitos live-owner-boot \
  --spore PRIVATE_PROVISIONED_ISO --candidate-id EXACT_CANDIDATE \
  --owner-forward PRIVATE_IPV4:PORT --qmp-socket PRIVATE_SHORT_DIR/qmp.sock
```

`targets/conduitos/make/xtask/mod.rs:205` defines these exact arguments. `owner_boot.rs::prepare/execute` checks the retained provisioned artifact/route, private IPv4 forward and per-connection `nc` backend; `demo.rs::boot_visible_image_with_network/qemu_args` supplies normal-product USB keyboard/mouse, GTK display, and serial stdout. A private external driver can launch this supported command, drain stdout/stderr continuously into bounded files, connect to its QMP socket, and retain command/source/manifest/image hashes. This avoids a new registered command or production producer extension. The existing visible entrance requires working graphical display/GTK; establish that precondition instead of silently substituting an unsupported headless launch.

QMP socket: absolute UTF-8 path <=96 bytes, no commas/control characters, nonexistent socket under existing private directory without group/other permissions (`owner_boot.rs::qmp_arg`). Keep invitation, TLS key and provisioned ISO private; export only reviewed sanitized records.

## Reuse, without inheriting incompatible lifecycle

`owner_action_proof.rs::prove` already demonstrates standby -> wardrobe coordination -> one F5 -> owner-acknowledged arrival. Its `wait_for_standby`, `wait_for_arrival`, `records`, and `bounded_serial` give concrete passive readiness predicates (120s arrival, 2MiB serial bound). `owner_todo_face_validation.rs::read_and_validate_current` independently reads the installed owner's actual Face and validates Body/item count/status.

Do **not** use `live-owner-todo-face-proof --coordinate` as a persistent session: its Todo branch returns immediately after acknowledged arrival and `execute_mode` kills/waits the guest. The Todo-action entrance additionally requires an isolated selected/protected checkpoint pair and executes a domain action; it is unnecessary for Look-only proof. The generic coordinated action runner is clock-specific after arrival. No extension is needed merely to keep a guest alive: `live-owner-boot` already does that.

Reuse the transport methods from `work/6158-runners-e1c6dda63/prove-5990-panels-qemu.py::Guest`: traced `request`, explicit key press/release pairs, newline-complete JSON records, passive bounded wait and `screendump` capture. Do not invoke its constructor: it boots an unprovisioned product without the authenticated owner route. Use a small external session adapter for the supported process/socket/serial instead. Retain finite command/reply/record/capture/serial limits and terminal cleanup; one controller owns QMP. No action is executed inside a wait predicate.

## Exact sequence and observation

Build/install/provision via `5628-owner-proof-sequence.md` and the existing native-spore/three-host guides. Browser joins before native sealing. Reuse wardrobe controls from `proof/browser/three-host-owner-journey.mjs:280–299`: identify exact available native route, Wear native-graphical, Doff the selected browser Mask, Prefer only native-graphical, then one F5. Preserve the owner Body Plan during wardrobe-only handoff; do not require the replacement Mask route Plan to remain unchanged. Replace that file's repeated action-performing Inspect loop with passive participant readiness, one Inspect, then passive completion; do not copy the retry loop.

After acknowledged arrival, independently snapshot installed Todo Face. `CONDUIT_NATIVE_COMPONENT_REALIZATION` already contains complete Face, exact revision, checked configuration, expression, fallback and acknowledged Show (`face_arrival.rs::acknowledge_owner_show`, <=65536B). Pair with `CONDUIT_NATIVE_OWNER_FACE` acknowledgement and `CONDUIT_CRECHE_FOCUS` current action/argument/page/hit rectangle. Derive the exact named argument from actual available actions; traverse Tab with a finite bound, not labels or array-order assumptions.

`product_front_door/owner_selection.rs::dispatch` maps F6=Compact+Flat, F7=Spacious+Flat, F8=Grouped (exclude), F9=scale and Shift-F9=forced colors. For each deliberate key, passively await a fresh realization+ack+focus record; require identical complete Face/action contracts and Flat recipe, distinct Show, retained action/argument identity, valid current hit and actual expression preferences. Capture actual pixels; compare density/palette truthfully.

On Compact, type a bounded ASCII draft with supported key mapping; capture the visible provisional value. One F7 must emit `CONDUIT_NATIVE_COMPONENT_SELECTION` refused/`owner-look-provisional-input`, produce no replacement acknowledgement, preserve current Face/Show and visible draft, and leave independent owner state unchanged. Escape cancels; one subsequent F7 succeeds. Do not claim native Unicode composition: seed long Unicode via an offered owner action before sealing. No domain commit is needed for this sequence. Exercise F9/Shift-F9 once per intended transition, restore actual preferences, capture Details/current owner snapshot, and hash records. Scalar/font observations are distinct from full shaping or human/physical proof.

Next safe action: review/implement only this private external adapter with synthetic transcript tests first; actual private setup and execution require root's source freeze/resource handoff. No new producer field or command is presently justified.

Replacement route identity clarification: a Look swap may legitimately reseal a different nonempty Mask route Plan. Bind each emitted route_plan_id to the independent owner wardrobe report/current acknowledged Show. Preserve owner Host and complete Face/Body/recipe; only the wardrobe-only handoff requires unchanged owner Body Plan. Require fresh focus after its matching ACK, not just after realization.
