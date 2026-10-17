# Checked DSP Source package

`conduit_semantic_catalog::prepare_dsp_source_package()` prepares the owned `audio/dsp` package with the
ordinary checked-package and export-catalog owners. `pack.conduit` declares the
public entries. The returned bundle retains exact content/member identities;
its checked document retains actual component bodies and typed Fores, and its
export catalog resolves the declared package-qualified names.

The current package groups the oscillator and resonator's existing complete
declarations as coherent modules. Their equations remain in their original
Source files. The mixer uses the existing structural pair/quad profiles derived
from its owned MixerChannel Type. Preparation retains that startup catalog
alongside the checked document. It supplies no new numerical implementation.

A caller resolves an export such as `audio/dsp/oscillator-prepare`, then uses
ordinary `expand_canonical_plot_for_authoring` against the returned checked
document. Host Back selection, buffers, reservations, Plans and Play remain
with their existing owners. Source-package preparation may allocate and does
not run audio or acquire a device. Individual component profiles continue to
own their coefficient domains, state, bounds, precision and refusal behavior.

Run `cargo xtask check dsp-shared-compositions` for the package and installed
component corpus. These are development/software proofs. The package seam
alone does not establish a Patchbay import UI, physical playback, independent
voiced/aspiration/frication profiles, or completion of #5217.

`noise-gain-example.conduit` is an authored package member with explicit local
imports. It preserves the full noise outcome on its diagnostic output and
selects advanced samples into the existing gain component. Its checked
expansion retains the exact standalone noise and gain programs. Downstream
imports of five public control Types retain their owners' exact typed Fores.
The installed proof executes the same imported graph through an ordinary
sealed Std Plan and the production expression/selector Backs. Six input cases
retain all noise outcomes and produce PCM only for the three advances, against
an independent wide-integer reference. The observed kernel value-allocation
capacity remains stable. This is hosted software execution.

The catalog owns package preparation and generic Flow contracts. Audio owns
the component Source and numerical profiles. This keeps the dependency
direction intact when composition uses the existing typed Flow pairing.

The package also declares four per-frame Value profiles: noise-step-value,
envelope-value, gain-value and mix-two-value. Preparation uses the ordinary
Source AST to retain each owner's exact pure expression and Type spellings.
Generated profile text records the original Source identity. Unsupported
Fores, additional port contracts and non-expression bodies refuse this
projection. The original closing-Flow interfaces remain available.

Tests compare complete canonical programs for every Flow/Value pair. A Value
profile performs one explicit frame operation; it does not silently adapt a
stream or own a whole-Flow lifetime quota. The package/compiler checks run
before the separately named installed-execution test and full DSP regression.

## Experimental full-frame compositions

`audio/dsp/experimental-q14-frame` checks and expands the ordinary authored
noise or periodic diagnostic excitation → Q14 recurrence → envelope → gain
chain. `audio/dsp/experimental-q14-two-source` invokes that same chain twice,
with separate caller-carried controls, then invokes the unchanged two-channel
mixer. Both inputs have gain Q8=128, a declared relative digital headroom
policy. The mixer retains its frame/rate mismatch and saturation outcomes.
These are new explicit coefficient-domain profiles, not a waveform change to
the existing compact voice or the earlier accepted shared-excitation specimen.

The single-frame cursor carries seed, phase, filter history and finite remaining
work. The frame commits that cursor only after its noise, envelope and gain
results are admitted; a refused frame retains its original cursor. Filter output
clamps to ±32767 before envelope/gain. The experimental recurrence admits
|b|≤8192 and |c|≤4096 in Q14, with histories in ±32767. Its absolute feedback
sum is at most 3/4, and intermediate ordered sums fit I32. The caller may
replace these experimental coefficients before each frame; every frame admits
the same bounds, with no implicit interpolation or hidden retuning state. The
uniform absolute feedback bound applies across abrupt changes, and the explicit
clamp keeps committed histories within ±32767. Independent integer-reference
and fresh-checkpoint tests exercise alternating coefficient-domain corners.
This is not an acoustic frequency/bandwidth projection. The separate acoustic
resonator retains its fixed-coefficient preparation/reset policy. The cubic periodic source remains a limited
diagnostic profile; this composition supplies no new antialiasing claim.

