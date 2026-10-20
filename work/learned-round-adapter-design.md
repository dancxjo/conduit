# Learned complete-round custody: API/ordering review

Inspected exact 73defbe38cb9813c374896cb0a56872fb4b7ea3d in isolated work/learned-round-custody-prep. No builds or source edits. This is an implementation proposal, not accepted learned execution.

## Reuse and one orchestration

`analysis_fallback_round.rs:run_fallback_round` already enumerates all four private survivor slots, obtains actual checked decode-complete/transition legality, enumerates Source-admitted lexical choices, ranks at most four actions, rechecks transitions, expands and merges. Preserve this traversal exactly once. Extract its scoring choice to private `RoundScoring` collaborators; do not copy the parser loop into a learned module. Existing `prepare_fallback_round[_with]` and profile remain explicit zero-score fallback wrappers.

Proposed explicit `RoundMode::{ZeroScoreFallback,AuthenticatedCategorical}` is result classification, not a caller permission to provide scores. An authenticated entrance must require the opaque actual runtime channel below. Prepared complete round retains mode, prior exact beam/lexical/generation, full Source observations and consumed opaque numeric tokens. Existing apply checks generation/prior/full lexical/closed/capacity transactionally. Do not accept another mode through the old fallback entrance.

At the present fallback ranking site, learned flow creates existing `LanguageParserScorerChoiceQuery::new(choices, lexical.source.sequence, invocation, lexical, branch.parser.state)`. This includes entire lexical tape, exact parser basis/state, selected choices and source epoch. Replay actual `language-parser-scorer-features` to `LanguageParserScorerFeatures`, retaining all frames. Canonical encode the exact25 U64 indices using selected profile.indices_type. Existing tests/common/parser_selection_driver.rs:431-459 demonstrates these exact types and field order but its public diagnostic receipt is not new authority.

Only after authenticated token admission decode its exact canonical76 I64 score collection and field-copy to existing `LanguageParserScorerRanking::new(mask.allowed,0,0,false,0,scores)`. Existing checked Source rank/transition/expansion/merge still decide outcomes. No host arithmetic or legality/class-selection policy is added. Existing output codec must check collection type and count exactly, not accept arbitrary native score arrays.

The fallback Source program export list needs existing `language-parser-scorer-features` appended (existing chain, not new semantics). Proposed neutral name `PreparedAnalysisRoundPrograms` with original public alias; actual build/source identity changes require root's later carrier refresh. Existing fallback proof remains reusable only where relevant programs/source identity unchanged; do not claim exact-source identity across re-export without recording it.

## Necessary ordering seam

A genuine opaque numeric token proves an acknowledged numerical invocation. It does NOT alone prove which private Language branch requested it. Counterexample: same model and selected Plan, branches A and B produce equal25indices; caller obtains token1 for A then passes it for B. Comparing bytes, model, Plan, epoch and even invocation1 succeeds without preserving A's dispatch lease. Merely consuming a nonClone token prevents duplication but not this first relabelling. A public callback returning any real token cannot establish dispatch order.

Minimum AI helper: `CategoricalInvocationOutlet::owns(&CommittedCategoricalInvocation)->bool`, implemented only by private inner identity (Arc::ptr_eq), with no exposed integer outlet ID or token constructor. This rules out identical newly-prepared factories sharing a Plan/model and each starting invocation1. Root's independent outlet RAII patch should own this narrow change, not this Language slice.

Minimum production target seam: one `ParserModelChannel` privately owns a fresh observed categorical factory/outlet, exact selected profile+Plan+placement and actual Play ingress/egress relationship. It must not accept an arbitrary already-running Back or arbitrary token callback as current authority. The same owner that publishes the exact pending request into that Play retains its unique lease until its corresponding token is consumed or terminal failure recorded. Language does not invent an ActivePlay ID; ordinary target adapter retains actual one.

Proposed nonClone private-field `PendingParserScore` stores owner session identity/context, generation, exact prior beam/lexical snapshot, round index, survivor slot, choice ordinal, full feature query+execution+result, canonical numerical input and expected channel invocation. It is issued only inside shared traversal. There is at most one pending model request in this minimal profile; caller cannot substitute branch/query/phase. Actual channel dispatch consumes that lease, emits its canonical input, retains lease internally, then returns private-field `AdmittedParserScores` carrying same lease and consumed opaque numeric token. No public constructor from `(pending,token)`.

Channel completion checks outlet.owns(token), exact invocation sequence, full selected Plan/placement/profile/artifact/signature/reference/content binding, exact canonical input and canonical typed output. Refusal returns all still-owned evidence; cancelled/pressure cannot advance parser/round state. It must reject foreign/stale/duplicate/out-of-order tokens before scoring. A single outstanding request avoids caller-selected reordering but does not replace actual runtime dispatch custody.

**Current obstacle:** there is no production Language/ordinary-target channel that atomically owns pending feature dispatch and categorical acknowledgement. Building an apparently authenticated public `(lease,real_token)` checker would leave the counterexample above. Shared-round extraction, Source feature custody and strict numerical identity comparison are safe independent work, but must remain prepared/unpublished until target channel exists. This is a concrete missing production seam, not a need for new grammar, numeric kernel or parser runtime.

## Finite profile and tests

