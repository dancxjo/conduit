# Next learned-round score custody: read-only plan

Inspected actual production AI/Language APIs at the isolated fallback preparation
snapshot; root imported reviewed fallback at4b89e28f2. This document changes no
fallback Source/Rust/test files, model selection or acceptance. No builds/polls.
Fallback complete-round proof remains explicitly zero-score and is not a substitute
for the learned parent goal.

## Reuse these existing production owners

1. Language `src/parser_model.rs::prepare_english_parser_scorer` already selects
   only adopted exact V2 artifact bytes/content, full signature semantic digest,
   complete resource reference and descriptor digest; it validates finite residence
   bounds and dimensions(413,76,25). `english_parser_scorer_descriptor` binds the
   feature/action schema and exact source-pinned int16 weights. Do not replace
   this selector with architecture-name or dimension matching.
2. Explicit `parser_punctuation_model.rs::prepare_punctuation_parser_scorer` is a
   DIFFERENT opt-in, unaccepted V3 content/reference/access-class profile, despite
   sharing the V2 feature/action ABI. Never silently promote it to accepted V2 or
   report candidate evidence as the accepted model. Root chooses model policy.
3. AI `model_resource.rs::AdmittedModelResource::adopt` is already opaque and
   retains immutable bytes, full artifact/signature, exact descriptor identity
   and admitted resource-access receipt. Digests alone do not replace it.
4. AI `integer_categorical_step.rs::PreparedCategoricalStep` owns that resource,
   exact input/output Types/codecs, model, pool and bound residence. Its offer
   binds adoption; `verify_placement` checks exact kind/revision/capability/back/
   artifact/ports/limits/semantic contract/host/boot/resource pool/content binding.
   Reuse its real resource-bound numerical implementation and finite budget.
5. `CategoricalStepBack::prepare_planned` verifies that placement and allocates
   finite codec/index/score storage before Play. `step` performs actual existing
   categorical inference; only `step_committed` advances committed invocation
   and publishes last receipt. Cancellation clears staged work. Reuse that exact
   commit boundary, not successful output decoding as a commit surrogate.
6. Optional AI `owner::CategoricalOperationFactory::for_plan` verifies sealed Plan,
   unique exact profile match and selected placements; its ordinary factory
   prepares actual categorical Backs. Target runtime owns this registry/Plan,
   resource residence, kernel stepping, typed transport and output acknowledgment.
   Language must not import conduitos or copy test scheduler into production.
7. Language `parser_scorer_features.conduit` owns exact feature extraction from
   full lexical/state/choice request. Query retains exact Source/analysis basis,
   lexical Source sequence(epoch) and invocation; result carries those plus25
   indices. Scores stay a numerical proposal; existing legality/rank/transition/
   merge/agreement/commit Source remain unchanged.

Existing numerical test evidence is REAL but scoped: ModelDriver in
`tests/common/parser_model_kernel.rs` uses actual selected/admitted resource and
actual CategoricalStepBack inside the existing fixed kernel scheduler. Test-only
source/sink/wrapper transport captures the Back receipt after step_committed.
This is not a fake numerical scorer. It is also not an installed production
controller/evidence API: the custom test wrapper has direct concrete Back access.

## Concrete missing production seam

`CategoricalStepReceipt` currently has PUBLIC copyable fields: model_content,
input digest, output digest, invocation, work_units. It is useful diagnostic
metadata, but any caller can construct it. It has no opaque committed-execution
custody, full input/output, descriptor/signature/adoption/residence, selected
placement or Plan/Play binding. Language cannot accept that struct plus an
arbitrary [I64;76] as learned scores, even if all hashes correlate.

`CategoricalStepBack::last_receipt` exposes only this copyable struct. The normal
factory erases Backs behind StepBack, so a target consumer cannot currently obtain
a production opaque committed invocation from the selected ordinary factory.
`EmbodiedModelView::from_plan` and serializable EmbodiedModelReceipt describe
selected wiring/IDs, not private numerical execution custody, and cannot fill
this gap by themselves.

## Minimum new AI-owned contract (ownership handoff required)

Proposed names, not implemented:

- `CommittedCategoricalInvocation`: PRIVATE fields, minted only when the existing
  Back's staged transaction actually commits. Retain immutable prepared profile
  or its exact opaque resource owner; full canonical input/output bytes; committed
  invocation number/work; selected placement/implementation/adoption binding.
  No public constructor/deserializer from metadata and no accepted Boolean.
- Preserve old public diagnostic receipt for compatibility/inspection. Add an
  opaque getter or consuming observation method; do not relabel diagnostic bytes
  as authority. Prefer borrow tied to committed Back storage where practical;
  owned retention must preserve actual private minting and bounded capacity.
