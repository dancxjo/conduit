# Hosted model authoring evidence

The [canonical tutorial](../../wiki/Creating-models.md) develops issues #5205–#5207.
The [hosted contract](../../targets/std/model-authoring.md) distinguishes tested
library behavior from proposed ordinary Plot/HostCall and ConduitVoice work.

Repository proof enters through `cargo xtask prove model-authoring`. Its CPU
suite exercises the actual public Burn adapter with a bounded regression fixture.
`--documented-command` extracts and executes the tutorial's marked command;
`--cuda` runs the explicit hardware test with separate same-checkpoint inference and independent-training comparisons, both
with declared absolute output tolerance `1e-3`. These are F32 storage profiles;
no bitwise or full-mantissa device-kernel arithmetic equivalence is promised.
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

The first independent-training probe used `1e-4` and failed after 80 steps:
its first reported difference was approximately `2.26e-4`. The revised profile
separates accumulated training drift from inference on an identical checkpoint,
reports both maxima, and uses `1e-3` for this fixture. The failed run remains
failure evidence; it does not establish the revised profile.
