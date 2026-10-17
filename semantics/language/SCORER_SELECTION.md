# Finite checked parser action selection

The optional `parser-model` feature admits the exact English artifact accepted
under #5722. `prepare_english_parser_scorer` returns the existing generic
`PreparedCategoricalStep`; it does not parse text. Absence, foreign content,
signature, descriptor, residence and resource bounds refuse explicitly. The
admitted resource reference is compared in full, including custody metadata
that the generic descriptor digest intentionally excludes.

Checked Language Source retains the linguistic policy:

- `parser_scorer_features.conduit` constructs the artifact's exact 25 categorical
  indices from current admitted lexical alternatives and parser state. A future
  token contributes available alternative membership, never a chosen future POS.
- `parser_scorer_actions.conduit` maps 76 class identities to ordinary parser
  requests. Numerical output is 76 I64 scores, never a tree or linguistic fact.
- `parser_scorer_ranking.conduit` defines the legal mask and deterministic
  fallback. `parser_scorer_tournament.conduit` ranks all 76 masked classes using
  75 comparisons in eight Source nodes, preserving lower-class ties. The proof
  compares its canonical output against the original scan on empty, negative
  and tied-score cases. Five checked transition results supply root/nonroot action groups; the
  selected action is checked again through `language-parser-transition`.
- `parser_scorer_search.conduit` branches only at the current buffer token,
  refuses cumulative score overflow, and retains a deterministically ordered
  four-slot beam. Native admission validates every resulting checked state.

The public selected-transition Source compound uses the existing typed finite
Zip owner with 16384-byte Selected and Result envelopes and its computed
53414-byte pair envelope. It carries the exact prior state and selected score
into the checked transition, then publishes a checked expansion.

The deterministic baseline uses zero scores with the same checked legal-action
ranking. Admission refusals and this baseline are exercised separately;
automatic live-editor fallback routing is not supplied. It has no learned model receipt. Its output can be incomplete or wrong;
zero scores are not confidence or evidence of grammatical correctness.

`cargo xtask check parser-selection` exercises the public admission bridge and
Source operations using ordinary planned generic numerical and expression Backs.
The hosted orchestration in `tests/common/parser_selection_*` is a finite proof
runner, not another production parser runtime. It does not inspect reference
edges to choose actions. Actual retained test annotations enter only the metric
comparison after decoding. Source accepts bounded numerical proposals; the
hosted proof joins their exact input/output digests and invocation receipts with
source/analysis basis assertions. This is an explicit orchestration custody
proof, not a single Source score envelope containing model identity. The five held-out sentences contain 13 tokens and one
vocative: their results do not establish broad English quality. Original corpus
whitespace is unavailable in those retained annotations, so evaluation explicitly
reconstructs text by joining forms with single spaces.

The greeting proof supplies successive real text revisions to lexical preparation
and the existing checked revision-rebase contract. Period punctuation does not
grant source finality or commitment. Current-beam agreement may advance a stable
frontier; commitment requires the separate existing checked operation. An explicit
commit observation reports its returned beam and frontier and does not silently
modify the decoder's retained input state.

The numerical work bound remains 25 lookups by76 outputs (1900 lookup/add units),
62796 immutable model bytes and exact I64 arithmetic. The proof runner bounds
text to four available tokens, lexical alternatives to four per occurrence,
beam width to four and decoding to eight read/pop actions. It requests at most
128 model invocations per analysis; fixed queue, value and Sign storage must also
admit the graph. Each Source entry reserves 60000 additional local and 60000
remote Sign items, within the existing finite storage contract. The earlier
1024-remote-item run exhausted that reservation; this failure is retained in
the campaign evidence and is not a parse or quality result. Resource failure is
not attachment success. Hosted debug timing
includes transport and checked Source execution; preparation is reported
separately. Process RSS/HWM covers the whole proof runner, not model-only memory.

No live editor, speech carrier, prosody, phoneme timing, guest or physical-device
integration is supplied here. The frozen training artifact and its test failures
remain unchanged. #4907 remains open until its end-to-end conditions are met.

## Retained hosted result

`proof/parser_selection_hosted.json` retains the actual hosted debug run, not a
quality target. All four greeting analyses completed. Dependent/head indices
are zero-based; ROOT is the parser's separate root sentinel.

| Actual source revision | Selected relationships | Stable / committed | Model calls | Decode seconds |
| --- | --- | --- | --- | --- |
| `Hello ` | Hello → ROOT | 1 / 0 | 6 | 32.82 |
| `Hello, ` | comma → Hello (`punct`) | 1 / 0 | 14 | 108.08 |
| `Hello, Travis ` | Travis → Hello (`vocative`); Addressee evidence | 1 / 0 | 26 | 224.16 |
| `Hello, Travis.` | Travis remains vocative; comma revises to Travis; period → Hello | 1 / 1 after explicit commit observation | 34 | 359.65 |

The earlier revisions abstain from addressee evidence. No reference attachment
enters selection. At the last revision, survivors agree on the vocative but
still disagree on the earlier comma. The existing contiguous stable/commit
policy therefore stops at one token. The Addressee evidence is a candidate fact;
**the vocative is not committed**. The returned commit observation commits only
Hello and does not mutate the retained decoder beam. Source finality remains
explicitly partial throughout, including after the period.

Blind held-out UAS and base-relation LAS are both 10/13 (76.92%), including
punctuation. Vocative TP=1, FP=1, FN=0 gives 50% precision and 100% recall on just
one gold vocative. `Hi David :` incorrectly selects Hi as ROOT and David as
vocative; the corpus has David as ROOT and Hi as discourse. `Hi Sara ,` attaches
the comma to Sara instead of Hi. Both failures remain in the retained report
and executable regression assertions. These five covered examples do not
establish broad English accuracy. The accepted gold-state incremental ranking
result 14/16 (87.5%) measures a different condition and is not blind UAS/LAS.

The learned greeting analyses used 80 model invocations and the five held-out
analyses used 92, with 172 real committed numerical receipts. The zero-score
baseline used no model calls and produced a different complete graph. The
adversarial high-score left-arc probe refused with `IllegalAction` and unchanged
state. No selected expansion exceeded its admitted score bounds.

Preparation took 386.72 seconds; all tests took 2204.39 seconds. Held-out wall time
was 772.47 seconds, with individual decode times 73.41–208.41 seconds. Whole-process
peak RSS was 1093184 KiB and final RSS 1058512 KiB. These are hosted debug proof
costs, not an interactive performance claim or model-only memory measurement.
The earlier 1024-remote-Sign-item run failed with
`Scheduler(Sign(RemoteItemCapacityExceeded))`; increasing the explicit finite
reservation and replacing repeated scans with the parity-checked tournament
allowed the complete run. That earlier failure remains a resource failure,
not a grammatical result.

#5724 has the checked mechanisms needed to start a bounded slice that derives prosody from committed facts. This demonstration is **not ready for parser-driven committed
vocative prosody**: its vocative remains behind unresolved comma agreement.
That gap must stay explicit, alongside its false-positive vocative and latency.
Punctuation fallback must remain labeled heuristic. #4907 remains open.
