# Finite media-frame mixing

`mixer.conduit` owns the arithmetic and refusal policy for two or four signed
integer excitation channels. It uses the existing generic pure-expression Back;
there is no device effect, scheduler, pronunciation policy, or resampler.

Each `MixerChannel` carries a sample, dimensionless Q8 gain, media timeline
identity, frame ordinal, and integer rate in Hz. The admitted profile is sample
−32768..32767, gain 0..4096, nonnegative timeline, frame 0..65535 and rate
1..192000. Every channel must agree exactly on timeline, frame and rate.
Invalid controls emit `invalid_profile`; unequal timebases emit
`timebase_mismatch`. Neither outcome contains a sample.

Each weighted channel is computed as `sample * gain_q8 / 256`, with integer
truncation toward zero. Weighted channels are summed before one symmetric
saturation to −32767..32767. Four admitted weighted channels fit signed I32
arithmetic, including every intermediate sum. Opposite-sign channels retain
headroom; individual channels are not clipped before summing.

Each external channel Fore explicitly admits at most 512 canonical bytes,
matching the selected pairing Back. Internal Cords use their exact admitted
output value bounds rather than a larger generic queue request.

Two actual input Fores feed one typed pair stage. Four input Fores feed two
channel-pair stages and one pair-of-pairs stage. Each stage keeps at most one
pending value per side. Pairing is by ordinal arrival; Source then checks the
explicit media coordinates. This component neither infers alignment from
arrival time nor silently changes a channel's timebase.

These are closing, payload-only Flows. Pair stages use the existing finite zip
normal-close policy: publish an already-formable pair, discard an unmatched
value, then close. Missing input therefore produces no invented zero or sample.
Abnormal and cancellation inputs are not admitted by this finite profile.

Prepare the declared `MixerChannel` with `prepare_source_types`; form the exact
checked `MixerPair` and `MixerQuad` tuple schemas, and retain them in the
StartupCatalog. `flow_zip_typed_offer` retains bounded canonical schemas and
schema-derived Kind identities before Play. Rechecking the generated Source
requires these same exact catalogues. The installed Back revalidates its Kind,
ports, schema configuration and immutable contract before preparing buffers.
Source graph export retains these semantic identities independently of layout.

`architecture/plot/tests/bounded_mixer.rs` exercises independent wide-integer
reference results, extreme profiles and exact timebase refusals.
`targets/std/tests/bounded_mixer.rs` uses actual authored multi-port graphs,
ordinary expansion and Plan construction, installed Kernel HostCalls and
fixed value-allocation capacity, including asymmetric arrival and missing input.
These are hosted component checks, not physical audio or parent acceptance.
