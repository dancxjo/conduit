# Punctuation-supervised scorer preparation

This is an unaccepted, opt-in model candidate. Accepted `ewt_joint_v2` files,
metrics, known false-positive vocative and comma/frontier failures remain intact.
No Rust runtime, checked consensus rule, pruning law or acoustic policy changed.

The new trainer uses the same 413 features, 25 lookups, 76 actions, 64-entry
lexical profile, exact I16 weights / I64 sums, seed 4907, 12 epochs and reviewed
teaching repetition factor 16. Ten supplied greeting-context annotations replace
the old three teaching rows. They use Hi/Alex/Morgan/friend/pal; no Hello/Travis
motivator annotation is trained. Any EWT TRAIN row containing both Hello and
Travis is explicitly excluded and listed in the manifest. The original EWT
corpus digests are checked before training; DEV/TEST annotations remain separate.

Authored annotation choices and repository MIT licensing are recorded in
`teaching-provenance.json`. They are proposed reviewed supervision, not an
independent human annotation signoff. Punctuation attaches to the adjacent
addressed unit under the [UD punctuation guidance](https://universaldependencies.org/u/dep/punct.html);
the addressee links to the host under [UD vocative guidance](https://universaldependencies.org/u/dep/vocative.html).
These are orthographic/syntactic labels, never silence or acoustic controls.
Greeting interjection ROOT and name vocative is a deliberately supplied reading;
other corpus readings remain possible and the old contrary held-out example
must remain visible.

`reserved_evaluation.json` was fixed before this training job. The Hello/Travis
motivator is already observed from the prior campaign and is not newly blind.
Other reserved name/greeting/punctuation combinations have not been used to tune
this candidate. Existing EWT DEV/TEST are **previously observed regression
checks**, not newly blind evaluation. Do not tune to reserved results and then
continue calling those rows independent. Forms, POS and reference edges enter
training or later metric comparison only, never runtime selection.

Reproduce with one CPU (choose an allowed CPU in taskset):

```sh
PYTHONHASHSEED=0 taskset -c <allowed-cpu> python3 semantics/language/training/train_ewt_punctuation_v3.py \
  --corpus <pinned-ewt-directory> \
  --output <fresh-output-directory> --epochs 12
```

Compare model bytes, manifest identities, lexical profile and ranking cases with
this retained directory. The trainer's Source SHA256 and base trainer SHA256 are
in the manifest. Copying the prepared model into the accepted V2 path would
misrepresent its custody and is forbidden by this preparation contract.

The training script reports conditioned gold-state/action/POS diagnostics only.
It performs no actual checked beam decode and cannot demonstrate agreement,
commitment, incremental readiness or trained voice. A changed metric here is
not proof that the comma gap is fixed.

## Required next actual Source evaluation

The existing `cargo +stable xtask check parser-selection` remains an accepted-V2
check and does **not** select this artifact. The separately named opt-in
artifact descriptor/admission and retained profile test below use the same
generic model compute owner; V2 selection stays intact. Root queues the native
build after this independent Python job. No environment override selects V3;
do not claim the old aggregate command evaluates V3.

Then run that explicit profile test through the existing checked legal-action,
ranking, selected-transition, four-survivor beam, agreement, advance, actual
commit and new shared AnalysisSession operations. Retain every survivor's score,
identity, lexical choices, heads and relations at each available prefix. Raw
scores are uncalibrated. Do not introduce top-one or selective pruning to hide a
remaining disagreement.

Evaluate all unchanged previously observed EWT DEV/TEST rows, their old failures,
and every reserved row/prefix. Report final UAS/LAS/POS/vocative metrics, per-token
revision and stabilization/commit delay, contradicted committed edges, all
refusals and resource bounds. The first critical result is partial
`Hello, Travis ` (three tokens, no sentence-final punctuation): all surviving
admitted hypotheses must agree through the vocative before actual Source commit
can advance the current session. If they disagree, retain that negative and
leave the product acceptance open. Applying a commit observation only to a JSON
report without advancing the owning decoder/session is insufficient.

## Explicit evaluation entrance prepared (not run)

The separately named `parser_punctuation_model` module and
`parser_punctuation_selection` test now provide the opt-in entrance. Default V2
selection and evidence remain untouched. The shared test drivers accept explicit
metadata/bytes/preparation function parameters; there is no environment switch
for changing the adopted model. The report variable below controls only output.
Root owns the coordinated cache/build queue:

```sh
CONDUIT_PARSER_PUNCTUATION_V3_REPORT=<absolute-new-report-path> \
  cargo +stable test -p conduit-language --locked --features parser-model \
  --test parser_punctuation_selection -- --nocapture --test-threads=1
```

This runs all five retained DEV and five retained TEST sentences plus all seven
reserved cases, each through incremental prefixes. It retains all survivor
scores, arcs, lexical choices, consensus/frontiers and generic numerical receipts.
The actual checked commit result is applied to the current beam before the next
revision's rebase. It counts failures in the denominator and retains diagnostics
before the final motivating acceptance assertion. Candidate success requires
committing through a **vocative Travis→Hello** on the partial three-token exact
`Hello, Travis ` revision, not merely returning a complete graph or any committed
edge. The test remains orchestration proof, not an installed editor/playback.

The artifact's loose arbitrary-index max-weight bound is 165675, above the old
parser range. `feature-group-bound.json` separately encloses all outputs from
the current checked 25-index feature construction by 25827, below the unchanged
Source range ±128525. The analyzer reproduces this bound by independent per-group
interval maxima; arbitrary categorical input vectors are not covered. No Source
score clipping or bound relaxation was introduced.

## Declared two-track inference profile

The candidate experiment now explicitly distinguishes independent uncommitted
prefix quality from a causal commit track. This is a new declared evaluation
profile as well as a new model; comparisons to the old observation-only V2
report do not isolate a model-only effect. Accepted V2/default entry behavior
is preserved.

At each causal text revision every active state passes the existing checked
rebase. Exact committed lexical choices survive; uncommitted choices may be
reconsidered. All seed scores reset to zero, so discarded old actions do not
contribute historical scores. Existing checked Source merge/rank orders every
retained seed before mechanical coalescing of **exact state+choice duplicates**;
all distinct seeds remain within the unchanged four-slot beam. The final
retained seeds again pass Source merge. No gold edges or punctuation policy
enter this step.

Causal commitment is consumer-driven: the motivating utterance requests the
Travis dependency through token3, and no root/comma prefix is locked early.
Only actual contiguous agreement/stability covering that request allows the
existing checked commit operation to run. Its actual output is applied to the
current beam before the next revision, retaining later refusals and corrections
as evidence rather than rewriting history. Other cases request the complete
input scope, independent of reference arcs. The quality track starts fresh for
each available prefix and makes no commitment requests. Both tracks retain all
survivor scores/choices/arcs, coalescing counts, agreement, frontiers, actual
commit results and per-token revision/delay metrics.
