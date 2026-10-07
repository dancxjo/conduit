# Bounded actual native graph continuation

`learned_vocative_playback.rs` consumes a separately produced actual learned
parser receipt file. Its default tests use supplied graphs only to verify loader
refusals and checked ASR event reconstruction; those tests do not establish
learned parser accuracy. The two continuation tests are explicitly ignored until
an actual receipt path is supplied. No supplied-graph fallback exists.

The JSON array has exactly three reviewed teaching rows, one per initial,
medial and final vocative position. Each row retains whole native canonical
`source_material_bytes`, `lexical_tape_bytes`, `basis`, and each predicted
`canonical_arc_bytes`, plus four lexical choice indices. Full native Source
admission, exact source/analysis identity, lexical reconstruction, choice bounds,
arc occurrence coverage and finite connected single-root checks are required.
The checked Source pronunciation choice must match the learned lexical index.
The reviewed phone profile explicitly supplies surface-lemma `Hello` data.

Run with `CONDUIT_LEARNED_GRAPH_RECEIPTS=/absolute/actual-graphs.json` and:

```
cargo +stable test -p conduit-speech --features kernel,semantic-bindings \
  --test learned_vocative_playback \
  actual_learned_native_graphs_feed_three_position_playback -- --ignored
```

This uses the existing ordinary Plan producer and scheduler pressure proof,
including queued versus played acknowledgements and cancellation. Played
acknowledgement is supplied explicitly in the fixture; it is not physical
listening evidence. Teaching graphs are not held-out accuracy evidence. This
route renders formant PCM; it does not establish FARGAN neural waveform output.
Retain the actual decoder's artifact/manifest/model identity and metrics alongside
its complete native graph file before claiming learned acquisition.

A separate ASR-origin run is required to prove event-to-graph lineage. Invoke
`export_asr_origin_tapes_for_actual_parser -- --ignored` with the same input
variable and `CONDUIT_ASR_GRAPH_SOURCES=/absolute/new-asr-tapes.json`. It prepares
partial, revised and committed envelopes through `prepare_asr_revision`, derives
three exact Language revisions and lexical tapes, and exports final native tapes
with complete envelope/revision receipts. The parser must actually decode those
new tapes and recompute analysis/artifact policy bases. Existing initial-final
graph metadata cannot be retrofitted into this chain.

Fresh ASR graph rows must retain `asr_envelope_bytes` and
`language_revision_bytes`. The loader replays the checked bridge with explicit
zero playback commitment, requires full equality of each derived revision and
then the final native tape. Optional non-ASR `source_revision_history_bytes`
also supports canonical lexical reconstruction with exact previous occurrence
correspondence. Bounds are four words, three ASR events, at most eight generic
history revisions and 262144 bytes per native receipt leaf; the reviewed playback
profile remains at most 32 phonetic occurrences.