Existing4 survivors x<=4 lexical alternatives =><=16 numerical requests/round; <=8 rounds/epoch =><=128. Retain actual tokens only within admitted outlet capacity; up to128 held per epoch. Pre-admit source observation/token vectors and exact payload byte budgets before target dispatch. Existing Source observation cap1024/32MiB may prove insufficient after adding feature chain; measure exact chain accounting, refuse unsupported bounds instead of raising silently. Invocation overflow, model absence and all target failures remain named; caller explicitly chooses separate fallback preparation, never silently changes learned mode.

Focused proof: same-index different branch requests cannot exchange tokens; identical Plan/model from another outlet refuses; wrong artifact/reference/access/residence/placement/signature/input/output refuses; token nonClone plus private dispatch consumes once; stale generation/epoch/lexical/prior beam refuses unchanged; pressure/cancel/error retains owner unchanged and no orphaned lease; learned/nonlearned classifications separate. Positive proof must run actual feature Source and ordinary numeric Plan/Play, retain signs and full source/numeric output, feed existing shared rank/transition Source and advance nonzero commitment through checked frontier. Direct StepBack ack remains diagnostic and cannot satisfy installed acceptance alone.

## Proposed ownership for reviewed implementation

This agent: disjoint NEW `analysis_round_scores.rs` (private leases/score evidence), shared traversal extraction from owned fallback round only after root explicitly hands it off, new focused tests. Root: Language exports/build list/manifests/owner-history wiring and target integration ownership. AI outlet owner: private same-outlet helper/RAII fix. Ordinary target adapter owner: actual fresh-Play channel and immutable ingress dispatch custody. No Speech/Core changes.

## Concrete production channel API after ordinary runtime audit

Existing production `KernelCompositeHost` privately prepares real child kernels from definition+registry. Its `admit_input`, `step`, `output_into`, `complete_output`, `cancel` and `active_plays` provide all mechanics; no Source/Sink test Back is needed. `KernelCompositeDefinition::from_authored_export` verifies checked Source/Plan/boundary identity. Minimal channel constructor additionally requires exactly one selected categorical Flow placement, exactly one mapped input/output, no other operation or Host Call, and exact profile port kinds. It creates/installs its own fresh observed factory, retains the sole reader and the resulting private host, and starts it. There is no constructor from caller-supplied already-current kernel/outlet/token.

Suggested NEW AI `integer_categorical_step/channel.rs` under kernel-operation-owners+ptr8:

```
PreparedCategoricalChannel<L>::prepare(definition, Arc<PreparedCategoricalStep>, slots)
  -> Result<Self, ChannelRefusal>
channel.submit(lease: L, exact_input: &[u8])
  -> Result<(), ChannelSubmitFailure<L>>
channel.step() -> Result<Option<CompletedCategoricalRequest<L>>, ChannelRefusal>
channel.pending() -> Option<&L>
channel.cancel() -> Result<(), ChannelRefusal>
channel.active_plays() -> &BTreeMap<HostId,ActivePlayId>
CompletedCategoricalRequest<L>::into_parts(self)
  -> (L, CommittedCategoricalInvocation)
```

Generic L is metadata to AI, not authority asserted by AI. Language L is private-field, nonClone, issued only by shared round traversal; it cannot be externally manufactured. `submit` moves L into private pending storage BEFORE actual ingress and retains exact submitted bytes/expected invocation there. A second submission refuses returning its unconsumed lease. Once ingress succeeds, errors/cancel preserve pending evidence; no fallible return drops an emitted request's lease. There is no public token injection path. On completion, exact output and invocation from its own outlet are checked against private pending input/expected sequence and actual kernel output; the same moved L is returned in a private completed object. Hence an old real token cannot be relabelled as another pending request: the adapter never accepts external tokens.

Preallocate input/output ValuePayload canonical buffers and outlet at constructor. `submit` copies into exact-sized input buffer without growth; kernel ingress copies into admitted storage. `step` advances existing kernel once cooperatively, uses output_into fixed storage, holds any taken token in pending state until output acknowledgement succeeds, then moves pending L/token into completion. No spin-until-done helper. Refusals preserve pending+token so accepted effects cannot orphan evidence; fail-closed terminal/cancel state prohibits another submission. Request sequence/inc exhaustion and capacity checked before ingress. Full input/output checks use bounded preallocated scratch buffers. Existing factory output framing sets exact lengths.

The ordinary composite currently mints its actual child active_plays with sequence0; the channel reports those exact retained IDs. This establishes fresh private runtime/outlet ownership, not globally unique Std issued-Play identity. A later Std owner can bind actual admitted containing Play before start; do not accept a freely supplied ID as a replacement for that custody.

Language proposal: `PreparedAnalysisRound<'owner>` retains owner/generation and cooperative replay progress; `next()` returns an enum with actual pure Source operation request or a moved private `PendingParserScore`. Language accepts only `CompletedCategoricalRequest<PendingParserScore>` for learned numerical progress, compares private pending lease context plus exact bytes/model selection, and consumes the token into retained round evidence. It cannot accept `AdmittedParserScores::new([i64;76])` or `(lease,token)` directly. Shared Source traversal must be retained as resumable state rather than replay-growing-prefix per poll; initial private loop extraction may use synchronous own-channel cooperative drive only in diagnostic preparation, never claim installed controller or unbounded sealed work.
