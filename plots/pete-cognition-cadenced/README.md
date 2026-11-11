# Pete's three resident Forms

These Sources compose event reaction, standing attention, and reflection in one
ordinary expanded Plan/Play. `main.conduit` accepts finite explicit reflection
triggers. `timed.conduit` uses the ordinary `time/tick` Gear for two triggers,
100 ms apart. The existing `pete std cognition` operator still runs the basic
per-event Plot; these Sources currently enter through the proof suite:

```sh
cargo xtask check pete-cognition
```

`pete-reaction` situates every typed frame and emits a fast reaction for an
explicitly selected observation (`observation.retain`). Its `when` predicate is
ordinary checked Conduitese. Every situated input also reaches
`pete-attention`, which runs the existing generic Experiencer and maintains a
one-slot coalesced Current without an LLM. `pete-reflection` samples that Current
on an explicit trigger, retrieves selected autobiographical memory, renders a
bounded prompt, invokes the model, and returns typed episode outcomes through
the existing memory cell.

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
These fenced-model tests do not prove physical acquisition or live provider
latency. Patchbay replay, operator admission for the cadenced Sources, and live
multi-rhythm proof remain follow-up work before owner acceptance.
