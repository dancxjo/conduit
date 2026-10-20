# Smallest retained streaming round seam

Base c8946a6f3, isolated work/streaming-round-design. Read-only code investigation; no source edits/builds. Root has already proved canonical numeric channel and single hosted learned round (38.26s), including full model/feature custody, foreign owner refusal and post-ACK Source failure. Those proofs do not establish a resumable or installed streaming controller.

## Concrete recommendation

Retain the ONE existing round traversal as a private resumable request cursor, and put actual Source/model polling in a target-owned controller around existing kernels. Replace the old synchronous traversal with this cursor and make hosted fallback/learned preparation wrappers drive the same cursor. Do not leave a second old parser loop. Do not add a new executor, scheduler, host legality engine or Source-score policy.

First implementation should be a cohesive retained-cursor slice with explicit allocation proof boundary, followed by installed target preparation. Existing Source operations already decide decode-complete, transition legality, lexical choice acceptance, rank, expansion, merge and agreement. Cursor phases retain only where execution paused in that already-accepted operation sequence. Finite counters enumerate4survivors/4choices/4rankeddescendants; they cannot accept caller-supplied phase, branch list, scores or best beam.

## Reusable production primitives

* `semantics/language/src/analysis_fallback_round.rs::run_round` is the single current traversal; `analysis_round_scoring.rs` is its existing mode seam. `analysis_fallback_frontier.rs::run_frontier` uses the real all-survivor agreement and advance Source; no stable Boolean may be introduced.
* `PreparedCategoricalChannel<PendingParserScore>` already owns fresh exact actual numerical kernel/outlet, one private lease, one-step polling, pre-ingress pressure, full ACK and actual runtime map. Split the wrapper's bounded16poll hosted loop into submit_pending/step_pending without changing this custody. A legitimate Pending is resumable, unlike the hosted preparation timeout. Terminal errors remain terminal and preserve evidence.
* Every retained Source operation is an existing **linear pure-expression chain**: `language/build_support/fallback_programs.rs` explicitly checks one program configuration per gear, one next connection and complete chain coverage. No dynamic grammar evaluator is needed to execute these entries.
* `targets/std/src/flow_activation/local_expression.rs::{LocalExpressionFactory,PreparedLocalExpressionBack}` is an existing public ordinary KernelOperationFactory/StepBack. It verifies exact program/selected offer, uses PreparedPortableExpressionEvaluator and prepared canonical buffers, and supports closing Flow reuse. It can execute the retained Source chains with the already-accepted canonical export bridge and exact Fore queues. No custom Source test Back/Host Call driver is needed. Target preparation must publish these existing local pure-expression offers, then prepare the exact canonical Source Plan; do not substitute another Source identity or duplicate program bytes under a new semantic owner.
* `KernelCompositeHost::{admit_input,step,output_into,complete_output,cancel}` supplies actual Source execution, as for numerical channel. One target controller call advances at most ONE outstanding Source or numerical kernel once, never drains an inner loop.
* Core `PreparedStructuredComposer` prepares type prefixes/output envelopes and exposes `prepared_heap_bytes`; prepared selectors/validators support canonical bounded views. Use these existing adapters for frame assembly/selection where needed; do not use growing Vec canonical encoding in installed steps.
* Existing `PreparedActivationChildPool`, `BoundedScanActivationHost`, `KernelScanBack` retain pre-admitted child kernels and advance cooperatively. They are useful for an eventual authored outer stream/scan. They do **not** automatically implement this controller: the existing pure scan profile excludes Host Calls and numerical/resource coordination, and each scan item must already name one checked child transition. Do not smuggle the parser/model into its pure child profile or copy its kernel.

## Language ownership/API

Proposed NEW `analysis_round_cursor.rs` (+ small responsibility-named phase modules if needed), replacing run_round implementation rather than duplicating it. Language owner retains existing immutable owner snapshot/generation/full lexical/prior beam/revision context, read-only programs, pre-admitted counters/byte capacity, next beam/identity cursor, current branch/mask/rank and completed evidence.

```
owner.begin_round(programs, explicit_mode, admitted_limits) -> PreparedRoundCursor
cursor.next_request() -> RoundRequest // private fields, nonClone lease
cursor.accept_source(private_lease, observed_output) -> SourceLawAdmittedProgress
cursor.accept_model(CompletedCategoricalRequest<PendingParserScore>) -> Progress
cursor.finish() -> PreparedAnalysisRound
owner.apply_prepared_round(prepared) // same context/generation/beam/lexical guards
```

`RoundRequest` exposes immutable operation identity/canonical input and a move-only request lease. It has Source and model variants, never arbitrary phase/branch configuration. Source lease binds owner snapshot+generation+exact Source/program+operation+input+monotonic request sequence. Admission verifies exactly this pending request using existing prepared Source programs once, not replays the whole prefix. Raw output bytes and Source-law admission are different from actual execution attestation: the Language cursor must not claim an actual Source Play merely because a public caller can supply correct output. Actual runtime relation belongs to target controller below. Numeric completion remains opaque actual evidence, not a public score-array or arbitrary token/lease pair.

