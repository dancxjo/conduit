# Pete cognition

Each typed frame supplies a battery reading, human utterance, safety state,
and reference instant with explicit source signs. The Pete Situation Gear
selects the Situation and constructs the bounded Experience input for the
episode composer. The model and memory cell run in the same ordinary Play.

The composer retrieves from the four retained episodes using distinct shared
ASCII words of at least four characters, with a small common-word filter.
It searches retained observations. It includes the selected responses as
model-derived history and reports each match score in the model request.
Higher scores and then newer episodes take precedence under the prompt bound.

The hosted proof registers Pete's Back on the selected Host, submits two frames,
and checks that the second model turn can use the retained first episode.
It also checks that a future observation fails before model admission.

```sh
cargo xtask check pete-cognition
cargo xtask check pete-cognition-live
```

The live check uses installed Ollama models on loopback. The proof supplies
frame inputs through the Fore. Live sensor and homeostasis sources, durable
autobiographical feedback, provider-loss recovery, and Patchbay causal
inspection remain part of [#3707](https://github.com/dancxjo/conduit/issues/3707).
