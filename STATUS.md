# Current project status

Reviewed against the development tree and open issues on **8 September 2026**.
This is a capability summary, not a claim that every check has been rerun today.
Published products and visual evidence follow accepted releases and can lag
`dev`. The [recorded acceptance history](docs/history/accepted-milestones.md)
preserves the original proof receipts and limitations.

**[Watch the current ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)**
for the clearest demonstration of the system so far.

## What exists

| Area | Available behavior and evidence | Boundary |
|---|---|---|
| **Language and composition** | Canonical `.conduit` parsing, located diagnostics, checking, recursive Forms through Faces/Backs, exact typed Ports, and separate source/checked/expanded identities. Ordinary examples run through planning and the production kernel. | Supported semantics and implementations are finite; a catalog entry does not promise an implementation on every Host. See [Forms](forms/README.md). |
| **Planning and execution** | One port-aware kernel, explicit fan-out, bounded queues and operations, resource and authority admission, immutable Plans, cancellation, pressure, and correlated Signs. Shared code serves hosted, browser, and embedded paths. | Deadline guarantees belong to specific admitted profiles. General SMP, preemption, and physical real-time guarantees are not established by cooperative execution. |
| **Body lifecycle** | Zero/one/many initial Forms, durable Part membership, current/offline Host presence, Body-wide workload admission, one current Plan and at most one active Play. Multi-Form execution and workload changes have hosted and browser evidence. | This is implemented, not merely the old #2062 proposal. Quiescence versus semantic completion still needs the runtime correction in [#3006](https://github.com/dancxjo/conduit/issues/3006). |
| **Tour and browser Hosts** | An executable Tour with real browser/WASM execution, embedded Patchbay, independent browser Hosts, and admitted browser operations. Camera/microphone paths have bounded acquisition and failure handling. | Browser permission, device availability, tab lifetime, and supported profile still apply; no blanket background or physical-media guarantee. |
| **Patchbay** | Native and browser inspection and editing; Body workbench; execution activity, bounded Watches, observation replay, and scoped breakpoint/causal-trace support. | Replay of observations is distinct from re-executing a program. Missing telemetry remains a gap. These features do not establish distributed stop-the-world debugging. |
| **Crèche and fabrication** | Birth a Body with reviewed Forms, prepare target-native artifacts, inspect membership, and retain Body evidence into Patchbay. Fabrication packages cover hosted computers, browser, ConduitOS, and board families. | Building or downloading an artifact is distinct from installing, booting, admitting a Part, and executing work. Consult each [target](targets/README.md). |
| **ConduitOS** | Five product targets: x86_64 and IA-32 PC, AArch64 and RISC-V64 virt, and LoongArch64 virt. x86_64 has the graphical shell, Tour/Patchbay, keyboard and pointer input, and USB attachment journey; the other four have serial product media. | The illustrated journey is **freestanding-emulator** proof. The physical laptop campaign is open. A target's boot proof does not establish graphics, drivers, or hardware parity. |
| **Lines and physical Pico work** | Recorded WebSocket/USB CDC execution and one-Body Pico W control, including new-Plan recovery and continuation over an already-admitted fallback Line. | This is bounded, device-specific physical evidence. It does not imply arbitrary discovery, federation, public-Internet security, or a general reconnect policy. |
| **Standard semantics and local tasks** | Executable text, time, state/flow, logic/math, input, presentation, and other reviewed families; protected local file-copy operations exist. Catalog and target gap reports derive availability from code. | The old “copy a file is disabled” record describes a retired prototype. ConduitOS storage and physical file-copy remain separate unfinished work. |

For implementation owners, start with the [repository map](docs/repository-layout.md)
and [architecture references](docs/architecture/README.md). The table above
summarizes capabilities; the historical ledger retains the detailed evidence
for earlier accepted slices without turning every past limitation into a
present limitation.

## Recent implementation and unfinished integration

These source-level capabilities are present in the reviewed development tree.
Their presence is not an additional physical or release acceptance claim:

- **Body continuity and supporting contracts:** surviving-Part continuity,
  atomic workload transition, typed failure disposition, bounded Resource
  collections, and causal evidence have dedicated implementation and tests.
  See [Body lifecycle boundaries](docs/architecture/body-lifecycle-waists.md).
- **ConduitOS compositor:** retained surfaces, damage, and input routing exist
  under [the native compositor](targets/conduitos/src/native_compositor.rs).
  Tour now reports distinct retained workspace/status surfaces, and its journey
  verifier checks their identities. The broader shell proof and interaction
  polish remain tracked in [#3043–#3049](docs/roadmap.md#conduitos-shell).
- **House speech:** recorded-audio recognition, address detection, model context,
  and conversation components exist. The attended, live named-house speech
  experience remains open in [#2297](https://github.com/dancxjo/conduit/issues/2297).
  Recorded PCM and a model response do not prove a microphone-to-speaker household.
- **Reusable applications:** Forms-as-Gears is implemented, while several
  complete reusable application compositions still have open acceptance work.
  [The roadmap](docs/roadmap.md#reusable-forms) names those remaining slices.

The current `cargo xtask conduitos std-gap` report also identifies missing
ConduitOS host operations for `math/map-quantity` and
`structured-info/wrap-quantity`, alongside the missing storage Base for
`file/copy`. Run the report for the current profile instead of relying on a
frozen catalog count.

## What is missing

The major remaining gaps are product integration and breadth of real-world
proof, as well as specific architectural work:

- Keep idle Forms alive by default until explicit completion or lifecycle
  disposition, preserving the same admitted execution across later input.
- Make the graphical ConduitOS shell a usable set of independently retained
  surfaces with inspection, transients, resizing, scrolling, and clear focus.
- Boot and qualify a real old laptop, then implement its storage, network,
  audio, input, and house integration in the staged hardware campaign.
- Complete a persistent multi-Host House with live speech and supported
  smart-home adapters. House admission work is currently marked paused.
- Complete reusable application capstones and the larger Pete robotics Body;
  deterministic robot motion selection is not physical motion proof.
- Extend implementation confinement where required. Cooperative admission
  and recorded authority are not proof of hostile-code isolation.

See the [roadmap](docs/roadmap.md) for issue owners, dependencies, and paused
work. These are not prerequisites for contributing a small, useful change.

## Reading proof correctly

| Evidence | What it establishes |
|---|---|
| Contract tests and deterministic simulation | Identities, bounds, transitions, and refusal behavior in the tested model |
| Hosted or browser execution | The actual implementation running in that environment |
| Emulator execution | A freestanding image running against the specified emulated machine |
| Live transport | Actual messages crossing the named transport/session |
| Physical/HIL evidence | The named firmware and device behavior under the recorded physical conditions |
| Human enactment | A person completed the specified interaction; automation cannot supply this claim |

Use [visual evidence](docs/visual-evidence.md) to inspect screenshots with their
provenance, and the [proof-class reference](docs/architecture/proof-classes.md)
for precise definitions. This documentation review creates no new boot,
browser, physical, or human acceptance receipt.

CI now separates inexpensive PR admission, combined `dev` integration, and
exhaustive release proof. The current procedure is in the
[CI guide](docs/contributing/ci.md); old workflow names in historical receipts
are not commands to reproduce the present gate.
