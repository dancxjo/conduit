//! Canonical IPA and first-class Speech authoring conformance.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new(
        "speech-ipa.nominal-projection",
        "Preserve typed identifier encoding through field projection",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-plot",
            "--test",
            "nominal_field_projection",
            "--locked",
        ],
    ),
    Step::new(
        "speech-ipa.authoring-and-admission",
        "Check authored phone/phoneme values, Unicode notation and inventory custody",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-speech",
            "--features",
            "semantic-bindings",
            "--test",
            "ipa_authoring",
            "--test",
            "ipa_phone",
            "--test",
            "ipa_notation",
            "--test",
            "ipa_inventory",
            "--test",
            "ipa_membership",
            "--test",
            "ipa_source_execution",
            "--locked",
        ],
    ),
];
