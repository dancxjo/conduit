# Native shell and deterministic linear correspondence

Run the canonical product journey from a clean reviewed source revision:

```sh
cargo +stable xtask make conduitos graphical-profile-proof
```

This supported entrance builds the canonical x86_64 live ISO, boots one normal product guest, captures its actual VGA frames through QMP, and retains its serial and input logs. It requires the existing ConduitOS x86 build prerequisites and QEMU. The shorter `journey-proof` command currently builds an architecture proof appliance; its name alone must not be used to claim canonical live-product evidence.

The bounded journey births one Body, admits Wake, attempts Wake again while already Awake, then admits Plan and Play. The second Wake must produce a typed unavailable refusal with identical complete serialized owner projections before and after. A later valid action establishes recovery. The guest then exercises resident applications, a stale-revision refusal through ordinary keyboard input, Home, Patchbay, Face/diagram inspection, Stop, Home return and resident reentry. Every lifecycle receipt must retain the same Body identity.

The same guest also commits six physical-key editor updates through `a`, empty text, `c`, `ca`, `caf` and `café`. The committed editor receipt is validated against actual serial state and protected execution cost, and retained alongside the paired Face evidence before product sealing.

Exactly one `CONDUIT_NATIVE_BIRTH` receipt must prove an admitted zero-Body to born-Body transition against its current source Face/Show and argument contracts. Its Body and born Sign must remain identical throughout later lifecycle records; navigation never counts as another Birth.

The guest emits `CONDUIT_NATIVE_FACE` with its full canonical Face and actual serialized acknowledged MaskShow, and `CONDUIT_NATIVE_ACTION` with source Show, action, terminal disposition and before/after owner state. The harness validates the Face content digest and exact within-path action/target/availability correlation. It feeds that same emitted Face into `render_linear_face`, preserving its basis, encounter context and action argument contracts. This is deterministic semantic reconstruction; it does not claim another enacted linear Mask Plan or acknowledged Show.

Outputs under `target/conduitos/x86_64/`:

- `journey-proof.json`: source, image, boot and Body binding.
- `memory-lantern-native-edit.json`: actual committed Unicode editor value, owner and execution identities.
- `native-shell-paired-linear.json`: full guest Faces, independently retained native Show identities, linear records and accepted/refused/recovered owner outcomes.
- `journey-serial.log` and `journey-qmp.log`: complete retained guest and input observations.
- `journey-frames/manifest.json`: source/image-bound real screenshots, serial offsets and adjacent native receipt identities. A restored application screenshot is explicitly distinct from the transient acknowledged Mask Show used to admit a lifecycle shortcut.

The normal product keyboard route currently does not offer semantic pointer actions. A source-equivalent installed browser route for this shell is also unavailable. Both are recorded as unsupported; parent #5356 acceptance remains open. The installed negative matrix requires both unavailable Wake and a stale Face revision. For the latter, hold Shift in the actual keyboard Plot, leave the editor surface, open ordinary Face Details, and focus its source-offered Stop. Release Shift and press Enter in one USB report. The held release remains owned by the original Plot and advances the owner revision before the queued Enter reaches the retained Mask; its old Face/Show interaction must refuse without another owner effect. This uses normal product input and its bounded Service scheduling, with no special replay callback, alternate owner or proof-only domain ingress. Repeated ordinary shortcuts alone still construct fresh interactions and are not stale evidence.

The stale receipt retains the old available Face, actual MaskShow and typed interaction, the newer canonical admission Face, and identical complete before/after owner projections. The validator checks the original interaction against its source encounter, then requires canonical `StaleFace` against the newer admission Face. It independently reconstructs that admission Face's linear meaning, requires a fresh available Show after refusal, and requires a later accepted action. The eighteenth screenshot captures the real visible refusal. Wrong-target and argument negatives remain focused deterministic coverage. Retained earlier seventeen-frame evidence does not prove this additional installed case; acceptance requires a successful source-pinned eighteen-frame run and its actual CI/publication receipts. QEMU and deterministic reconstruction establish neither physical-device operation nor human enactment.

The current CI product route can also be reproduced with a fresh output directory:

```sh
cargo +stable xtask make host build targets/conduitos/profiles/conduitos-native.host.conduit --output target/native-face-product
cargo +stable xtask make host verify target/native-face-product --boot
cargo +stable xtask make host verify target/native-face-product --journey
cargo +stable xtask make host verify target/native-face-product
```

This verifies the retained normal Host product and its source/image manifest. It does not use the architecture-only journey appliance. The journey itself uses one fresh guest boot; the separate boot check is not another chapter of that Body's journey.
