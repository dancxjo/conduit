# Continuing Experience (development)

`current.conduit` wires the ordinary `experience/integrate-current` Gear to
`experience/project-evidence-flow`. Each typed upsert or removal declares its
expected committed revision. The integrator stages a bounded snapshot and commits
only after its output is published. Up to eight evidence items remain available
for subsequent updates in the same Play. Queue pressure waits upstream;
malformed, stale, future, unknown-removal and capacity cases refuse explicitly.

The snapshot carries actual selected evidence through its Cord, including
content kind, origin, temporal role, availability, certainty and source
references. Snapshot revision fields advance together; original source nodes
remain unchanged. The older `main.conduit` identity projection retains its
existing Fore.

`model-context.conduit` adds `experience/context-to-prompt`. It emits the existing
LLM generation-request kind with a complete JSON request of at most 4096 UTF-8
bytes. Evidence stays under a fixed instruction as data; readable source roles
and identities accompany it. Resource references carry an explicit inert
encoding and confer no lookup authority. Binary content is labeled as hex.
Overflow refuses the complete request rather than truncating evidence.

Run the focused contracts and installed execution checks:

```sh
cargo xtask check pete-cognition
```

The suite exercises successive revisions, explicit observation/recollection
separation, cancellation, byte pressure and prepared storage. Separate Body
model-route cases exercise a sealed shared Body Plan with a standing clock and
an explicitly fenced fake provider. Those calls currently execute synchronously;
provider loss retains whole-Play failure semantics. These cases establish no
real-model behavior or continuity during a slow model call.

The full [continuous cognition MVP](https://github.com/dancxjo/conduit/issues/3707)
still requires Situation execution bindings, authored exact temporal context,
autobiographical selection/retention and outcome feedback, three bounded
cognitive cadences, model-loss isolation/recovery and complete Patchbay proof.
