# Pete's three resident Forms

These Sources compose event reaction, standing attention, and reflection in one
ordinary expanded Plan/Play. `main.conduit` accepts finite explicit reflection
triggers. `timed.conduit` uses the ordinary `time/tick` Gear for two triggers,
100 ms apart. `resumable.conduit` adds the ordinary checkpoint Fore.

```sh
cargo xtask prove pete std-cognition --retain \
  --checkpoint-dir target/pete-memory \
  --replay-out target/pete-cadence-replay.json \
  --episodes-out target/pete-cadence-episodes.jsonl \
  --utterance 'The hallway switch label is COPPER KITE.' \
  --utterance 'I walked past the hallway.' \
  --utterance 'What was the hallway switch label?' \
  --reflection-after 1 --reflection-after 3
```

Reflection positions are strictly increasing, 1-based, and include the last
utterance. The operator captures at most 16 typed frames before Play and admits
them and the explicit trigger values through bounded ordinary Fores. Intermediate
frames can revise attention while a model call is pending. Without reflection
positions, the operator retains its per-event mode. Optional Create acquisition
uses the same `--serial-path` input; otherwise bodily readings remain unknown.

Reuse the checkpoint directory in a later invocation to continue the Body, Wake,
and episode count with a new Host Boot and Plan. Checkpoints use the resident
memory cell's 8 KiB bound and at most four selected entries, with the same eviction
policy. Previous 4 KiB journals are upgraded without changing retained evidence
or outcomes. Evicted data cannot be recovered by migration. Source Signs and
new model run identities distinguish separate Host sessions.

Open the saved snapshot with the Workbench's `--replay-snapshot` argument. The
same Face inspector exposes selected Host/Boot/Back identities, one in-flight
model call, Cord capacities, and configured pressure policy. The timeline keeps
reaction, attention, sampling, and model traffic separate; the upstream response
trace follows the observations actually sampled and the memory feedback Cord.

Run the deterministic contracts with `cargo xtask check pete-cognition` and the
installed model checks with `cargo xtask check pete-cognition-live`.

`pete-reaction` situates every typed frame and emits a fast reaction for an
explicitly selected observation (`observation.retain`). Its `when` predicate is
ordinary checked Conduitese. Every situated input also reaches
`pete-attention`, which runs the existing generic Experiencer and maintains a
one-slot coalesced Current without an LLM. `pete-reflection` samples that Current
on an explicit trigger, retrieves selected autobiographical memory, renders a
bounded model context, invokes the model, and returns typed episode outcomes through
the existing memory cell. Instructions and evidence occupy separate fields in the
generic typed generation context; the provider adapter preserves those roles.
The complete encoded context, including its envelope, must fit the 4 KiB bound.

Each internal Cord and Fore has one queued item with an exact finite byte
reservation. The finite coalescer accepts closing input and does not invent a
replacement at closure. The sampler has one committed value and one candidate,
prepared before Play. It accepts Current replacements independently of a blocked
sample output. Pending reflection triggers backpressure; they do not create one
model job per elapsed interval. Sampling can skip intermediate attention
revisions. Only a sampled experience selected for retention becomes an episode;
coalesced attention updates are not silently added to autobiography.

The model owns one in-flight Host Call. Ollama generation uses one prepared
background effect slot; the worker only executes that dispatched request and
returns a bounded result. It schedules no Forms, retries, cadence, or new model
work. Kernel cancellation revokes publication before polling a pending result.
Provider loss can become the existing typed episode outcome, while deterministic
reaction/attention remain executable. Adapter teardown joins its effect worker;
a blocked provider is bounded by the existing 120-second curl timeout.

The deterministic proof sends 32 frames, with the later 31 arriving only after
the first model call is pending. It checks standing attention continues, selected
reaction continues, the first reflection records provider loss, and a later
reflection succeeds with the same Body and Wake. A separate causal proof checks
that fewer reflection triggers than events still retrieve an earlier episode.
These fenced-model tests do not prove physical acquisition or provider latency.
The live operator demonstration additionally verifies two reflected episodes,
then a replacement Host recalling both retained episodes. Browser proof checks
that Patchbay renders the exact causal trace and discloses realization and
pressure facts through normal inspector controls.
