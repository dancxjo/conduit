# Bounded authored acoustic gesture component

This additive component composes with the existing contextual allophone choice;
it owns neither another utterance IR nor a phonological inventory. The exact
original occurrence event, membership, selected full phone definition, choice
state and timing frames remain inspectable. Each gesture also carries the exact
occurrence and original Language source references. The separate
`PreparedContextualPhoneGestures` bridge borrows the original opaque
`IntentAllophoneChoice`, including its complete original inventory, phoneme,
candidates, context decisions, rule status and revisioned intent. It looks up a
unique selected definition in that same inventory. A raw declared selected state
is not proof of contextual selection.

The bridge does **not** establish custody correspondence with a separate
`PreparedIpaInventory` or another shared common-IR carrier. That later composition
must bind the exact retained inventory and event to the joined original material,
not merely compare IDs. Language and variety profiles remain authored data; this
component's demonstration is not a universal or complete English phonology.

## Named numerical profile

`speech/authored-acoustic-gesture-demo/1` is a deliberately authored formant
approximation. Source recognizes complete proper IPA strings `i`, `a`, `u`, `t`,
`d`, `tʰ`, and `t͡ʃ`; it never splits phones by codepoint or uses renderer codes as
public identity. Other symbols, unsupported combining marks, embedded stress and
length marks refuse with `UnsupportedSymbol`. Stress remains a separate original
six-state specification; no unresolved state becomes zero or a neutral stress.
Length-mark realization remains a required broader-profile capability.

Original Audio elapsed-second fractions retain their full U64 domains. The
separate `SpeechGestureU32Timing` numerical eligibility requires original common
denominators and U32 numerators/denominator, a nonempty nominal interval, a
committed frontier at or before its start, and lookahead covering its end. It
refuses reducible foreign bases rather than silently reducing or resetting them.
These anchors are abstract elapsed-second origins, not Host clocks or clock
relations. No played frontier is revised.

Source computes exact fifths: closure occupies the first three fifths, release
the fourth, and declared aspiration/frication the final fifth. All products and
sums are bounded by five times an admitted U32 endpoint (<2^35); independent
u128 tests cover extreme endpoints. Voicing for `d` begins at closure end;
vowels voice throughout. Source selects the roles and exact windows. Closure
has known relative gate gain 0; active excitation roles have known gain 1. These
are explicit profile controls, not unknown states replaced by zero and not
absolute physical power or SPL. Centers and independent bandwidths are exact
positive Hz values authored in Source. They are acoustic hypotheses, not measured
anatomy or biomechanical tract simulation.

Source raw phase/window/formant numeric results are retained in execution frames.
Adapters only copy exact fields into canonical Audio fractions/quantities and
Native-admit the final gesture. Receipts retain exact fixed Source programs, all
admitted inputs, all raw results, final canonical gesture frames, and full original
frames. The expression evaluator itself is not a refinement validator.

`gesture_to_audio_trajectory` explicitly copies an admitted step gesture into the
existing Audio quantity trajectory carrier, retaining the whole original gesture
and canonical target frame. It performs no arithmetic, normalization, private
identity cast, or interpolation. The separate Audio U8 arithmetic evaluator can
honestly refuse these higher-frequency carriers; this copy is not proof of its
numeric execution profile.

## Authored overlap and boundaries

`prepare_authored_acoustic_gesture` Native-admits Source-owned exact interval,
anchor and common-basis laws. Authored gestures may begin before or extend after
the nominal interval, within the committed frontier and lookahead. Its comparison
only profile admits full U64 values without multiplication. Original gesture and
original timing frames plus admitted eligibility are retained.

The named conflict policy permits independent channels to overlap and refuses
same-channel, same-formant-index overlaps. Source executes that conflict decision;
its receipt keeps both originals and full executed frames. It does not invent a
blend, claim-resolution authority or learned/manual override. Callers must check
all bounded pairs in their containing prepared owner. No punctuation is interpreted
as pause duration.

## Proof boundary

Standalone tests include actual original Speech/Language/Audio Types and fixed
Source programs. They check exact Unicode identity, six original stress states,
independent u128 timings and extremes, native corruption/domain refusals, authored
anticipation/extension, independent overlap/conflict, exact program replay, and
canonical Audio trajectory field-copy admission. Representative canonical Type/
value sizes: gesture 6565/7429 B; timing 1185/1620 B; original occurrence event
17796/18534 B. No limits were raised; these are ordinary allocating preparations,
not Flow execution receipts.

The separate actual-choice integration test is supplied for the Speech build
owner and is **not** counted in standalone acceptance until that gate runs. It
checks stressed-onset `t`→`tʰ`, unstressed `t`→`t`, unchanged original phoneme and
intent, and missing-context refusal through the existing opaque choice.

Renderer projection is explicitly unsupported here. Existing compact DSP has
voicing/noise and closure/release controls, but its fixed 96-frame release is not
this arbitrary rational fifth profile, and exact bandwidth-to-coefficient
projection is not supplied. Nasal/lateral/rhotic effects, a general coarticulatory
or articulatory simulation, stress/length symbol composition, actual sample-rate
projection/clock relation, PCM, attended listening, multiword acceptance and
common prepared-IR custody composition remain separate required work. No #5218
closure, complete linguistic profile, renderer effect or natural-speech claim is
made by these traces.
