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
or trace refusals. It neither synthesizes debugger parent sequences nor proves
rendered Patchbay inspection. Existing RAG integration, physical acquisition,
homeostasis and the three cadences, provider readmission/state-preserving replan,
and complete Patchbay proof remain separate acceptance work under #3707.

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

Run deterministic contract checks with `cargo xtask check pete-cognition`.
Keep raw run artifacts ignored under `target/`; do not commit them as fixtures.
