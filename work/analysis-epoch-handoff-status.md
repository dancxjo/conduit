# Prepared implementation handoff (UNBUILT)

Isolated branch prep/4907-analysis-epoch-handoff at base97bf3e4d5.

Implemented preparation surface:
- Existing parser initializer unchanged; replay compares the complete observed
  numeric result. A tiny new Source plot declares one-active/four-slot scorer
  initialization, including exact zero score/choice/selected values.
- Private AnalysisEpochOwner owns same AnalysisSession, all four scorer slots,
  lexical preparation, generation and retained opaque commitment references.
  append_event cannot commit; append_commitment compares entire private parser
  beam and lexical tape to actual opaque replay, pre-admits all four replacement
  slots preserving choices, then updates journal/beam/generation transactionally.
- Consuming handoff requires FOUR opaque rebase replay receipts (including inactive
  slots). Exact prior states, old/new complete lexical tapes and common new basis
  are checked against actual owner. Existing rebase Source owns graph changes;
  separate new authored envelope owns named fresh-revision score/uncommitted-choice
  reset while preserving identities, active membership and committed choices.
  No survivor is discarded/coalesced by Rust.
- New journal starts at retained committed cursor via crate-private constructor;
  no current proposal/receipt is minted for carried facts. Old owner moves into
  archive exposing only immutable evidence/history, never current()/append().
  Failed handoff returns original owner. Caller owns finite archive storage; this
  wrapper creates no archive queue or claim of available outer storage.
- Three focused prepared tests cover exact init, forged observation refusal and
  actual single-expression rebase of all four initial slots across a zero-prefix
  epoch. Source/Plan/Play and nonzero-commit integration are not yet proved.

Real remaining gap (intentional): NO general learned decoder update API. Neither
arbitrary Native beam nor installed-current bool can replace private state. The
current owner can initialize/rebase and admit lifecycle/commit receipts, but cannot
reach a nonzero legal commit from initial state until root adds actual opaque
Source stage custody for branching/transition/model/rank/merge/agreement. Therefore
nonzero committed handoff is implemented structurally but not reachable/proved by
these tests; do NOT call this a complete installed streaming parser. A test-only
setter would defeat this boundary and has not been added. Existing external
AnalysisSession supplied-graph tests remain separate preparation evidence.

Old-epoch correction and newly-current access to carried committed facts also need
explicit cross-epoch owner law if required. Archived epochs cannot be mutated;
this slice never re-proposes protected occurrences or relabels old arcs as current.
Runtime request cancellation/stale asynchronous result admission, selected model
profile/resource identity, installed ingress and output playback remain target
controller obligations. No second runtime/kernel/parser or Core changes introduced.

New production files remain below500 lines. Rustfmt and git diff --check pass.
No Cargo/build/tests/CI/GitHub polls. Root should review Source envelope/catalog
integration and compile only through coordinated queue; Source identity changes
require root's later single carrier custody refresh.

## Dynamic Native construction audit

Both new Source operations return existing unrestricted numeric/raw forms:
LanguageAnalysisInitialRawBeam contains ONLY LanguageParserScorerRawHypothesis;
that contains LanguageParserRawHypothesis with LanguageParserNumericState. The
rebased envelope returns the same raw hypothesis. Neither constructs refined
LanguageParserState/ScorerHypothesis/AdmittedBeam in pure Source. Rust copies
Source-selected fields into existing generated Native constructors after replay;
all original owner laws remain active. Input prior hypothesis is already admitted
and unchanged. This follows the earlier CND-FRM046 repair boundary, not a checker
exception. Actual checker/build acceptance still requires root's coordinated run.

Next-transition proposal is separately retained in
work/analysis-epoch-next-transition-custody.md; not part of implementation patch.
It requires complete-round custody, since one legal selected successor cannot
justify dropping other active survivors. Smallest first mode can explicitly be
Source-owned zero-score fallback; learned mode additionally needs opaque actual
model-feature-score receipt correlation. No arbitrary setter is proposed.
