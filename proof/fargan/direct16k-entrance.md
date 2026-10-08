# Direct 160-sample / 16 kHz entrance

This separate entrance consumes exact contiguous 160-sample PCM16 epochs. It does
not reinterpret or decimate the legacy 80-sample / 8 kHz entrance. Generic
integer-to-float conversion is followed by Source-authored multiplication by
2^-15. The original causal preemphasis, 640-sample history, 320-sample analysis
window, spectral analysis resources, 20-feature interface, conditioning history,
trained tensor resources, model signature and three feedback/ACK domains remain
owned by their existing contracts.

`FarganFeaturePcmEpoch16k` and `FarganFeatureInputEpoch16k` are numeric carrier
Types, not common-IPA, clock, measurement or utterance authority. Their new Native
profile identities use the extended declaration Source. Every reused legacy
profile is prepared against its exact original declaration Source, preserving
its existing identity. Canonical events undergo original Source Native recursive
contract/invariant admission before execution.

The preparation profile accepts 2..65535 exact epochs. It refuses partial epochs,
padding, empty original custody material and absence of the explicit accepted
analysis/resynthesis loss policy. The retained tape is opaque and retains all
original custody bytes and original PCM16 samples. It does **not** independently
prove the caller's common-IPA custody. The actual joined common owner must admit
and supply that material. This interface currently supports only the original
fixed period interface; foreign or varying timing requires explicit admitted
period controls, never guessed period values.

The direct finite Source composition binds the selected count into the original
continuation and alignment policies: first feature and five warm-up repetitions,
remaining N-1 native events, two Source continuation events, N+1 model PCM rows,
last 80 samples of the first row, complete middle rows and first 80 samples of the
last row. The endpoint is N*160 samples. The legacy63 wrapper remains fixed at63.
No model quality, clock mapping, listening, target boot or real-time guarantee is
implied.

The full retained model artifact/signature/resource access and shared blob remain
in the existing admitted session/resource owners; they are not copied into an
unbounded live scheduler cell. Existing 16 KiB fixed-cell transport is unchanged.
A trained two-epoch numeric preflight is separate from a committed greeting run.
Its output cannot be advertised as common-carrier or multiword acceptance.

Component gates use the actual cached AI/Plot/Core dependencies and four actual
ConduitOS operation-owner modules. The legacy Speech SDK fixture preparation is
excluded from that component harness; production package validation and an actual
terminal committed common-carrier session remain required. No Source/Plan/kernel
arithmetic is replaced by a C implementation or an oracle.
