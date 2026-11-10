# Episode loop (development)

The ordinary `episode-loop` Plot composes `body/episode-compose`,
`llm/generate-flow`, `body/episode-retain`, and an explicit
`body/episode-memory` delay. It retains at most four selected episodes inside
one Play. A later observation waits until the previous episode's response has
been paired and its retention decision committed. Model text remains
model-derived, including when recalled. A typed observation owns the retention
decision; instructions inside text cannot change it.

Run the focused deterministic checks from the repository root:

```sh
cargo xtask check pete-cognition
```

The suite includes Pete's typed Situation-to-Experience preparation adapters,
explicit bounded evidence projection, episode contracts, and installed std
execution with a **fenced fake model**. The fake proves causal dependence of the
second request on the first retained episode; it proves no real LLM behavior.
The evidence projection never resolves source identities or grants memory access.

The opt-in hosted proof runs the same Plot with the production Ollama Back.
It requires a running loopback Ollama with `gemma3:1b` already installed.

```sh
cargo xtask check pete-cognition-live
```

The second episode asks for a label supplied only by the first episode. The
proof checks the retained observation and the model's use of its label. It then
asks the same question in a fresh Play as a control for the recall.

This remains an unfinished slice of [#3707](https://github.com/dancxjo/conduit/issues/3707).
The episode loop is not yet wired to Pete Situation, generic Experiencer
execution, or selected autobiographical RAG. The explicit evidence projection
preserves observed and recorded instants when available, including clock basis,
resolution, and uncertainty. Pete's preparation adapter admits Situation facts,
their explicit relationships, and caller-selected autobiography records into
one bounded CurrentExperience revision. The episode model request still needs
a Cord from that projection. Current provider failure terminates
the dependent Play rather than proving continuing deterministic attention in the
same Body. Three cognitive cadences, provider-return replanning, bounded
allocation-free Play implementation, and complete Patchbay causal proof remain
required. Play-scoped ring retention does not establish durable autobiography.

Source lineage: [PR #6852](https://github.com/dancxjo/conduit/pull/6852), absorbed
with its original commits into the Pete cognition implementation branch. Its
PR description predates the evidence projection and episode-loop checkpoint.
