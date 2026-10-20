# One matched causal-prefix objective ablation — proposal only

No training, inference, build, feature execution or production edits performed.
The independent role manifest remains quarantined and unchanged. This experiment
would legitimately REUSE394previously fitted TRAIN rows for supervised development,
not manufacture a blind calibration/confirmation set. Observed DEV/TEST and the
motivating greeting/reserved cases are evaluation-only, never tuning inputs.

## Frozen experiment

Same frozen64word profile,413features,25ordered indices,76classes, I16weights/
I64sum ABI, seed4907,12epochs, quantization256, finite score/resource/beam limits,
Source legality/agreement/retention. Fresh-from-zero weights in BOTH arms. Do not
compare a new model against an unmatched old artifact as the causal effect.
No teaching augmentation in either arm: the matched cohort is exactly reconstructed
394TRAIN rows, identical membership/order. These existing TRAIN rows are NOT
independent data for calibrating frozenv3. Profile/POS alternatives were selected
using global TRAIN; disclose prior resource and analyst exposure. No probability,
new confidence margin, default switch or general quality claim follows.

Freeze raw/profile/trainer/membership hashes from the adjacent prior reconstruction
manifest. Record new generator/Source/Native identities before producing samples.
The prior393excluded/onequarantined result concerns INDEPENDENT partitions; do not
confuse that exclusion rule with authorization to reuse already declared TRAIN for
this explicitly nonblind matched training-objective experiment.

## Critical state seam verified in current Source

`parser_scorer_features.conduit` query requires state.token_count == lexical.token_count.
Feature count comes from this actual current count, not full utterance size.
Lookahead membership is available ONLY when unread+1 < lexical.token_count.
POS choice projection excludes unselected future occurrences. Native choice
bounds refer to actual lexical candidate lists; padding is not a new occurrence.

`parser_revision_rebase.conduit` rebase RESETS uncommitted structure: unread becomes
committed, uncommitted heads become5, stack is reconstructed from committed
structure. It does not preserve a speculative full oracle stack. Therefore a
naive generator carrying its teacher stack through arrival and merely changing
n is wrong. The first bounded proposal uses committed0 throughout sample
preparation: each prefix revision executes actual rebase to a fresh canonical
initial(p) state and replays a prefix-valid teacher path. Do not claim training
on actual learned-survivor commitment/rebase decisions from this conditional
teacher data. Evaluation must still use actual whole-history Source machinery.

## Deterministic prefix query and teacher path

For each TRAIN sentence of lengthN<=4, create arrival revisions p=1..N in order.
Construct real SourceText/lexical preparation for exactly the FIRST p occurrences
using existing lexical/profile owner; retain exact spans/occurrence/revision and
available-prefix receipt. If an unterminated token has no admitted availability,
record no-available-occurrence and emit no sample. Use explicit existing source
finality/stability contract; a trailing space is not an invented commitment.
The synthetic token-arrival schedule must be frozen and separately identified
as supervised TRAIN schedule, not recorded natural ASR/user timing. The full
sentence/gold labels remain an OFFLINE supervision object outside query bytes.

At each revision use checked begin/rebase output with committed0, n=p; no future
heads/POS/count/candidate lists in canonical query. Generate the sentence's pinned
static oracle class sequence offline using original gold tree. Replay from the
fresh prefix state, taking that same sequence only until the first action whose
required token has NOT arrived. Gold future supervision can tell the generator
where to stop; it cannot supply a missing occurrence or accepted transition.
All state transitions execute existing checked Source before the next sample.
Do not copy full-oracle state with n relabeled. Heads already made by replay
must refer to available occurrences/root only; unknown heads remain5.

For each next offline class:

- SHIFT requires unread<p, valid current lexical choice; if unread==p stop
  `no_arrived_buffer_token`, emit NO action sample.
- LEFT/RIGHT arc requires actual available endpoints and Source-accepted result;
  if buffer endpoint is outsidep, stop `future_endpoint_not_arrived`. Gold future
  head is supervision, never an index installed in current state/query.
- REDUCE may be supervised only when it is the pinned full-gold oracle's next
  action and actual Source accepts it; its offline child-completion condition
  prevents deleting a node before an unseen required dependent. Never choose
  REDUCE merely because the current truncated buffer is empty.
