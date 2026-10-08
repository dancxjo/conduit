# Hosted Rust model authoring

**Proposed contract for #5205**, developed against the
[model tutorial](../../wiki/Creating-models.md). Existing portable model/training
identities remain authoritative.

A small descriptor/trait family declares architecture/version, bounded signatures,
parameter groups/trainability, checkpoint schema, supported numerical/realization
profiles, finite resource estimates, initialization, forward/objective
participation, and snapshot/reload. Burn objects stay inside the Rust back.

The adapter composes with `ModelComputeOffer`, `ModelComputeSession`,
`TrainingSession`, and explicit state boundaries. Model families use ordinary
Rust composition, not a universal registry or new kernel dispatch table.
Whole-model Burn backs, submodel backs, native plot compositions, and numerical
operation backs are independent on-ramps at exact contracts. No level may claim
meaning, bounds, or interchangeability that its evidence does not establish.

One step prepares candidate model/optimizer state without replacing committed
state. Check generation, parameter policy, request/batch/objective identity,
work, and cancellation before publication. Discard failed candidates.
Read-only evaluation cannot advance parameters or training cursors.

SafeTensors and bounded versioned metadata form explicit off-ramps. Resume
bundles include optimizer/scheduler/random state and cursor; inference exports
omit them. Publish/sync immutable objects before replacing/syncing the durable
reference. Reload checks digests and compatibility. Export/import preserves
source/derived identities, exact meaning, numeric profile, and provenance; paths
and framework handles are host-local bindings, never portable authority.

Admission includes committed/candidate/temporary storage, batch/checkpoint bytes,
queue/in-flight limits, and finite steps/work. Cooperative Burn execution is not
hostile-code confinement. Unsupported shapes, devices, precision, schemas, and
bounds remain distinct refusals. Device identity is realization evidence.
