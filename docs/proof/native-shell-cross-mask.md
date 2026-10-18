# Native shell and deterministic linear correspondence

Run the canonical product journey from a clean reviewed source revision:

```sh
cargo +stable xtask make conduitos graphical-profile-proof
```

This supported entrance builds the canonical x86_64 live ISO, boots one normal product guest, captures its actual VGA frames through QMP, and retains its serial and input logs. It requires the existing ConduitOS x86 build prerequisites and QEMU. The shorter `journey-proof` command currently builds an architecture proof appliance; its name alone must not be used to claim canonical live-product evidence.

The bounded journey births one Body, admits Wake, attempts Wake again while already Awake, then admits Plan and Play. The second Wake must produce a typed unavailable refusal with identical complete serialized owner projections before and after. A later valid action establishes recovery. The guest then exercises resident applications, Home, Patchbay, Face/diagram inspection, Stop, Home return and resident reentry. Every lifecycle receipt must retain the same Body identity.

Exactly one `CONDUIT_NATIVE_BIRTH` receipt must prove an admitted zero-Body to born-Body transition against its current source Face/Show and argument contracts. Its Body and born Sign must remain identical throughout later lifecycle records; navigation never counts as another Birth.

The guest emits `CONDUIT_NATIVE_FACE` with its full canonical Face and actual acknowledged Show identity, and `CONDUIT_NATIVE_ACTION` with source Show, action, terminal disposition and before/after owner state. The harness validates the Face content digest and exact within-path action/target/availability correlation. It feeds that same emitted Face into `render_linear_face`, preserving its basis, encounter context and action argument contracts. This is deterministic semantic reconstruction; it does not claim another enacted linear Mask Plan or acknowledged Show.

Outputs under `target/conduitos/x86_64/`:

- `journey-proof.json`: source, image, boot and Body binding.
- `native-shell-paired-linear.json`: full guest Faces, independently retained native Show identities, linear records and accepted/refused/recovered owner outcomes.
- `journey-serial.log` and `journey-qmp.log`: complete retained guest and input observations.
- `journey-frames/manifest.json`: source/image-bound real screenshots, serial offsets and adjacent native receipt identities. A restored application screenshot is explicitly distinct from the transient acknowledged Mask Show used to admit a lifecycle shortcut.

The normal product keyboard route currently does not offer semantic pointer actions. A source-equivalent installed browser route for this shell is also unavailable. Both are recorded as unsupported; parent #5356 acceptance remains open. Stale Show replay and wrong-target/argument cases are currently covered only by focused deterministic admission tests, while unavailable Wake is enacted in the installed guest. This does not complete #5605: its numbered real-journey step 5 separately requires an installed retired-Show/revision replay. The local-Birth product has no supported retained-interaction replay ingress; repeated keys construct fresh interactions. Retain that requirement as unresolved unless actual installed replay evidence is added or the issue scope is explicitly revised. No special replay callback, alternate owner or proof-only domain ingress is used. QEMU and deterministic reconstruction establish neither physical-device operation nor human enactment.
