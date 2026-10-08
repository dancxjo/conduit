# Hosted model authoring evidence

The [canonical tutorial](../../wiki/Creating-models.md) develops issues #5205–#5207.
The [hosted contract](../../targets/std/model-authoring.md) distinguishes tested
library behavior from proposed ordinary Plot/HostCall and ConduitVoice work.

Repository proof enters through `cargo xtask prove model-authoring`. Its CPU
suite exercises the actual public Burn adapter with a bounded regression fixture.
`--documented-command` extracts and executes the tutorial's marked command;
`--cuda` runs the explicit hardware test with CPU comparison tolerance `1e-4`.
Use a fresh output directory; prior logs are never overwritten. Manifests retain
source revision, dirty status, proof class, build profile, device evidence, exit
status, and combined log digest. A failed run establishes no successful contract.

The CPU producer also retains `artifacts/journey.json`, immutable resume objects
at step 40, and a separate inference-only export after step 80. Their build/data
identities are explicitly those of the synthetic regression fixture. The ordinary
std-host interface has separate tests under its optional `burn-model` feature.

These tests establish neither a trained acoustic voice nor naturalness. The tiny
ConduitVoice fixture and ordinary authored plots remain required capstone proof.
The selected FARGAN dependency has a separate [source inspection](fargan-dependency.md);
source inspection is not executable compatibility or reconstruction evidence.