The two-source result includes both controls only when both frames are valid
and the mixer returns a sample. Otherwise it returns `rejected`, without an
implicit continuation, hidden retry or advancing one branch for the caller.
Authored Value pairing preserves one result per input; a dropped branch cannot
be accidentally paired with a later frame. Preparation installs the existing
finite typed `data/pair` contract and verifies that its Stage Type matches the
Type in the checked package exactly.

The package ships both input Types and explicit seed initializers. Current
evidence covers Source checking/expansion, unchanged component program reuse
and production local-kernel execution. The latter checks independent integer
recurrence, serialized partition resume, held outputs/cancellation, independent
two-source histories and declared mixing headroom, with zero allocation during
prepared child advancement. Independent periodic phase/envelope and invalid-coefficient cursor retention
checks also pass. Public catalog and product authoring integration are implemented in this
development slice and covered by focused Source tests. The complete parent
acceptance audit remains before claiming the full #5217 delivery. No
device or physical audio is involved.

## Independent excitation experiment

`experimental-independent-excitation` has separate voiced, aspiration and
frication controls and returns all three committed controls with the mixed
sample, or one `rejected` result. Its role check requires periodic voiced
excitation and two independent noise cursors. These are relative digital
signals with explicitly chosen coefficient profiles; the names do not assert
an aerodynamic airflow model or measured phonetic quality.

All three paths use the same authored frame. Two unchanged mixer invocations
apply half gain at each sum: voiced contributes one half, while aspiration and
frication each contribute one quarter, with the mixer's existing per-stage
integer truncation and saturation. Frame mismatch, an invalid role or any
invalid child prevents the combined result from exposing partial committed
controls. A caller retains its original inputs after rejection.

Explicit context-carrying Value profiles for noise, envelope, gain and mixing
return caller context with the numeric outcome. Preparation uses ordinary AST
Input spans to rebind the owned expression to `.control`; the mixer explicitly
names its two original tuple inputs `control.left` and `control.right`. No
arithmetic token or branch is rewritten. The original Flow and Value entries
remain available. Source custody tests restore only those declared input
projections and verify exact equality with the owned numeric expression.

The three-source graph has 61 gears and two boundary nodes, within the existing
64-node composite profile, and two pair instances, within the existing eight-
instance offer. The direct Control-entry frame avoids redundant Stage wrappers.
No default Host limit, port bound or retry policy was raised. The compact
carried topology passed the supported DSP suite (34 tests across 14 harnesses),
including production local-kernel independence and five rejection classes, and
strict Clippy. Prepared child advancement remains allocation-free in these
fixtures; no whole-Host or physical-audio claim follows.

## Ordinary public caller

The product catalog registers closed `audio/dsp/...` component interfaces and
shipped Types. Selected Source Backs retain their checked owner definitions,
Native construction metadata and private primitive contracts. Caller helpers
cannot capture those bodies; the private mixer aliases and pairing contracts
are not installed as ambient caller definitions. No Host offer is invented by
registering a Source interface.

Check the ordinary public gain caller without executing effects:

```sh
conduit check semantics/audio/dsp/public-gain.conduit --json
```

The same product Source loader expands this caller for authoring. Ordinary
expansion selects DSP Source Backs; the existing Morse recursive-Back selection
remains opt-in. Public gain, complementary one-pole and composed control-frame
callers also have catalog-level checking/expansion regressions. These compiler
and product boundaries are included in `cargo xtask check dsp-shared-compositions`
alongside the independent numerical and Kernel-fixture proofs. They do not
establish a Patchbay palette, default physical audio provider or device playback.
