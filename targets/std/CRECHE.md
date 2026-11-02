# Supplied Crèche encounter in the ordinary Std Body runner

`StdHost::new_with_creche_encounter(config, composition, resource)` installs the
existing `creche/prepare` Back with one explicitly selected encounter slot. The
slot must have the canonical encounter class, one unit and no content/compute
claim. The constructor does not grant source acquisition, toolchains, files,
devices, display or Make authority. This is initial Host construction, not an
API to replace a retained Host or reconstruct its ledger from received metadata.

The caller supplies a `HostedCrecheAdapter` to
`run_body_plan_with_creche_to_with_start`. The adapter retains admitted finite
encounter state, effects and canonical evidence. Preparation validates the
current Host/Boot/generation, sealed complete BodyPlan, exact selected operation,
resource and prospective Play. The ordinary runner confirms the actually issued
Play after its start callback. Only then can the single exact Host Call begin.
The external request Fore is derived from this checked owner, never invented by
a client or a PlannedGear-only factory. Missing or mismatched owners refuse
before Play. This API neither births a Body nor selects a new-Body image producer.

The owner paces bounded polls, verifies exact retained evidence bytes against the
outcome digest and correlates the one acknowledged typed outcome. Output pressure
does not repeat an effect. The adapter must cancel and drain platform work before
kernel retirement; cleanup failure remains distinct from the execution failure.
Its `begin`/`poll` methods are nonblocking; any platform workers remain adapter
owned. The shared fixed StepBack is unchanged and gains no hidden allocator or
scheduler.

If checked retirement still fails, the Std ledger **retains the exact charged
reservation vector by move**. It does not release/re-reserve the slot. This is a
terminal Host quarantine: observation, fresh reservation and capability refresh
refuse; there is deliberately no automatic recovery API. The advertisement is
implementation/capacity inventory, not fresh Ready evidence. The caller receives
the original execution report/error plus explicit cleanup uncertainty and must
retain the platform owner. Drop/process exit does not prove external quiescence;
a replacement Host needs independent real provider admission.

Owning tests run with `cargo +stable test --locked -p conduit-std-host --lib creche`.
They exercise the shipped Source, real checked/sealed Plan and issued Body Play,
production factory/kernel Host Call and acknowledged Fore with a software adapter
that returns a classified fixture refusal. Other cases cover missing owner,
changed resource/call, corrupt evidence, once-only/paced progress, cancellation,
failed drain, late output refusal, zero-allocation reservation custody and actual
next-admission refusal. This establishes reusable Std execution/admission, not an
installed interactive authoring encounter, successful Seed bytes, physical display,
deployment, destination Boot/membership or completion of #6458/#6468/#6341.
