# Experience episode loop

An `EpisodeExperience` carries one trigger and one typed evidence request as
an atomic input. The composer validates the evidence, waits for the memory
cell to reflect the previous episode, and sends one bounded request containing
the trigger, selected experience, and recalled episodes to the model Gear.
The retained episode keeps its observation and model-derived response separate.

The hosted proof submits Pete Situation evidence for battery state, human input,
and bodily safety across two episodes. It exercises one installed Play and
compares the live model's follow-up to the same question in a fresh Play.

```sh
cargo xtask check pete-cognition
cargo xtask check pete-cognition-live
```

The live proof uses an already installed `gemma3:latest` on loopback Ollama. This
Plot takes prepared Experience values. The [Pete cognition
Plot](../pete-cognition/README.md) accepts typed observation frames and runs
Situation selection inside the Play. Live
sensor and homeostasis wiring, durable autobiographical retrieval and feedback,
provider-loss recovery, and Patchbay inspection remain open in
[#3707](https://github.com/dancxjo/conduit/issues/3707).
