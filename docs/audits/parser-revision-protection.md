# Bounded symbolic parser commitment and revision

For #5753, the existing four-slot symbolic parser gains a checked Source
commitment law that updates active hypothesis states together with their
frontier. The existing legal transitions, five-comparator ranking network and
pruning law remain the owners of their respective decisions. Scores are bounded
proposals; learned scoring and automatic linguistic accuracy are separate work.

The supported profile admits four tokens and four hypotheses, scores between
-1,000,000 and 1,000,000, and a 65,536-byte revision request. Mechanical lexical
preparation retains the original Unicode-scalar spans, complete-token occurrence
lineage and exact source revision. Rebase protects committed dependents and their
head occurrences; unavailable correspondence refuses. The next analysis gets a
new basis while committed heads/relations remain unchanged and uncommitted
choices reset for ordinary legal transitions.

Commitment requires an agreed assigned edge in every active hypothesis through
the entire claimed stable prefix, a matching basis and matching previous
commitment. Stability must fit every active hypothesis's processed prefix;
requested commitment must fit the admitted stable prefix. Pressure, cancellation,
disagreement, wrong basis, invalid frontier and beyond-stable requests return named unchanged outcomes.
The eight Source stages retain the existing portable-expression decoder bounds;
no compiler bound was increased. Raw internal proposals require ordinary Native
admission before becoming an admitted state.

Focused native fixtures execute checked Source through actual Plan/Play on the
existing kernel and generic ConduitOS expression adapters compiled for the host.
They cover a partial two-word source with competing legal hypotheses, deterministic
ranking/ties, inactive high-score exclusion, pruning, actual commitment followed
by text revision/rebase/ordinary transition, and preservation of the committed
source and edge. Streamed refusal frames share one Plan/Play. The rebase fixture
also checks actual full input pressure and exact normal output closure. Resource
fixtures refuse five tokens and an oversized but otherwise legal lexical inventory.

The issue acceptance maps to the following actual execution fixtures:

| Requirement | Evidence |
| --- | --- |
| Competing bounded hypotheses before punctuation | `partial_prefix_keeps_legal_alternatives_then_rank_prunes_deterministically` prepares the partial source and executes existing ranking/pruning Source. |
| Later context revises only uncommitted facts | `actual_commit_then_text_revision_rebases_and_revises_only_uncommitted_edges` chains Source commitment, actual revision admission, Source rebase and existing legal transition. |
| Exact pressure, cancellation and frontier behavior | `disagreement_pressure_and_cancellation_leave_every_hypothesis_uncommitted` streams six refusal frames through one Plan; `claimed_stability_cannot_extend_beyond_survivor_agreement` checks disputed, unprocessed and beyond-stable prefixes. |
| Finite admission and transport | Revision fixtures check committed head correspondence, five-token refusal, encoded request ceiling, actual input Full and normal output closure. |

This is an in-process native fixture proof. Live editor transport, a learned
parser, natural-language accuracy, guest/physical ConduitOS execution and parent
#4907 acceptance retain their own requirements. The original retained branch is
still useful: only its reviewed committed dependent/head correspondence and
finite rebase idea were recovered, rather than its independent learned-session
subsystem.

Review caught a concrete stability error: a request committing only edge zero
could claim edge one was stable despite survivor disagreement. The regression
failed before correction, then passed after consensus covered the full stable
prefix and stability was bounded by every active processed prefix. Existing
compiler, parser-action and numeric bounds remain unchanged.

The corrected source `1b763d11fc85bdb24deef358386b66b402133a7c` passed
89 tests across 33 package targets with no failures or ignored tests. Strict
all-target Clippy passed with warnings denied. The retained
[evidence receipt](evidence/parser-revision-protection.json) pins source files,
commands and complete validation logs. The pre-correction suite passed 88 tests; the added
regression is included in the corrected complete suite. A later documentation
commit is not the source of those measurements.
