# Exact Window8 evaluation references

Prepare inputs before starting expensive actual Native decoding:

```sh
python3 proof/evidence/language/prepare_window8_evaluation.py \
  --repo /path/to/conduit \
  --model /path/to/exact-model-directory \
  --corpus /path/to/pinned-ewt \
  --output /path/to/evaluation-references
```

The preparer verifies model bytes, lexical-profile semantic identity, pinned
TRAIN/DEV/TEST digests, reviewed teaching digest and reference extractor digest.
It excludes unsupported profile trees, vocabulary/POS alternatives outside the
selected profile, and exact form sequences found anywhere in TRAIN or teaching.
It writes no oracle actions. Reference labels are evaluation inputs, never
constraints on model proposals or Source transitions. Output text joins the
supplied UD forms with one space; original corpus whitespace is not claimed.

The current 64-entry profile leaves only 3 DEV and 6 TEST sentence instances
after these exclusions. Several instances share the same short surface sequence.
These are not nine independent lexical/structural challenges, a broad clause
benchmark, or vocative precision/recall evidence. Preserve all instance identities
and exclusions; report duplicate surfaces and cross-split overlap in evaluation.
A larger/general linguistic claim needs stronger coverage and its own frozen
model, profile, supervision and evaluation membership evidence.

Actual decoding uses the existing ordinary model and checked Source bank:

```sh
WINDOW8_MODEL_DIR=/path/to/exact-model-directory \
WINDOW8_EVALUATION_ROWS=/path/to/evaluation-references/test.json \
WINDOW8_EVALUATION_OUTPUT=/path/to/separate-native-evidence \
cargo +stable test -p conduit-language --features parser-model-selection \
  --test parser_window8_cached_corpus_model -- --ignored --exact \
  actual_cached_window8_reviewed_clause_decode --nocapture
```

The external-reference mode refuses reviewed teaching text overlap and the
model-directory output path. It retains actual Native states/model invocations
and reports all-token UAS, universal-base LAS and POS accuracy, including
punctuation. Subtypes are excluded from base LAS. Model failures remain evidence;
external mode does not assert that every reference graph was predicted correctly.
An aborted run is incomplete evaluation, not a successful zero-error result.

Neither preparation nor a no-run compile proves held-out accuracy. A final-only
replay also does not establish prefix reanalysis, stabilization delay, committed
contradiction percentage, memory admission or useful streaming latency. Those
remain separate #4907 gates. No new corpus decoding was performed to validate
this preparer; its retained result checks inputs and yields the counts above.