- Existing codecs already have fixed templates/offsets; retain input bytes in a
  preallocated exact-size snapshot when input is admitted. Output encoded storage
  already exists. No new inference arithmetic, collection codec or hidden data
  conversion. If owned receipts are emitted, pre-admit finite slots and exact
  byte capacities before Play; reserve before committing and refuse/await pressure
  without losing an output's receipt. Never append an unbounded runtime Vec.
- Ordinary factory needs a small AI-owned bounded observation outlet, or an
  existing owner observation mechanism if AI owner identifies one, so the target
  runtime can retain opaque commit evidence without concrete-Back downcasting or
  a second numerical driver. Factory remains the operation owner. The outlet
  contract must identify which selected placement produced each invocation and
  must not let the consumer publish its own evidence.
- Kernel active Play ID is not available in PreparedCategoricalStep alone.
  Runtime composes the outlet's private selected-placement evidence with its
  actual started kernel/Plan/Play and acknowledged output. A serialized Play ID
  supplied beside a forged score is insufficient. No AI evidence claims a human
  action, playback or an external service's current state.

This is a narrow generic AI numerical-evidence prerequisite, not parser policy.
AI/target owners must agree it before Language accepts learned output.

## Minimum Language admission and round integration

A new private `PreparedParserFeatureInvocation` is constructed only inside the
owning complete-round protocol from the exact current branch/state/choices,
lexical tape/Source sequence, owner generation and selected model/session profile.
It executes/replays existing feature Source and retains the full request/result,
actual Source program/operation receipts and exact canonical25-index input.

A numerical invocation lease binds this pending feature context BEFORE inference
and reserves the expected next invocation from the SAME selected Back/Play. It
must not reset invocation across source epochs while the numerical Play survives;
a new Play has a distinct explicit identity and counter scope. Generation,
request sequence and epoch are separate facts.

Admission consumes/borrows the opaque committed numerical invocation and requires:

- same selected exact profile/resource/signature/adoption/residence/placement;
- exact feature input Type and canonical bytes, not only equal-looking indices;
- exact expected numerical invocation/Play/pending-request identity;
- Source output's basis and Source sequence match the private owner/branch;
- output canonical Type is the existing76-I64 contract, exactly retained;
- observed scores and output digest correspond to actual opaque output; work
  matches selected model, with the finite typed score bound re-admitted.

Equal indices can occur for DIFFERENT branches/epochs. Input digest alone does
not bind their context. The private pending lease/full feature request plus exact
invocation identity is therefore mandatory. A caller may not choose invocation,
substitute a previous valid score receipt, or attach a current=true flag.

Only the resulting opaque `AdmittedParserBranchScores` may populate the existing
Source ranking request for that branch. Zero-score fallback and learned receipt
must be distinct Type/enum variants and observable proof classifications; a model
failure may yield named refusal or an explicitly chosen fallback, never a learned
label on zero scores. Existing Source tournament masks/ranks legal classes and
selected-transition/merge decide successors. Keep complete-round coverage across
all4 slots/branches; one successful numerical result cannot justify caller pruning.

Round prepare/apply retains all feature/numerical receipts, exact complete prior/
result beam and model policy, and rechecks generation/capacity before mutation.
Cancellation, pressure, edit/rebase or late completion leaves beam, choices,
frontiers, history, invocation lease and outstanding request custody consistent.
A consumed invocation cannot be accepted twice. Epoch handoff preserves committed
choice history and model profile; changing model requires an explicit checked
migration or refusal, not a hidden default swap.

## Finite proof and implementation order

1. AI owner adds opaque committed-invocation/outlet with generic actual Back tests:
   nothing before commit, exact input/output, wrong placement/resource, cancel and
   full evidence-capacity refusal. Existing ordinary factory/kernel must exercise
   it. Do not rely only on the direct test wrapper's public receipt fields.
2. Language adds Source-feature invocation/score admission with exact receipt
   correlation. Refuse foreign weights/signature/schema/adoption/residence,
   changed input/score, stale generation/source, wrong invocation/Play, duplicate
   completion, and identical-feature-vector/different-context replay.
3. Extend complete-round mode explicitly to the selected learned profile, preserving
   every actual numerical and Source observation. Use the same actual runtime
   fixture for the smallest learned positive, then connect ordinary target owner
   ingress. No arbitrary score array entrance.
4. Prove learned committed greeting and downstream consumer with the selected
   reviewed model; retain negative model-quality findings and scope. Generic
   learned round custody does not establish V3 acceptance or broad English quality.

Use a declared finite budget tied to chosen4-token/4-survivor/<=8-round profile:
<=4*4*8=128 branch inferences per epoch before early complete cases, plus bounded
Source operations. Existing test ModelDriver MAX_INVOCATIONS4096 is a test ceiling,
not the production profile budget. Generic history limits do not imply this model
work budget. Pre-admit and measure actual input/output/evidence packet sizes;
raise no generic bounds and truncate no custody to fit.
