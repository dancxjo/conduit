# Four-token learned transition scorer

This artifact is an averaged multiclass perceptron trained on the official
UD English EWT 2.18 train split, conditional on gold UPOS and gold syntactic
word segmentation. It predicts no lexical/POS analyses. Its numerical owner
adds seven categorical I16 embeddings into 76 I64 scores; Language source
maps those scores to transition proposals and retains legality/graph ownership.

Corpus: https://github.com/UniversalDependencies/UD_English-EWT/tree/b7711cce01cdd4f5fcc0a8199b8a50d951b16c0c
Treebank description and contributors: https://universaldependencies.org/treebanks/en_ewt/index.html
Source genres include email, social/web discussions, reviews, and weblogs.
The original treebank's CC BY-SA 4.0 text is retained in CORPUS_LICENSE.txt.
Derived annotation fixtures and the distributed learned artifact retain that
license and attribution. Corpus source text is not vendored.

Reproduce with Python 3 using no external dependencies:

```sh
python3 semantics/language/training/train_ewt.py \
  --corpus PATH_TO_PINNED_CONLLU_FILES \
  --output semantics/language/training/ewt_four_token
```

Fetch the three en_ewt-ud-{train,dev,test}.conllu files from the exact commit
above. The manifest retains all source SHA256s, exclusion counts, feature/
transition/vocabulary profiles, seed, epochs, precision, model SHA256 and
held-out oracle diagnostics. The binary contains a generic CI16SUM1 header
and output-major little-endian I16 weights, rather than executable grammar.

The runtime must admit its ModelArtifact, ModelSignature, exact immutable
resource, TensorValue identities, precision, and finite compute envelope.
Changing weights or signature is a different artifact, not a silent update.

Oracle action accuracy measures supplied gold states. It is **not** final
attachment accuracy, learned lexical accuracy, native execution proof, or
product acceptance. Native decode evidence and its tested profile limits are
reported separately. Full-sentence, >4-token, MWT and empty-node coverage is
outside this artifact. Scores are learned rankings, not semantic confidence.

The default native test checks exact model/tensor admission and a representative
held-out sentence through ordinary source feature/proposal/transition Plays.
It intentionally records the greedy decoder's dead end: learned class0 shift
ranks above class67 obj after the root verb. Feature indices, learned scores,
and source class decoding agree; a legal shift leaves an unassigned token with
no continuation. This is a regression proof of a limitation, not attachment
accuracy or successful beam-four decoding.

The 533-sentence native corpus diagnostic is explicitly ignored by default:

```sh
cargo +stable test -p conduit-language --test parser_learned \
  pinned_learned_artifact_scores_native_streaming_source_graphs_on_heldout_ewt \
  -- --ignored --nocapture
```

It is opt-in because this foundation reprepares ordinary graph instances per
sentence and the greedy profile can test all76 refused proposals at a dead end.
Native timing distinguishes source preparation, planning/lowering/owner/kernel
preparation, source graph transactions, and numeric compute; process RSS/HWM is
recorded on Linux. `CONDUIT_PARSER_NATIVE_METRICS_OUTPUT` optionally saves JSON.
The width-four lexical-alternative/revision runtime and scalable preparation
reuse remain follow-on work; no complete native corpus metrics are claimed here.

The manifest also pins the exact source feature/class contract using the
`language/parser-scorer-encoding@1` semantic digest. Preparation compares that
identity to the authored scorer source, and the callable ModelSignature/port
semantic kinds carry the pinned identity. The artifact descriptor pins that
signature digest as well as exact weight content; a changed source encoding
requires reviewed compatibility metadata instead of silent weight reuse.
