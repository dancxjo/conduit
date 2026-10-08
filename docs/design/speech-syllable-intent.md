# Explicit target syllable intent

This is a typed component of the existing `SpeechUtteranceIntent`, following the
pinned Tongues `Syllable` meaning in the common-intent mapping. It does not infer
a grouping from IPA or introduce another utterance architecture. The complete
original intent and `SpeechPhoneSequence` remain borrowed by
`PreparedSyllableIntent`; checked Native receipts retain their original supplied
material as well. An ID alone does not resolve a phone or establish commitment.

| Field | Meaning |
| --- | --- |
| `identity` | Explicit local syllable identity; no global registry claim |
| `basis` | Exact utterance, revision, sequence, inventory and language of the target phones |
| `phones` | One to32 ordered references to actual occurrences in the supplied phone sequence |
| `phone_positions` | One onset/nucleus/coda position per supplied phone occurrence |
| `stress` | All six existing `StressSpecification` states, unchanged |
| `span` | Optional original seconds or frame span; absence does not invent timing |
| `nucleus_index` | Optional index naming a supplied phone whose position is nucleus |
| `provenance` | Original supplied evidence source/method/version |

Source laws own position coverage, nucleus range/position, exact intent/sequence
basis, member resolution and strict within-syllable order. Preparation traverses
the bounded members and invokes those Native contracts. It also reuses the
existing seconds-span ordering check because current pure Source comparison
does not cover F64. Equal endpoints and negative relative offsets remain valid;
frames keep their original explicit timebase/rate. No sample clock or alignment
to individual phone spans is inferred.

Strict ordering rejects repeated/reversed members within one syllable. It does
not impose contiguity, force syllables to partition the sequence, or prevent an
explicit occurrence from participating in more than one group. Those aggregate
relations require an explicit supported linguistic profile. The32-member bound
is this compact component's admission profile, not a bound on human language.

The largest component static Native Type is24038 bytes, below the unchanged
65536-byte Type ceiling. Components are separately admitted ordinary typed
material retained by the prepared owner; they are not embedded repeatedly into
`SpeechPlaybackBasis`. This does not yet establish a complete shared aggregate,
whole-preparation memory quota, Flow lifecycle, phone realization, trajectory,
commitment or playback authority.

Four staged canonical-owner tests and the actual workspace syllable target pass
for full original custody, all six stress
states, foreign basis/missing member/reverse/duplicate refusals, nucleus/position
coverage and ordered seconds spans. The same actual workspace gate passes all eight existing IPA tests, for12 in
total, with all17 selected source hashes unchanged. Scoped lint remains under
test. These are local conformance results, not product acceptance.
