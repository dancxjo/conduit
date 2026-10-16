# Shared speech and musical compositions

Run the checked Source/Plan/Play proofs from a checkout:

```sh
cargo xtask check dsp-shared-compositions
```

This development suite executes a finite sibilant-frication specimen and a
small periodic musical specimen. It also checks the original compact voice's
phone histories against its frozen ordered-I32 scalar reference. These are
software execution and numerical proofs, with no listening or physical-output
claim. They do not establish a trained voice, a general IPA synthesizer, or the
common linguistic IR required by the larger speech journey.

The actual Source is under `targets/std/tests/fixtures/`: the shared frame and
closing scan, plus `shared-speech-frication-profile.conduit`. The test preparation
modules retain Source, checked plot, expanded plot and sealed Plan identities;
the production kernel executes existing expression, pair and scan factories.
Rust prepares finite fixtures, supplies/collects typed values, and computes
independent numerical references. It does not implement the DSP used by Play.

The speech specimen receives the existing admitted `SpeechPhoneNotation` from
the qualified Unicode IPA constructor. An ordinary Source policy selects one
explicit coefficient-domain frication profile for `s` and another for `ʃ`;
`a` returns the `rejected` variant before DSP execution. This is a limited
experimental sibilant profile, not a claim of measured phonetic fidelity.
The profile's actual committed output supplies the shared frame and subsequent
scan Plans. Input digests and each Source/Plan/Play identity are retained in the
proof output. No ASCII voice code substitutes for the public IPA input.

The musical specimen selects the owned cubic periodic phase/flow plots. Its
period is 4096 Q8 sample units (16 samples); interpreted at the specimen's
8 kHz rate, this is 500 Hz. No clock projection or band-limited quality is
asserted. The noise seed still advances explicitly, independently of which
excitation the authored policy selects. Both specimens use the same authored
noise, periodic phase/flow, envelope, gain and Q14 resonator compositions.

The noise/envelope/gain owners retain their original closing-Flow interfaces.
This fixture derives explicit Value profiles from those exact pure Source
bodies. Tests compare canonical expression-program bytes, so changing temporal
profile does not replace the arithmetic. The periodic and resonator templates
are included unchanged from the audio owner; the compact voice includes these
same owners. No second DSP engine or relaxed Source checker is introduced.

Each specimen has 160 frames, an eight-frame linear attack, and a sixteen-frame
release. The envelope is Q8. Resonator coefficients are fixed for a run, bounded
by |b| <= 8192 and |c| <= 4096 in Q14; histories and output are explicitly clamped
to -32767..32767. These bounds keep the ordered I32 recurrence within range.
Invalid coefficients or phase state are gated before arithmetic and preserve
the cursor. Coefficient modulation is not offered. Seed, remaining work, frame,
phase, coefficients and both filter histories remain explicit state.

A scan admits at most 64 values per Play. Full specimens resume from the actual
committed accumulator across finite preparation windows; no global limit is
increased. Tests vary both preparation boundaries and producer block sizes,
exercise `Full` pressure without treating it as acceptance, hold outputs before
commit, close before final delivery, drain, and check normal terminal Signs.
Each named fixture reserves its work, queues and Sign capacity before Play.
Allocation probes observe no allocations or reallocations during those runs.
Independent wide-integer excitation/envelope references and checked ordered-I32
filter references validate every frame, including attack and release.
