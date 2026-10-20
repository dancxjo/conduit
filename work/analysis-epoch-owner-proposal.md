# Minimal Language epoch owner: proposed implementation boundary

Base97bf3e4d5c40f93269766c6e4ab388509298713e. Isolated branch
prep/4907-analysis-epoch-handoff; no Core, pronunciation or stable-tree edits.
No build/installed proof claimed.

## Private production state and safe storage

AnalysisEpochOwner<'arena> composes AnalysisSession<'arena> (the SAME authoritative
journal), owned PreparedLexicalTape, all FOUR private scorer hypotheses including
choices/selected, a monotone generation and immutable initialization/commit/rebase
receipts. Caller-owned immutable domain/context/event storage outlives owner and
archives. No Vec borrowed by its own enclosing owner, unsafe storage or leaks.
The session is private; accessor methods only mint borrowed CurrentCommittedDependency
through the existing session or expose immutable frontier/history. Neither a public
already-current beam constructor nor a caller current flag exists.

## Narrow initialization entrance

PreparedAnalysisInitializationProgram prepares existing language-parser-initialize
once. prepare(owned lexical, exact LanguageParserBegin, actual numeric output)
checks source/basis/count against complete lexical availability, replays that
unchanged Source, compares complete output, field-copy admits LanguageParserState,
and requires zero committed frontier. Source identity/program/input/output retained.
Runtime observation equality proves semantic custody, not installed execution;
the target runtime separately retains its selected Plan and actual observation.

The surrounding four-slot scorer initialization must also be authored Source:
new tiny envelope plot consumes the Source-initialized state and emits one active
hypothesis, three inactive, deterministic IDs1..4, score0/choices0/selected0. It
uses existing Types and introduces no graph or grammar policy. Its raw output is
re-admitted through owner Native constructors rather than weakening refinements.
This is a declared deterministic initial envelope, not learned output. Alternative
IDs/profiles require distinct declared preparation; caller cannot inject states.

AnalysisEpochOwner::initialize(initialization, domain, context, limits) constructs
journal at0 only after all exact checks pass. Preparing initialization alone mints
no current dependency. No accepted-result Boolean is trusted.

## Commit and update custody

prepare_commit(&self, request, actual) requires request's entire parser beam
(identities, scores, active flags, all state fields) equals projection of all four
private scorer slots. Existing opaque PreparedParserCommitment replays actual
Source and binds full output. Apply all four actual parser results, preserving
unchanged lexical choices and selected counters. Query/source/analysis/full lexical
custody additionally uses existing PreparedCommittedDependency. Journal commit,
beam replacement and generation advance must be transactional: build/validate next
beam before journal append; only then assign fields, with no fallible operation
remaining. Cancellation/pressure/history/window failure changes none of them.

A decoder update cannot accept arbitrary Native beam plus digest. Need private
prepared update objects binding owner generation, exact prior beam/lexical/profile,
all actual Source observations and exact execution/replay. Existing branching,
rank/selection, merge, legality, agreement and scoring Source remain owners. The
single initialization plot and commit replay do NOT establish full learned decoder
custody. A comprehensive update pipeline is a separate cohesive controller step;
it is a real prerequisite before committed epoch handoff becomes reachable from
this initially uncommitted owner. No unsupported placeholder should pretend to
supply it.

## Consuming handoff

PreparedAnalysisEpochHandoff binds self generation, full prior beam including
inactive slots, exact source/profile/model identity, retained committed choices,
full lexical tape and prior actual commitment. For EVERY active slot retain one
PreparedParserRebaseReplay whose request.previous equals that private slot state;
request.previous_lexical equals full owner tape; next lexical/new analysis identical
across receipts. Missing/extra/foreign active slot refuses. Preserve every slot's
identity/active flag and committed choices. Any score reset/coalescing happens
only in a separate named checked Source phase, not Rust default arithmetic.

handoff(self, prepared, next_domain, fresh_context, limits)
 -> Result<(AnalysisEpochOwner, ArchivedAnalysisEpoch), HandoffFailure<Self>>

Recheck all snapshot equality/generation and bounded archive capacity immediately
before success. Construct next journal with existing RevisionJournal::new at
common retained committed cursor, not0. The next session has no current proposal
or current admission: old arcs name old Source/analysis and stay historical.
Do not re-propose protected occurrences. A future carried-current-fact API would
need explicit owner law; it is not supplied by this patch. New uncommitted token
facts can use ordinary checked proposal/stable/commit later.

Success consumes old owner; archive keeps private session and immutable receipts
behind history-only methods, never current()/append(). Borrowed Current authority
prevents moving the old owner during publication. Failure returns unchanged old
owner including beam, choices, generation, receipts and journal. Fallible checks
finish before move/archive install. Caller reserves archive/arena slots first.

## Proposed files and integration

NEW semantics/language/src/analysis_epoch_initialization.rs (<500): checked program,
opaque raw/admitted initial receipt. NEW analysis_epoch_owner.rs (<500): private
owner, initial constructor and immutable authority views. NEW authored scorer
initialization-envelope Source only after root approval. Language build.rs/lib.rs
wiring confined to isolated branch; root can apply minimal hunks. Existing
AnalysisSession gets only crate-private checked initial-frontier constructor if
handoff is implemented; no public arbitrary nonzero initializer.

Tests must independently preserve all four slots, reject nonzero init/foreign
basis/output/token count, compare program replay, preserve failure atomicity,
reject omitted-survivor handoff and committed edits, and ensure archive cannot
mint current authority. Actual installed Source/model/consumer acceptance remains
root-owned proof, not inferred from preparation tests.
