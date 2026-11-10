//! Deterministic cognition contracts; fake-provider evidence is explicitly fenced.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new("pete-cognition.adapters", "Check Pete's typed experience adapters", "cargo",
        &["test", "--locked", "-p", "conduit-pete", "--lib", "pete_"]),
    Step::new("pete-cognition.evidence", "Check bounded explicit evidence projection", "cargo",
        &["test", "--locked", "-p", "conduit-semantic-catalog", "--features", "plot-catalog", "--test", "experience_evidence"]),
    Step::new("pete-cognition.installed-evidence", "Check the installed allocation-free evidence projection Back", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "experience_evidence"]),
    Step::new("pete-cognition.episode-contracts", "Check finite episode state and provenance", "cargo",
        &["test", "--locked", "-p", "conduit-chat", "--features", "plot-catalog", "--lib", "episode_loop"]),
    Step::new("pete-cognition.installed-episodes", "Exercise two episodes and failures through the installed std kernel with a fenced fake model", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--test", "episode_loop_execution", "--test", "episode_loop_failure"]),
];
