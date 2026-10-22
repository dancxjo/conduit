//! Generic Source/Plan/Fore correspondence; no Language model fixture dependency.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new("canonical-export.correspondence", "Execute generic wide-payload canonical exports and reject resealed Source/Fore/graph forgeries", "cargo", &[
        "test", "--locked", "-p", "conduit-composite", "--features", "fixture-registry-preparation", "--test", "canonical_export", "--", "--nocapture",
    ]),
    Step::new("canonical-export.channel", "Execute numerical custody and canonical submit/output-ACK regression coverage without Language policy", "cargo", &[
        "test", "--locked", "-p", "conduit-ai", "--features", "kernel-operation-owners", "--test", "integer_categorical_step", "--", "--nocapture", "--test-threads=1",
    ]),
    Step::new("canonical-export.strict", "Check strict generic export and correspondence regressions", "cargo", &[
        "clippy", "--locked", "-p", "conduit-composite", "--features", "fixture-registry-preparation", "--test", "canonical_export", "--", "-D", "warnings",
    ]),
    Step::new("canonical-export.bridge", "Check strict numerical canonical bridge without Language policy", "cargo", &[
        "clippy", "--locked", "-p", "conduit-ai", "--features", "kernel-operation-owners", "--lib", "--test", "integer_categorical_step", "--", "-D", "warnings",
    ]),
];
