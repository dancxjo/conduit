# Explicit-reference dB quantities and exact capability

This is a bounded numeric prerequisite for #5212/#5216. General logarithmic
conversion remains required and open. No Speech, clock, Flow, PCM, absolute SPL,
watts, measured impedance, reference calibration or whole-#5216 claim follows.

The broad Source owners retain exact authored fractions without reduction:

| Owner | Domain / declared meaning |
| --- | --- |
| `AudioDecibelReference` | Positive full-U64 numerator/denominator magnitude, existing Core `Unit`, nonempty identity (128B), explicit amplitude-magnitude or power role, existing Audio provenance kind/method/version. |
| `AudioReferencedLevelRatio` | Nonnegative full-U64 fraction x/x_ref, with positive denominator; magnitude reference and convention remain attached. |
| `AudioDecibelFraction` | Signed I64 numerator in dB / positive full-U64 denominator. |
| `AudioDecibelValue` | Finite rational dB or explicit negative infinity. |
| `AudioDecibelBasis` | Amplitude role uses 20 log10(x/x_ref); power role uses 10 log10(x/x_ref), checked by Source law. |

The unit is caller-declared metadata. The role law does not assert which physical
units are appropriate or invent a unit/reference authority. The reference is not
silently changed to unity. Ratios are already relative to that exact reference;
the converter does not divide by its magnitude a second time. Zero ratio maps to
negative infinity under the declared logarithmic convention, and inverse negative
infinity maps to zero ratio. Unknown provenance stays unknown.

`AudioAmplitudePowerReferences` explicitly declares a reference pair under
`same_positive_proportionality`: P=k A² and P_ref=k A_ref² for the same positive k.
That authored premise makes the squared amplitude ratio the power ratio; it does
not measure or validate k. The adapter reuses the existing Source amplitude square
with its separate U32 operand eligibility and retains that complete receipt plus
the original full reference-pair request. Reference magnitude consistency under
an unknown k is declared, not inferred from unit names or identities.

`PreparedExactPowerOfTenDecibels` is the explicitly named first capability:

* Forward accepts exact ratios 10^e for integer e in [-18,18], plus zero. Source
  recognizes quotient **and remainder** in either direction; it never multiplies
  a full-U64 denominator by a power of ten. Unreduced fractions remain in custody.
* Inverse accepts negative infinity, or finite authored denominator exactly 1
  with numerator an exact multiple of 10 (power) or 20 (amplitude), within the same
  exponent range. Reducible denominator-2 forms remain canonical but explicitly
  refuse this profile. No normalization or approximate logarithm is hidden.
* Supported power tables use balanced Source decision trees within the existing
  portable expression depth16 bound. No kernel or bound change is required.

Full canonical Native input admission precedes Source execution. Source performs
recognition, exponent decision and numerical computation. The raw inverse result
has unrestricted U64 fields; the adapter only copies these fields into the
semantic ratio and Native-admits positivity afterward. A forged zero denominator
refuses at this boundary. The forward finite denominator is the Source-authored
constant 1 and is Native-admitted after execution.

Opaque receipts expose borrowed original typed request, original canonical frame,
all exact program hex/input/output frames, typed result and final admitted frame.
Unsupported numeric profiles retain the admitted original frame and executed
Source recognition evidence. They do not substitute zero or approximate values.
`AudioExactDecibelProjection` uses actual Core `ProjectionReport` contracts and
sealed execution proof. It admits only the exact transformation with unchanged
reference/unit/role/provenance/convention and permits no loss. It rejects omitted
facts, fabricated preservation, changed targets and approximation claims.

Measured canonical static Type / representative value bytes are 1721 / 2099 for
the referenced ratio and 1590 / 1972 for finite dB. These complete semantic carriers
are not a claim of admission to a 2KiB Flow value profile. This is allocating
preparation and execution evidence; no real-time or Host/Plan guarantee is made.

Validation covers all 74 amplitude/power exponent combinations and inverse
cross-products against independent u128 arithmetic; zero/negative infinity;
full-U64 unreduced equality and safe recognition; unsupported general ratios,
10^19, fractional dB and I64 extremes; reference-role laws; complete original
custody; program replay; explicit amplitude→power→dB equivalence; corrupted public
frames; forged raw denominator admission; Core exact fidelity/refusals; and actual
canonical Type/value sizes. General non-power-of-ten logarithms and quantized
error bounds are deliberately not implemented by this checkpoint.
