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
For a separately trained candidate, supply `--teaching /path/to/exact-teaching.json`.
Its digest must match that candidate's manifest; the default remains the original
eight-row teaching input. A mismatch refuses before creating evaluation output.
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
The separate vocative candidate uses twelve TRAIN teaching rows and a different
64-entry profile. Its verified reference preparation yields 3 DEV and 5 TEST
instances; these counts are not Native decoding results. The Native runner defaults to the original embedded teaching input. Set
`WINDOW8_TEACHING_ROWS=/path/to/exact-teaching.json` for a candidate; its manifest
must pin `teaching_content_identity` using `language/parser-teaching@1`. The
runner verifies that identity before Source/model preparation. Legacy manifests
without this field permit only the original embedded teaching input and its
original SHA-256. Actual candidate decoding remains a separate gate.

`--authored-rows proof/evidence/language/window8-vocative-evaluation.json`
adds a separate eight-instance slice pinned to the vocative candidate's exact
artifact. It includes initial, medial and final addressees and four ordinary
object uses of the same proper name. Preparation verifies the reference profile,
lexical coverage and exact sequence exclusion against all pinned TRAIN and
teaching data. These analyst-authored references share vocabulary and templates;
they are not independently reviewed corpus gold or broad English evidence.
No oracle actions are exported. Use `authored_vocative.json` as the external
reference input and the candidate's exact `WINDOW8_TEACHING_ROWS` when replaying.
Preparation alone does not establish any vocative precision or recall.

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
punctuation. Subtypes are excluded from base LAS. Vocative edge precision/recall
requires the exact dependent occurrence, governor and universal base relation.
A predicted vocative with the wrong governor counts as both a false positive
and a false negative when the reference also marks that occurrence vocative.
A zero denominator is reported as null, never a perfect score. Model failures remain evidence;
external mode does not assert that every reference graph was predicted correctly.
An aborted run is incomplete evaluation, not a successful zero-error result.

Neither preparation nor a no-run compile proves held-out accuracy. A final-only
replay also does not establish prefix reanalysis, stabilization delay, committed
contradiction percentage, memory admission or useful streaming latency. Those
remain separate #4907 gates. No new corpus decoding was performed to validate
this preparer; its retained result checks inputs and yields the counts above.
