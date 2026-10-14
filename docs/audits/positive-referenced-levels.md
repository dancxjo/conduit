# Positive finite Audio controls

Issue #5743 adds Source-owned positive amplitude and power ratios as separate Types, an explicit referenced ratio constrained to the matching dB convention, and a finite dB level. Zero numerators, zero denominators, mismatched reference domains and negative infinity refuse at this boundary. Existing nonnegative audio/silence contracts retain their meaning.

`PreparedPositiveFiniteDecibels` reuses the accepted exact-power-of-ten profile. Amplitude uses 20 log10 and power uses 10 log10 for rational powers of ten from -18 through 18; other ratios and unsupported inverse values refuse, with the existing original/recognition receipt. No approximate log or inferred physical reference is supplied. Negative finite dB values remain valid for positive ratios below the reference.

The private receipt retains the original positive/finite Type and canonical input alongside the original exact conversion, full reference/unit/role/authored provenance, checked Source programs, inputs and outputs. Existing `AudioExactDecibelProjection` applies to the conversion receipt; the supported transformation is exact, with no numerical approximation error. Authored references grant no calibration or measurement authority and introduce no invented uncertainty estimate.

Focused tests cover all 74 amplitude/power exponent cases against the independent integer dB law, exact inverse ratios, reproducible Source execution, original canonical identity and invalid domains. Existing amplitude/power and broad dB regressions are run alongside them. This is native component evidence, not a physical measurement or a new device Back.
