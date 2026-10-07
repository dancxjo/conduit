The explicit closing-Flow Source documents preserve the pinned floating topology while retaining immutable tensor ports as Value inputs. Every numerical invocation, prior state, next-state proposal and PCM proposal uses a distinct closing-Flow contract. Generic numerical owners repeat through those contracts; Value Backs remain one-shot. Source still owns layer order, period offsets, gates and state projections.

`fargan_flow_topology` checks conditioning, signal and complete subframe expansion, exact finite transport bounds, immutable tensor port temporality, and transposed/foreign matrix rejection. This is checked Source topology evidence, not ordinary execution of the complete recurrent Flow cycle.

The authored epoch contracts carry an exact bounded model/session anchor and U64 epoch, consume four 80-value conditioning slices, and retain four 40-sample PCM proposals. Consumed conditioning is discarded after each phase. Their maximum canonical transport envelopes are:

| Source Type | Maximum bytes |
| --- | ---: |
| FarganFloatEpochInput | 14255 |
| FarganFloatPhase1 | 14138 |
| FarganFloatPhase2 | 13778 |
| FarganFloatPhase3 | 13417 |
| FarganFloatEpochProposal | 12672 |
| FarganPcm16EpochResult | 12246 |

The frame anchor is an exact link to session-owned admitted model, graph, precision and immutable intent receipts. It cannot independently admit them. Each complete record fits the selected 16 KiB frame envelope. This is narrower than the core structured-value maximum; the test asserts 16,384 bytes explicitly.

A homogeneous anchorless phase carry uses one 240-element workspace: remaining conditioning followed by provisional PCM, with explicit Source zero padding. The four meaningful layouts are 240+0, 160+40, 80+80 and 0+120 elements. Its exact canonical payload bound is 2,846 bytes; pairing it with the 9,962-byte subframe result requires 15,468 bytes including retained schema metadata. The selected immutable model anchor must be checked at epoch entry and authored into results through a typed Source startup parameter. Checked Source projections establish all four workspace layouts, with reference/prepared value parity. The selected-anchor Source matcher rejects a mutation at every one of its 96 receipt bytes and rejects a foreign precision; typed startup substitution is checked. This does not yet establish the complete scheduler cycle.

The final externally accepted epoch must contain canonical I16 PCM, next recurrent state, next period, epoch and model anchor. Provisional float subframes are computation material. Feature641 and conditioning128 proposals must remain held until the final PCM16 record is committed; pressure or cancellation before that publication must leave their externally committed feedback unchanged. The current contracts do not yet prove this complete scheduler cycle, startup initialization, resource/heap footprint on a booted target, same-intent WAV, real-time performance or intelligibility.

The final carry contains next state, next period and epoch, with no float PCM or anchor. Its 9,545-byte bound pairs with canonical I16Vector160 (1,236 bytes) in 12,877 bytes. Source owns period Q8 rounding and clamping before checked narrowing and native period admission. Source also owns normalized float clipping and the 32,768 PCM gain, followed by explicit finite-vector admission and the generic nearest-ties-away I16 converter. Edge-value reference/prepared tests cover these policies. These are Source policy and envelope proofs; whole-record native readmission after typed pairing and final scheduler commit remain required.