A cursor can have only one outstanding Source or model request. Taking request moves it to Awaiting; a duplicate/stale response cannot advance it. Empty/noneligible slots are skipped by bounded local bookkeeping, but no operation runs twice after resume. Every positive iteration advances source-operation sequence or a finite counter. Keep model/channel invocation sequence independent from cursor source request sequence. Owner mutation during pending work makes final application stale; do not reset owner or silently restart/replay a partly executed cursor. Cancellation/pressure return or retain exact lease/evidence without changing beam/frontiers; Closed cannot reopen.

Existing synchronous hosted wrappers become adapters driving this same cursor with existing provider/channel and their old explicit bounded hosted profile. This preserves fallback versus learned classification and existing tests while adding resumable cursor tests; it does not preserve their historical timeout-as-terminal behavior for the new installed entrance.

## Target ownership/API

NEW `targets/std/src/analysis_stream.rs` owns the cursor and a finite table of actual preprepared Source Plays, plus fresh numerical channel. Construction takes exact checked full Language Source (single generated source owner), reviewed model selection, admitted queue/evidence/operation bounds and actual target preparation; it creates all kernels itself. No constructor from arbitrary already-current kernel, arbitrary factory, callback token, caller scores or supplied ActivePlay ID.

```
PreparedAnalysisStream::prepare(exact_source, exact_model, limits, target_truth)
stream.admit_lexical_epoch(opaque_initialized_or_rebased_owner)
stream.step() -> Pending | RoundReady | FrontierReady | Refused
stream.cancel() -> retained_terminal_evidence
```

Target retains a private pending request before ingress, dispatches exactly its own canonical input into the corresponding real Source/model Play, captures exact output+ACK+source/Plan/Play identities and signs, then delivers the same lease/output to Language admission. It cannot accept externally executed Source or numerical tokens. The cursor's semantic Source observations remain separately classified; controller runtime evidence proves that these actual operations executed. A private correlation ledger binds the two, including equal-input/different-branch cases. Expose evidence by immutable getter, not a current Boolean.

Each step: (a) dispatch the already prepared exact request once if none in flight, OR (b) advance one selected kernel once and collect/ack one output if available, OR (c) perform one bounded Source-law admission/phase transition. No growing-prefix replay, draining loop, busy wait or fallback-on-timeout. Normal kernel Pending keeps the same request. Pressure before ingress preserves it retryably; actual execution errors close the controller and preserve evidence. Pre-admit signs and receipt history before any step.

## Important concrete blockers / acceptance boundaries

1. Current NativeBinding encode/decode/clones allocate during hosted round preparation. Merely moving the Rust loop into a retained enum does not prove sealed no-growth execution. Installed preparation must preallocate canonical frame/composer/selector/validator storage and evidence byte arena, then prove zero allocation/reallocation during step or explicitly admit a different bounded hosted execution profile. Existing32MiB observation cap is not proof of preallocation. Do not claim installed completion while this is unresolved.
2. Target needs exact full checked Language Source from the existing build owner, not a copied test-only `language_source()` concatenation. Add a single generated source getter/retained document from the same build input which owns COMMITMENT_SOURCE_ID. This is root-owned export wiring; no authored Source changes are necessary for initial cursor refactor.
3. `PreparedProtocolSource` is another real production route, but its convenience queue_limits caps input4096B. Existing parser_kernel fixture overrides that cap to exact Native envelopes; copying that test harness or raising4096 is not a production seam. Std LocalExpressionFactory plus canonical Fore planning avoids this convenience limitation using existing admitted Type bounds. If using ProtocolSource instead, an explicit checked bounded-profile production entrance is needed before claiming installation.
4. Existing scan/activation can wrap a future authored transition, but today no single complete round Source scan transition contains all model interaction/custody. Inventing a scan declaration does not resolve that absence. Do not promote fixture-registry preparation or a callback into installed scan proof.

## Focused proof sequence / no duplicate builds

Root shared queue only. First cursor differential: identical real operation order/inputs/outputs/results versus accepted hosted fixture, pausing after every Source request and between numerical ingress/ACK; no prefix re-execution, stale/duplicate lease and post-ACK failure/cancel/pressure preserve owner/evidence. Explicit modes and model/canonical bridge proof reused where unchanged. Then target preparation+one-step allocation probes and actual Source/model kernel signs. Finally a partial lexical stream with learned scoring reaches nonzero all-survivor Source agreement and checked commit **before finality**, obtains CurrentCommittedDependency, drives actual prosody/pronunciation consumers, then append/correction/handoff refuses protected edits or advances legitimately. Retain exact Source/model/lexical/canonical relations and actual publication trace. Do not call the first cursor test or one hosted round that final learned prefinal proof.

Ownership proposal: this agent can own disjoint Language cursor+private scoring resumption modules/tests after root hands off the one traversal; target adapter separate owner owns NEW std driver/source planning, root owns source export/manifests/xtask and shared queue. No Core/AI numerical changes or new runtime required unless allocation audit proves a specific missing canonical adapter.
