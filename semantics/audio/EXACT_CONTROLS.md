# Exact authored Audio controls

`prepare_exact_audio_frequency` and `prepare_exact_audio_time` prepare checked
physical quantities for Audio's existing rational control Types. Core owns unit
conversion and exact rational arithmetic. The caller supplies the work budget;
Audio does not enlarge it. Both numerator and denominator must fit `U64`.
Frequency must be positive; time offsets may be zero. Wrong dimensions,
negative coordinates, unsupported widths, exhausted work and mismatched original
literal spelling refuse before transport.

For example, checked `440Hz` prepares `440/1` Hz, `1.25Hz` prepares `5/4` Hz,
and `250ms` prepares `1/4` second. The preparation result retains the borrowed
original literal and checked quantity in Core's conversion receipt, alongside
the admitted Audio value and canonical bytes. The enclosing checked Source and
Plan retain the source document identity. `AudioFrequencyControl` and
`AudioTimeControl` carry the value with explicit timeline/origin anchor and
provenance through typed ports. A renderer cannot silently replace
these authored values with floating-point controls.

`AudioPhaseReference` expresses signed rational turns at an exact time under an
explicit timeline/origin anchor and provenance. Whole turns are retained.
`AudioPhaseReferenceMatch` refuses a query from a different anchor. This is an
authored reference, not a sample clock or a phase evolution algorithm.

Focused proof: `cargo test -p conduit-audio --test exact_controls` and
`cargo test -p conduit-std-host --test exact_audio_controls`. The installed test
uses actual authored typed control programs, placement, planning and Kernel
execution; it checks exact transported values, source identity and allocation
capacity. These controls do not establish device playback or renderer fidelity.

The generic periodic profile separately realizes an explicit finite phase cursor
from this retained reference; selected phase normalization and its remainder do
not erase original whole turns. Offered gain, noise, independent excitation,
numeric profiles and catalog paths are mapped in
[CONTROL_CONTRACT.md](CONTROL_CONTRACT.md). Clock/control-cadence completion
requires its separate positive binding/evaluation proof.
