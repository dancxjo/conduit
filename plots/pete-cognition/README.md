# Pete cognition

Each typed frame supplies a battery reading, human utterance, safety state,
and reference instant with explicit source signs. The Pete Situation Gear
selects the Situation and constructs the bounded Experience input for the
episode composer. The model and memory cell run in the same ordinary Play.

Situation selection also feeds fresh battery and safety readings through Pete's
homeostasis reducer. The result is labeled as deterministic derived evidence,
with the exact input signs and policy identity. Stale readings remain visible
as observations and leave current energy or motion unknown.

The composer retrieves from the four retained episodes using distinct shared
ASCII words of at least four characters, with a small common-word filter.
It searches retained observations. It includes the selected responses as
model-derived history and reports each match score in the model request.
Higher scores and then newer episodes take precedence under the prompt bound.

The hosted proof registers Pete's Back on the selected Host, submits two frames,
and checks that the second model turn can use the retained first episode.
It also checks that a future observation fails before model admission.

The Plot opts into a typed provider-loss outcome on the generation Flow.
An unavailable model attempt advances the episode count and emits a record
without response text. The recall ring retains earlier successful episodes.
The next frame gets its own model attempt, with one call in flight and no
automatic retry of the lost attempt. A hosted fault-injection check verifies
that a later frame resumes recall in the same Play.

```sh
cargo xtask check pete-cognition
cargo xtask check pete-cognition-live
```

The live check uses installed Ollama models on loopback. The proof supplies
frame inputs through the Fore. The [resumable Plot](../pete-cognition-resumable/README.md)
also exercises a Body and Wake checkpoint across a replacement Host Boot,
Plan, and Play. Live sensor and homeostasis sources, broader autobiographical
retrieval, and Patchbay causal inspection remain part of
[#3707](https://github.com/dancxjo/conduit/issues/3707).
