# Tongues paired-latent capstone

This bounded experiment asks whether one continuous latent process can explain paired sound and tongue motion without receiving phone boundaries or labels during representation training. It is an architecture and reproducibility proof, not a competitive speech model.

Run the checked-in experiment through the repository entrance:

```console
cargo xtask --json demo tongues-research
```

The command validates the exact derived corpus identity, checks the portable training and bidirectional-inference Forms, trains the deterministic paired model, freezes an exact checkpoint, evaluates held-out examples, and only then exposes segment labels to a small diagnostic probe. The JSON report records corpus, preprocessing, objective, seed, work, checkpoint, callable-signature, held-out, uncertainty, and negative-result evidence.

## Data and lineage

The source is PB2007, DOI `10.5281/zenodo.6390598`. The admitted archive is exactly 37,793,957 bytes with SHA-256 `123d3fc2f114ab37724c7f05e00a03ff21d7e815f7f957987e8255f56d73f243`. The deposit README says CC-BY-SA while Zenodo metadata says CC-BY-4.0; this experiment records both and applies the more restrictive README statement.

The checked-in 58 KiB derived slice contains twelve paired utterances: eight train, two validation, and two test. It preserves the 16 kHz audio and 100 Hz EMA source clocks, six tongue coordinates in source centimetre microunits, missing-data masks, per-resource digests, and post-freeze probe labels. Reproduce it from a locally obtained exact archive with:

```console
python3 semantics/tongues/tools/prepare_pb2007.py PB2007.zip /tmp/pb2007-derived-slice.json
cmp /tmp/pb2007-derived-slice.json semantics/tongues/data/pb2007-derived-slice.json
```

The large source archive is deliberately not committed and PR CI does not download it. CI uses the digest-bound derived artifact and runs in milliseconds.

## Model and honest stop line

The first model is a paired linear shared-latent realization: two encoders, two decoders, a two-dimensional continuous latent state, a learned recurrent transition, and a diagonal residual approximation for uncertain acoustic-to-articulatory inversion. Same-modality, cross-modal, latent-agreement, and dynamics objectives are measured. Both inference directions use one exact checkpoint through a finite `ModelSignature`.

The current evidence does not establish cross-speaker generalization, speaker conditioning, phone-class recovery, calibrated posterior coverage, interpretable articulatory factors, waveform-quality generation, or state-of-the-art performance. PB2007 contributes one speaker to this slice, so a speaker-conditioned versus speaker-agnostic comparison would be non-identifiable. The acoustic front end is four deterministic summary features, not an SSL encoder. These are explicit failures or deferred hypotheses, not claims hidden behind the successful pipeline.

The real Patchbay learned-Watch proof consumes the exact run and corpus values: observed acoustic and EMA trajectories, inferred latent state, articulation uncertainty and alternatives, objective values, recurrent dynamics, and checkpoint transition. It uses Patchbay's authoritative debugger/Watch model and DOM/SVG renderer rather than a separate visualization.

## Post-freeze dynamics analysis

Run the finite analysis layer over that same exact frozen checkpoint and corpus derivation:

```console
cargo xtask --json demo tongues-analysis
```

The generated identity-bound report includes descriptive relative phase and lag over a declared -3..3-bin window, pairing-reversed and alternate-seed controls, label-free turning-point events followed by annotation comparison, three-cluster induction fitted without labels, the frozen lightweight probe, and a thresholded polynomial sparse-dynamics fit evaluated on held-out utterances beside a constant-state baseline. The dedicated `.conduit` Form keeps continuous extraction and dynamics analysis upstream of the post-freeze overlay.

These are deliberately modest empirical results. Reversed-pair phase locking is not lower in this slice, so it does not support a stable coupling narrative. Some learned turning points align with post-hoc boundaries and others systematically miss them. The sparse predictor improves on its stated baseline but is an association, not a causal law. A C-center comparison, cross-speaker stability, rate/context effects, alternative front ends, and claims that the learned coordinates are oscillators remain non-identifiable. Patchbay renders the actual report values—relative phase, events, cluster assignments, later labels, and observed versus sparse-predicted deltas—through the existing authoritative Watch renderer.
