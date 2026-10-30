//! Finite current-binding, truthful fallback and visual/linear typed-return proof.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "action-prompt-i1.source-policy",
        "Check source-owned finite availability fallback and prepared no-growth evaluation",
        "cargo",
        &[
            "test", "--locked", "-p", "conduit-face", "--test", "parsec_input_policy",
            "--test", "parsec_input_policy_producer", "--", "--test-threads=1",
        ],
    ),
    Step::new(
        "action-prompt-i1.current-encounters",
        "Prove current Start instructions, native raster and linear typed returns with stale refusal",
        "cargo",
        &[
            "test", "--locked", "-p", "conduit-std-host", "--lib",
            "action_prompt_i1_", "--", "--nocapture", "--test-threads=1",
        ],
    ),
    Step::new(
        "action-prompt-i1.mask-regression",
        "Check shared terminal and remote Mask execution after carrier completion extraction",
        "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "terminal_mask_execution", "--", "--test-threads=1"],
    ),
    Step::new(
        "action-prompt-i1.installed-input-regression",
        "Check existing installed Parsec software-input and source transitions",
        "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "parsec_live_fore_tests", "--", "--test-threads=1"],
    ),
    Step::new(
        "action-prompt-i1.owner-regression",
        "Check retained terminal owner input, prompt and stale-scope behavior",
        "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "parsec_terminal_owner", "--", "--test-threads=1"],
    ),
];
