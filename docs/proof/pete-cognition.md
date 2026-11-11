# Pete continuing cognition: live local model proof

This draft-branch proof belongs to [Continuous Cognition #3707](https://github.com/dancxjo/conduit/issues/3707).
It exercises [the situated Plot](../../plots/pete-cognition/situated.conduit)
through the installed std Body runner, with scripted typed observations and an
already-local Ollama model. It does not acquire physical observations.

Run from the workspace root, choosing an exact already-installed model and a
finite memory admission appropriate to the host:

```sh
cargo xtask make host prove-pete-cognition \
  --model phi4:latest --admitted-memory-mib 16384
```

The command does not download a model or substitute a fake provider. A dry run
(`cargo xtask --dry-run make host prove-pete-cognition ...`) does not initialize
the provider. Each execution creates a fresh directory beneath ignored
`target/conduit-evidence/pete-cognition/`; `--output` can select another directory
beneath the workspace's ignored `target/`.

Two typed PeteSituation frames carry an external human's statement about a
cobalt-blue mug, then a question about its color. Battery and body observations
are explicitly unobserved. The second frame does not contain the color. The
ordinary Situation, autobiography recall, integration, evidence projection,
prompt, model and retention Gears produce both responses. The command supplies
no remembered input and no response text. One ordinary clock Plot remains
resident alongside cognition in the same sealed Body/Wake/Plan/Play.

The receipt checks two actual provider calls, complete distinct responses with
“blue” present in the second, the first context having no recalled history,
the second containing historical episode-one evidence, preserved sources,
byte-exact retained observations and result envelopes, two completed records,
and at least three actual clock ticks plus waits after feedback. These are
mechanical acceptance criteria, not a semantic quality guarantee. Read the
actual responses for attribution, unsupported claims and whether they answer
the question. Model-derived output remains inference even when the receipt
accepts it.

Evidence includes the exact admitted model content identity, Body/Plan/Play
identities, native input frames and selected Situations, actual model requests,
result envelopes and text, all three autobiography snapshots, clock output,
Body report and bounded causal inspection. Failed execution criteria retain
partial evidence before returning failure. Discovery or initialization failure
can occur before Body preparation and does not establish a Play.

The situated Plot admits 4096 output bytes, including the result envelope. Its
former 512-byte allowance left only one generation token after the existing
adapter's envelope reservation; live Llama 3.2 runs exposed that failure. Those
runs also demonstrated that correct memory delivery does not guarantee correct
interpretation by a small model. Negative runs remain in local ignored evidence.

Preparation and proof collection use finite bounded storage but may allocate.
Provider execution is synchronous with the existing per-call transport timeout;
the proof's 300-second deadline is checked between calls. There is no claim of
preemptible inference, overlapping inference/clock work, or whole-run allocation
freedom. The clock continues after cognition completes.

Causal inspection exports actual retained Body evidence and reports projection
or trace refusals. The command now also opts into the existing kernel debugger
observer through `StdHost::run_body_plan_with_observed_fore_to`. Its independently
prepared 128-record buffer retains at most 64 preview bytes per value. Exact
Body/Plan/Play identities are domain-separated digests of the actual execution
identities; node/Plot and Cord/type bindings come from the same lowered Plans.
Observation attaches after physical bindings and detaches after cleanup.

`debugger-timeline.json` projects actual records through Patchbay's existing
DebuggerTimeline; `debugger-upstream-trace.json` follows actual retained parent
pointers. Both preserve collector gaps and timeline evictions. Input/output
port-number collisions have explicit combined-direction labels and retain the
full exact identity table; Cord types remain distinct. No parent is reconstructed
from log ordering. The current optional profile requires canonical, one-slot
Cords because its kernel observer correlates the latest send per Cord. Wider
queues refuse debugger preparation instead of claiming false FIFO parents;
ordinary execution keeps its existing queue profiles.

The 2026-11-10 observed run `run-1794374757017-199943` retained 71 actual debugger
records, with no observer refusal, collector drop, or timeline eviction. Its
receipt SHA-256 is
`f64ea4c8d0ae39556b99efae3c06e9c95d0d72fa6a5f094171802105b9de2afe`;
the timeline SHA-256 is
`6e0693b2cb06ac159e5d46783e23766d051d35df3efbf0ad42af4b11127bba71`.
The final response is sequence 52. Its pointer chain follows State, recall,
integration, projection, prompt, model and retention but does not include the
Situation input at fan-in. The summary therefore marks the full cognition chain
and rendered browser proof **unverified**. The existing mandatory-Sign causal DAG
is exported separately; it must be matched to the actual final output and
rendered with the real graph before claiming complete Patchbay verification.

This observed run completed both model responses and retained both episodes;
the second response recalled cobalt-blue and attributed the report to the human.
The first response overstated the human report as “confirmed.” The raw inference
is retained unchanged and stays ModelDerived; receipt acceptance does not certify
that wording. A prior 64-token run truncated the second response and failed its
acceptance check. Recoverable generation now permits up to 128 decoder tokens
within the existing envelope reservation and exact output/work limits. Existing
model kinds keep their previous decoder ceilings. Existing RAG integration,
continuous acquisition/homeostasis and the three cadences, provider readmission
with state-preserving replan, and broader continuous-system proof remain
acceptance work under #3707.

The RAG follow-on now has prepared borrowed paths through the existing temporal
retrieval and context-selection algorithms. Temporal selection admits at most
128 candidates and returns fixed-storage indices; context selection prepares
scratch for at most 1024 candidates and writes indices and omissions into
caller-owned slots. Owned APIs delegate the same selection algorithms. Tests
cover exact lineage, ordering, budgets, history boundaries, refusal clearing and
zero allocation during prepared selection. The borrowed temporal API has an
explicit first-history `NoHistory` result; the owned API retains its existing
empty-batch refusal. Temporal selection retains the existing source-timestamp
ordering, with uncertainty preserved as metadata. These are semantic preparation
checks, included in `cargo xtask check pete-cognition`.

The ordinary `retrieval/temporal-select-flow` Gear has an installed std Back.
A native window carries its identity, explicit reference,
at most 128 candidates and a history-completeness declaration. A separate typed
Current intent selects the existing temporal policy; the outcome retains the
window identity and distinguishes selection, no history, incomplete history and
an unavailable boundary. The prepared canonical codec accepts all seven intents
without owned reconstruction or allocation during execution. The
[retrieval Plot](../../plots/pete-cognition/retrieval.conduit) authors latest
evidence intent as ordinary typed State. Recall also has an explicit-selection
entry point that resolves a supplied episode/item against the supplied state,
without applying its legacy latest-item policy. These surfaces still require
the typed autobiography candidate and outcome adapters before they replace the
situated Plot's recall path; they do not establish an end-to-end RAG proof.

The 2026-11-10 draft-branch run `run-1794373833048-189399` accepted with
`phi4:latest`, model content identity
`ac896e5b8b34a1f4efa7b14d7520725140d5512484457fab45d2a4ea14c69dba`.
Its local receipt SHA-256 is
`a99372e2b97c3aba3da3737218a8c987f08d80a6e679e313ba7691e89af4be05`.
It produced two complete responses, retained two episodes, emitted three clock
ticks after feedback, and exported 62 retained causal traces with zero refusals.
The second actual response attributed the historical report to the human and
identified the mug as cobalt-blue. Both responses kept the unobserved battery
and body state uncertain. This is one provider-nondeterministic run, not a claim
of general model reliability. Earlier Llama and Gemma runs exposed grounding
and attribution errors and remain retained alongside it.

## Historical Patchbay inspection

New proof runs retain the actual `body.json` and `wake.json` at Wake before
Play. The inspector validates these snapshots, the sealed Body Plan, the actual
Body Play, both authored graphs and exact lowered runtime bindings. It never
invents per-Plot Play receipts to satisfy the live renderer lifecycle.

```sh
cargo xtask make host inspect-pete-cognition \
  --evidence target/conduit-evidence/pete-cognition/run-1794375618863-207784
```

The command serves a read-only historical view and prints its local URL. Add
`--browser-proof` to build the same inspector and run the locked Chromium proof
with one worker and zero retries. It requires the locked dependencies under
`proof/browser` and their pinned Chromium installation. Each browser attempt
writes a fresh directory beneath the selected run; failed attempts are retained.

The view includes both actual Plot graphs, exact selected Host/Boot and
implementation facts, the Body-wide Plan/Play, all retained debugger events,
and structured causal Signs. `causal-final-response.json` selects the last
actual retention response routing event through its exact output binding. Its
mandatory-Sign DAG keeps fan-in branches distinct from the observer's
single-parent playback. The exporter checks exact execution metadata, all eight
stages, both external frame consumptions and both model results, and ordered
first retention → State update/current → later recall → final response ancestry.
One-slot queues are required for this correlation. A query-root Sign is not an
abnormal semantic terminal.

Recorded native evidence and model results appear as historical Info. Missing
observations stay unknown, human reports retain their origin, model words stay
ModelDerived, and native temporal sources remain explicit. Bounded UTF-8
snippets include truncation and artifact digest facts; full artifacts remain
unchanged. The historical page hides Plan/Play execution controls. Its browser
Manifestation is distinct from the recorded Body Play.

The 2026-11-10 run `run-1794375618863-207784` produced two complete Phi responses
and retained both episodes. Its final response DAG contains 42 nodes and 51
edges, with all eight stages and the ordered two-episode feedback chain present,
exact identities and no missing branches. Receipt SHA-256:
`bf2aab875cde38b8ad5bd544a166543e7bb164a39cea708ff7cf7183c692c4b8`.
Final-response DAG SHA-256:
`f1ccb36f15092552903f47a842e825cfbfa62827e4eec4340d35b462979b7fa2`.

The pinned Chromium proof passed for that run in
`patchbay-browser-1794378257440-257627` (one worker, zero retries). Browser receipt
SHA-256: `baf1281ce86c2e2d68e4338f904f8410d0f6d435b3bbc0cce7853b20816c9d05`.
The receipt binds the inspector executable, source artifacts, both captured PNGs,
both exact snapshots and capture declarations. Chromium was 151.0.7922.34 under
locked Playwright 1.62.0. The screenshots show actual ModelDerived words and
per-Gear causal Signs in an explicitly historical view. Earlier failed browser
attempts remain alongside the accepted evidence.

After synchronization with `dev` at `6e7f9c8a22`, the same proof passed again in
`patchbay-browser-1794380473888-291817`. Its browser receipt SHA-256 is
`b1e20f3df635e05c2828f1180563a6970a82a6a1c90c75f84b17aad66597a749`.
The six upstream target-preparation tests and strict browser-runtime Clippy also
passed; this does not claim a physical Raspberry Pi execution.

The browser proof compares every original event's sequence, parent, execution,
value and gap; all 42 Sign identities and metadata; all 51 exact causal edges;
and selected Host/Boot/implementation facts. It exercises the timeline trace and
inspects the actual second ModelDerived response. Screenshots and ordinary
capture declarations retain renderer provenance. A browser receipt separately
records the verified presentation and source artifact hashes; immutable input
summaries do not certify their own future rendering. This is historical
inspection of this scripted-input Play, not continuous physical acquisition or a
general guarantee of model wording.

Run deterministic contract checks with `cargo xtask check pete-cognition`.
Keep raw run artifacts ignored under `target/`; do not commit them as fixtures.
