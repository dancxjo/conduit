# Native shell and deterministic linear correspondence

Run the canonical product journey from a clean reviewed source revision:

```sh
cargo +stable xtask make conduitos graphical-profile-proof
```

This supported entrance builds the canonical x86_64 live ISO, boots one normal product guest, captures its actual VGA frames through QMP, and retains its serial and input logs. It requires the existing ConduitOS x86 build prerequisites and QEMU. The shorter `journey-proof` command currently builds an architecture proof appliance; its name alone must not be used to claim canonical live-product evidence.

The bounded journey births one Body, admits Wake, attempts Wake again while already Awake, then admits Plan and Play. The second Wake must produce a typed unavailable refusal with identical complete serialized owner projections before and after. A later valid action establishes recovery. The guest then exercises resident applications, Home, Patchbay, Face/diagram inspection, Stop, Home return and resident reentry. Every lifecycle receipt must retain the same Body identity.

The same guest also commits six physical-key editor updates through `a`, empty text, `c`, `ca`, `caf` and `café`. The committed editor receipt is validated against actual serial state and protected execution cost, and retained alongside the paired Face evidence before product sealing.

Exactly one `CONDUIT_NATIVE_BIRTH` receipt must prove an admitted zero-Body to born-Body transition against its current source Face/Show and argument contracts. Its Body and born Sign must remain identical throughout later lifecycle records; navigation never counts as another Birth.

The guest emits `CONDUIT_NATIVE_FACE` with its full canonical Face and actual acknowledged Show identity, and `CONDUIT_NATIVE_ACTION` with source Show, action, terminal disposition and before/after owner state. The harness validates the Face content digest and exact within-path action/target/availability correlation. It feeds that same emitted Face into `render_linear_face`, preserving its basis, encounter context and action argument contracts. This is deterministic semantic reconstruction; it does not claim another enacted linear Mask Plan or acknowledged Show.

Outputs under `target/conduitos/x86_64/`:

- `journey-proof.json`: source, image, boot and Body binding.
- `memory-lantern-native-edit.json`: actual committed Unicode editor value, owner and execution identities.
- `native-shell-paired-linear.json`: full guest Faces, independently retained native Show identities, linear records and accepted/refused/recovered owner outcomes.
- `journey-serial.log` and `journey-qmp.log`: complete retained guest and input observations.
- `journey-frames/manifest.json`: source/image-bound real screenshots, serial offsets and adjacent native receipt identities. A restored application screenshot is explicitly distinct from the transient acknowledged Mask Show used to admit a lifecycle shortcut.

The normal product keyboard route currently does not offer semantic pointer actions. A source-equivalent installed browser route for this shell is also unavailable. Both are recorded as unsupported; parent #5356 acceptance remains open. Stale Show replay and wrong-target/argument cases are currently covered only by focused deterministic admission tests, while unavailable Wake is enacted in the installed guest. The #5605 scope clarification of 2026-10-18 follows its negative-test matrix: stale Show/revision and wrong-target/argument rejection are deterministic admission evidence; the installed QEMU negative is unavailable Wake followed by fresh recovery. Installed retired-Show replay is not claimed or required for this slice. The local-Birth product has no supported retained-interaction replay ingress; repeated keys construct fresh interactions. No special replay callback, alternate owner or proof-only domain ingress is used. QEMU and deterministic reconstruction establish neither physical-device operation nor human enactment.

The current CI product route can also be reproduced with a fresh output directory:

```sh
cargo +stable xtask make host build targets/conduitos/profiles/conduitos-native.host.conduit --output target/native-face-product
cargo +stable xtask make host verify target/native-face-product --boot
cargo +stable xtask make host verify target/native-face-product --journey
cargo +stable xtask make host verify target/native-face-product
```

This verifies the retained normal Host product and its source/image manifest. It does not use the architecture-only journey appliance. The journey itself uses one fresh guest boot; the separate boot check is not another chapter of that Body's journey.

## Accepted product evidence

The retained normal-product bundle in [release-4c4cd59a4a66f5d853da7aff97ee92d9b16ceb74](https://github.com/dancxjo/conduit/releases/tag/release-4c4cd59a4a66f5d853da7aff97ee92d9b16ceb74)
was tested by [Integration 39149303771](https://github.com/dancxjo/conduit/actions/runs/39149303771).
Its publisher manifest separates tested source `4c4cd59a4a66f5d853da7aff97ee92d9b16ceb74`
from accepted main `8729ac31cf42966d9e99d879b1afee9cf258e383`; both trees are
`ede40074b4cfd3857c142351cb8c3335440f8844`.

A fresh retained-artifact audit verified all 30 x86 target file hashes and 17
screenshot hashes, 27 paired native/linear Faces with equal semantic actions,
basis and encounter context, one Birth, four accepted lifecycle actions, one
zero-effect unavailable refusal with unchanged complete owner projection, and
fresh accepted recovery. The committed Unicode receipt passes its captured-session
validator. This audits existing CI QEMU evidence; it is not another guest run.
The ISO SHA-256 is `3c4afd6e658eebbad5d32f22ff56df4b43bd12e5368efdd1671baecf86ed5465`.
The bundle explicitly retains the unsupported pointer/browser portions and absent
physical/human evidence described above; parent #5356 remains open.
