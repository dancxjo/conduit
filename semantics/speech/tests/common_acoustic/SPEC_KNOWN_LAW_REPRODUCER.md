# Generic Known payload parent-law retention reproducer

The adjacent `spec_known_parent_law.conduit` is a minimal isolated Source shape.
The generated `ParentLawProbe::new(1, 2)` rejects numerator2/denominator1, but the
generated flattened `ParentLawKnownProbe::known(1, 2)` accepts it. The instantiated
Known wrapper retains the denominator field contract but not the parent record
`where numerator <= denominator` law. Native decode of the resulting Known
wrapper also accepts it. This concerns ordinary Source generic/native metadata,
not expression evaluation proving forged byte admission.

The actual Speech owner reproduces the same defect with
`SpeechUnitInterval::new(1, 2)` (refuses) versus
`SpeechProbabilitySpecification::known(1, 2)` (currently accepts). The component's
`forged_known_probability_and_temporal_overflow_profile_refuse` test then verifies
that preparation explicitly re-admits that Known payload through the original
`SpeechUnitInterval` owner and refuses it. The full original Spec stays in custody;
there is no arithmetic, recast, normalized value or weakened law.

The adjacent isolated Source is architectural bug evidence, not an accepted
alternative owner. No Source checker/kernel change is included here. Raw generic
Known construction must not be described as proving its nested parent where law.
