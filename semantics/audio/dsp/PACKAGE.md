# Checked DSP Source package

`prepare_dsp_source_package()` prepares the owned `audio/dsp` package with the
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