- If next oracle class is otherwise Source-illegal, emit refusal metadata and
  STOP that revision, not a substitute action. Treat an unexpected legality
  mismatch on an arrived target as a generator defect, not trainable label.
- If full oracle ends, record `teacher_complete`; if prefix pauses with no legal
  gold-compatible arrived action, record `await_arrival`. No WAIT output class,
  fabricated SHIFT, forced finish/root or negative label for all actions.

Real runtime may explore legal alternatives at these boundaries; this proposal
does not claim conditional teacher samples teach abstention. If missing WAIT/
arrival semantics prevents the desired runtime behavior, report the contract gap
rather than repurposing a class. No class/Source/runtime change belongs here.

## Matched sample pair, objective and provenance

Every emitted arrived/legal teacher target produces TWO paired training records,
with identical sentence/prefix/path-step/target and same teacher topology.
ArmC uses exact causal query just described. ArmF uses a separately constructed
valid full-available query (n=N, full lexical resources), with the same replayed
arrived actions and gold supervision; execute legality again. The only intended
intervention is visible context/query support. This matched exposure baseline
is distinct from verbatim historicalv3, whose sampling multiplicities differ.
Report that distinction explicitly. Pair samples one-to-one, identical shuffle
and update budget across12epochs; no per-class/greeting weights or margin fitting.
Enumerate lexical alternatives from each arm's ACTUAL admitted candidate support
and existing selection semantics; retain feature duplicates/multiplicity policy
explicitly. Do not include future positions in causal Cartesian products.

Record per sample: pinned sentence/content ID; source/lexical/analysis revision;
prefix/available count; oracle sequence index/target; full-gold supervision hash;
prior state bytes; actual transition request/result bytes and accepted receipt;
choice-query canonical bytes; actual feature-program/SourceID;25indices; arm;
pair ID. Refusal/paused histories get records too and stay in coverage denominators.
No gold heads/POS appended as hidden features. This remains conditional static-
oracle training, not learned search roll-in or beam-aware training.

## Mandatory Source parity fixture BEFORE any model training

Use existing Language SourceGraph/kernel helpers; do not implement another
transition/feature runtime. For every emitted query run actual
`language-parser-scorer-model-features`, retain all25indices in order (duplicates
included), epoch/basis and canonical bytes. A proposed offline encoder must
match them EXACTLY, not only the seven legacy state features. Positive cases:
root sentinel top; unread0/intermediate/p; available lengths1..4; absent/present
next-occurrence support; multiple current POS alternatives; popped/assigned heads;
rebase-reset after each arrival. Current Source's choice visibility rules are
authoritative even where they differ from naive full POS Cartesian products.
Negative cases: fulln with prefix lexical tape; out-of-support choice; future
head in current state; foreign occurrence/revision; unavailable SHIFT/arc;
future candidate membership injected into a causal feature record. Native/Source
must refuse invalid queries/actions; no manual accepted flag.

Static oracle heads/relations are offline labels only. Demonstrate at least one
future-head pause and one later successful replay when that endpoint arrives,
with no emitted phantom WAIT/SHIFT label. Pin exact query/output fixture manifest,
counts by prefix/action/pause reason and paired membership hash before model run.
If actual lexical availability/rebase semantics cannot construct any proposed
record, quarantine it and report exact seam; do not adjust Source or limits.

## Predeclared reporting and limits

One candidate only, no sweep/epoch selection using held-out greeting. Run paired
TRAIN full causal histories as nonblind development; separately report previously
observed DEV/TEST/reserved/greeting as nonblind regression. Report full-history
any-false-commit, occurrence/head/relation/lexical errors, final exact-tree/LAS/UAS,
consumer span commitment before later punctuation, uncommitted/refused counts,
revision delay, correct-path survival, all-survivor disagreement and calls/cost.
A ranking/gold-state gain alone does not meet streaming acceptance. Preserve
negative histories and the unchanged0/0/0/3baseline record. The one ablation can
falsify the proposed causal-context benefit, not validate new calibrated pruning.
No blind confirmation exists in the current quarantined pool; no acceptance or
publication/default decision should pretend otherwise. Four-slot retention can
remain the limiting factor even if correct scores improve. #5755 untouched.
