# Finite English parser-action artifact (#5722)

This is a preparation slice. It exports 76 action scores, never a finished
parse, dependency edge, POS truth, stability decision or commitment. Grammar
and legal transitions remain in `semantics/language/parser.conduit`; checked
beam/frontier contracts remain in `parser_beam.conduit`. Integration with these
contracts through ordinary Plan/Play is #5754 and is not established here.

## Reproduce and verify

```
cargo xtask check parser-scorer
cargo xtask check parser-scorer-reproduce
```

The first command exercises the existing `IntegerCategoricalModel` numerical
owner, exact artifact/signature admission, held-out ranking and negative cases.
It prints measured numeric latency, invocation count, working storage and exact
counts. The second requires Python 3 and network access: it downloads the three
pinned corpus partitions into temporary storage, checks their SHA256s before
training, retrains from scratch and compares every generated file byte for byte.
It never updates the retained artifact. Training uses only the Python standard
library; no parser runtime, tensor framework or transformer is introduced.

## Source, partitions and license review

Recovered narrowly from PR #5196 at
`c02b31605203300773bc40674be186b7e8655453` under recovery issue #5388:
`train_ewt.py` oracle/training helpers, the v2 trainer and three separately
supervised teaching rows. None of the archived Session, Native, Speech,
Window8 runtime, old metrics or parser orchestration is recovered.

Corpus: [UD English EWT pinned source](https://github.com/UniversalDependencies/UD_English-EWT/tree/b7711cce01cdd4f5fcc0a8199b8a50d951b16c0c).
The original attribution/contributor README and CC BY-SA 4.0 license are retained
in `ewt_joint_v2/CORPUS_README.txt` and `CORPUS_LICENSE.txt`. License review checked
redistribution, attribution, indication of modifications and ShareAlike terms.
The learned weights and derived annotations are distributed under CC BY-SA 4.0;
retain these notices, source link and license when redistributing them. This is
an engineering license review, not a claim of upstream endorsement. Modifications
are finite sentence filtering, base-relation projection, lexical profile
selection, supervised teaching, perceptron training and I16 quantization.

Official TRAIN alone selects the 57 most frequent eligible exact-case forms,
plus seven explicitly authored reviewed forms. Up to four TRAIN-observed POS
alternatives per form are retained. Three archived teaching rows receive sixteen
training repetitions. They are TRAIN supervision, including vocative examples,
and are never held-out evidence. `Hello, Travis.` itself is absent from training.
No DEV or TEST weight updates or hyperparameter selection occur. Seed 4907,
twelve epochs and scale 256 are frozen from the archived recipe.

Evaluation excludes sentences with the same case-folded token sequence in TRAIN,
teaching or an earlier official partition, and duplicates within each evaluation split. This removes substantial repeated
text: eligible profile coverage is TRAIN 394, DEV 5, TEST 5 sentences after
exclusions. The manifest records all original eligibility and exclusion counts.
Tiny coverage is a measured limitation, not broad English performance.

## Exact encoding and bounds

`ewt_joint_v2/schema.json` specifies every category index, POS/relation order,
action class, binary byte layout and tie rule. `lexical_profile.json` is the
complete finite vocabulary. `manifest.json` pins corpus hashes, teaching hash,
schema and lexical semantic identities, weight tensor identity, immutable model
content identity and SHA256. The model is 62,796 bytes: 413 categories x 76 outputs
of I16 weights plus a 20-byte header. Twenty-five U64 indices select exact I64
sums: 1,900 lookup/add units, 200 input bytes and 608 output bytes per invocation.
`maximum_absolute_score` bounds all possible index combinations, including
repeated categories. Changed content/signature and invalid indices refuse.
Prepared numeric inference uses caller-owned fixed output storage. Its reported
working bytes exclude whole-process heap and preparation; no process memory
ceiling or allocation-free preparation is claimed.

## Evidence and limits

`ranking_cases.json` retains all held-out gold-state candidate features and
reference actions so Rust numeric execution independently reproduces Python
joint action ranking: TEST 25/26 (96.15%), DEV 22/22. With future lookahead
withheld, conditional incremental TEST action ranking is 14/16 (87.5%), DEV
11/13. These are gold parser-state/POS diagnostics, not blind interpretation
of incremental text, calibrated confidence, UAS/LAS or vocative precision/recall.
The latter require actual selected graphs and remain #5754 acceptance work.
Failures are retained in the cases and counts; no successful final parse is
inferred from action accuracy.

Unsupported: unknown forms, more than four tokens, multiword/empty-node UD rows,
non-projective/unreachable trees, POS absent from admitted alternatives, relation
subtype prediction, general English and autonomous lexical discovery. No
speech/prosody behavior, public Session, stable frontier or committed frontier
is claimed. Numeric microbenchmarks exclude feature extraction, Plan preparation,
checked transitions and end-to-end latency. The exact source revision, lexical
choice, model and analysis custody must be proved by #5754 before downstream use.

`numeric_evidence.json` retains the local unoptimized Rust observation: 63 numeric
invocations, 866,905 ns total, 15,412 ns maximum, 1,368 reported working bytes,
and 6,364 KiB process RSS/HWM. These are numeric microbenchmarks, not parser
latency or a whole-process memory bound.

`numeric_failure_evidence.json` retains the subsequent run with the actual failed
TEST decision: `email-enronsent04_02-0007` selects class 73 (right-arc/root) instead
of reference class 0 (shift). It reports 63 calls, 1,408,331 ns total and 23,734 ns
maximum; the earlier observation is preserved. Timing variation does not alter
the 25/26 result. No reference action replaces the selected action.
