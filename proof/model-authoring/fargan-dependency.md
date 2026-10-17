# FARGAN dependency inspection

Inspected repository documents at reviewed revision
9477021ecc6cc15a2bf7f5e4be7b55c6d27faac5, without modifying or absorbing PR #5183.
This is source inspection, not an executable or numerical compatibility proof.

- `proof/fargan/feature-profile/README.md` identifies
  `speech/fargan-formant-spectral-approximation@1` as an approximation over
  formant PCM, including an 8 kHz to 16 kHz resampling limitation and omission
  of the upstream encoder's richer pitch analysis. It cannot stand in for
  the required natural-speech training targets.
- Its upstream analysis pin is xiph/opus
  503d81b138d76621aae4b12786e90de48aa8db3a (`dnn/freq.c`, `dnn/lpcnet_enc.c`).
- `proof/fargan/README.md` distinguishes conditioning/compact matrix numerical
  agreement from complete signal/recurrent state, PCM, and physical listening.
  It records unresolved explicit redistribution permission for the model archive.
- At inspection on 2026-10-08 UTC, PR #5183 remains open, current head
  adb71e234dbe42e427886364a46c175ea51e027a. The reviewed source above and this
  newer head are distinct revisions; the source inspection does not establish
  the newer head's runtime behavior.

The feature PR must remain draft until the selected FARGAN contracts land in dev,
Rust natural extraction agrees with the pinned upstream reference, and recorded
speech reconstructs through the existing authored FARGAN composition. No acoustic
training or voice-naturalness claim follows from the Burn regression proof.
