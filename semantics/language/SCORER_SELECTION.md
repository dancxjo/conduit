# Finite checked parser action selection

The optional `parser-model` feature admits the exact English artifact accepted
under #5722. `prepare_english_parser_scorer` returns the existing generic
`PreparedCategoricalStep`; it does not parse text. Absence, foreign content,
signature, descriptor, residence and resource bounds refuse explicitly. The
admitted resource reference is compared in full, including custody metadata
that the generic descriptor digest intentionally excludes.

Checked Language Source retains the linguistic policy:

- `parser_scorer_features.conduit` constructs the artifact's exact25 categorical
  indices from current admitted lexical alternatives and parser state. A future
  token contributes available alternative membership, never a chosen future POS.
- `parser_scorer_actions.conduit` maps76 class identities to ordinary parser
  requests. Numerical output is76 I64 scores, never a tree or linguistic fact.
- `parser_scorer_ranking.conduit` defines the legal mask and deterministic
  fallback. `parser_scorer_tournament.conduit` ranks all76 masked classes using
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
proof, not a single Source score envelope containing model identity. The five held-out sentences contain13 tokens and one
vocative: their results do not establish broad English quality. Original corpus
whitespace is unavailable in those retained annotations, so evaluation explicitly
reconstructs text by joining forms with single spaces.

The greeting proof supplies successive real text revisions to lexical preparation
and the existing checked revision-rebase contract. Period punctuation does not
grant source finality or commitment. Current-beam agreement may advance a stable
frontier; commitment requires the separate existing checked operation. An explicit
commit observation reports its returned beam and frontier and does not silently
modify the decoder's retained input state.

The numerical work bound remains25 lookups by76 outputs (1900 lookup/add units),
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
