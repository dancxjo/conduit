# Separate finite availability-aware joint scorer

This profile preserves the v1 official-corpus artifact and evidence. `reviewed_teaching.json` contains three explicitly supervised training-only teaching examples approved for the named-vocative demonstration. They are authored annotations, not official UD corpus rows and not held-out evidence.

`reviewed_generalization.json` contains separately retained varied-name/greeting/word-order evaluation cases. Their lexical alternatives must be admitted from the separately named reviewed lexical profile; these names are outside the v1 TRAIN-only64 profile. Evaluation must report actual model predictions and must not substitute these reference arcs for learned output. These small authored cases do not establish broad English or out-of-vocabulary accuracy.

V2 candidate-set lookahead consumes only the same admitted complete available lexical prefix. It exposes candidate membership without selecting or committing a future lexical alternative. Finality is derived from the exact source tape. No native streaming or scorer acceptance is claimed before its Source runtime proof and artifact identity validation.

The authored feature graph and exact native artifact tests pass: 25 categorical lookups, 413 feature categories, 76 action classes, 1900 integer multiply/add work units, and a conservative 128525 absolute score bound. A second independent training run reproduced the model, profile and manifest byte for byte. Source tests verify that future candidate choices cannot change candidate-set presence and that unavailable future tokens contribute no membership. These tests are feature/admission evidence; blind graph metrics are recorded separately.

Reproduce training with Python 3 and the pinned external official corpus: `python3 semantics/language/training/train_ewt_joint_v2.py --corpus /path/to/ewt --output /path/to/output`. The corpus directory contains the exact `en_ewt-ud-{train,dev,test}.conllu` files whose hashes appear in the manifest. The trainer uses the standard library, seed 4907 and twelve epochs; it reads only TRAIN and the explicitly reviewed teaching cases for weight updates. DEV and TEST annotations are evaluation data. No full corpus is vendored.

The v2 lexical profile fits the unchanged 64-entry native capacity: the 57 most frequent eligible TRAIN forms excluding the seven explicitly reviewed forms, plus those seven reviewed alternatives. Its covered official splits are TRAIN 394, DEV 53 and TEST 80 before canonical tokenization exclusions. This selection differs from v1 TRAIN64; v1 artifacts and evidence remain separate.

The three reviewed teaching sentences receive an explicitly recorded sampling weight of 16 repetitions during TRAIN updates. Their conditional diagnostic is separate from blind native graph metrics. The first unweighted native run selected correct POS8/8 but attachment6/8 and vocative2/3; its initial 'Travis Hello' graph was wrong. That failure was retained rather than replaced with reference arcs.

The six manual held-out cases change names, greetings and noun forms while retaining the reviewed POS/attachment patterns. The learned feature encoding represents POS and transition context; this evaluation tests transfer across these admitted alternative sets. It does not establish unseen syntactic structures or unreviewed-name coverage.
