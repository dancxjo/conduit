# Binary64 tininess at the normal boundary (development)

Core's finite binary64 projection previously set `tiny_inexact` only when the
final encoding had a zero exponent. That misses after-rounding tininess at the
smallest normal value. Precision rounding with an unbounded exponent range can
remain tiny even when rounding into the destination's finite exponent range
produces the smallest normal encoding. The
[SoftFloat FAQ](https://www.jhauser.us/arithmetic/SoftFloat-3/doc/SoftFloat-FAQ.html)
explains this distinction.

The regression uses the exact product of binary64 minimum-normal and the
predecessor of one. Nearest-even produces minimum-normal with both inexact and
underflow status. The previous implementation failed this case. Tests also
cover both signs, all four admitted rounding directions, and distances
`2^-1075`, `2^-1076` and `2^-1077` below minimum-normal. Some produce the same
normal bits with different tininess status; changing the rule to classify all
minimum-normal results as tiny would also be wrong.

A separately built SoftFloat oracle checks eight multiplication cases and 24
exact binary128-to-binary64 boundary conversions, with after-rounding tininess
selected explicitly. Its source is pinned to
[`a0c6494cdc11865811dec815d5c0049fba9d82a8`](https://github.com/ucb-bar/berkeley-softfloat-3/tree/a0c6494cdc11865811dec815d5c0049fba9d82a8).
The retained oracle program and log describe the exact source encodings and
rounding modes. SoftFloat is a development oracle; no SoftFloat code or new
dependency is linked into Conduit.

The fix uses exact rational threshold comparisons only at the minimum-normal
boundary. It preserves existing target bits and error claims. Additional work
spends the existing admitted limb-work meter and may refuse when exhausted.
The std numerical artifact identity includes the new helper source. Earlier
source-pinned evidence remains historical; this is not a claim that existing
published certificates already used the corrected status.

This repairs a status fact in the finite projection profile. It does not supply
general IEEE nonfinite operations, exception traps or a runtime consuming
projection Back. The original #5392 acceptance gates remain open.
