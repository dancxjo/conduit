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
    Step::new("pete-cognition.experience-response", "Send selected evidence to an ordinary model Gear across revisions", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--test", "experience_response"]),
    Step::new("pete-cognition.installed-episodes", "Exercise two episodes and failures through the installed std kernel with a fenced fake model", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--test", "episode_loop_execution", "--test", "episode_loop_failure"]),
];

pub const LIVE: &[Step] = &[Step::new(
    "pete-cognition.live-episodes",
    "Run two linked episodes through the installed Ollama model Back",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "conduit-std-host",
        "--test",
        "episode_loop_live",
        "--test",
        "experience_response",
        "--",
        "--ignored",
        "--nocapture",
    ],
)];
