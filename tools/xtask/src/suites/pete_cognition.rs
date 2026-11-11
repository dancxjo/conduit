//! Deterministic cognition contracts; fake-provider evidence is explicitly fenced.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new("pete-cognition.cadence", "Run reaction, attention, and bounded reflection in one Play under provider delay and loss", "cargo",
        &["test", "--locked", "-p", "xtask", "--test", "pete_cadence"]),
    Step::new("pete-cognition.finite-sampling", "Check finite Current sampling and source closure through the installed kernel", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "current_sample_finite"]),
    Step::new("pete-cognition.ownership", "Check that foundational crates do not depend on Pete", "cargo",
        &["test", "--locked", "-p", "conduit-pete", "--test", "ownership"]),
    Step::new("pete-cognition.adapters", "Check Pete's typed experience adapters", "cargo",
        &["test", "--locked", "-p", "conduit-pete", "--lib", "pete_"]),
    Step::new("pete-cognition.experience-integration", "Check generic batch admission, exact provenance, and freshness refusals", "cargo",
        &["test", "--locked", "-p", "conduit-human", "--test", "experience_integration"]),
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
    Step::new("pete-cognition.pete-frames", "Run typed Pete frames through Situation and recall", "cargo",
        &["test", "--locked", "-p", "xtask", "--test", "experience_episode_loop"]),
];

pub const LIVE: &[Step] = &[
    Step::new(
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
    ),
    Step::new(
        "pete-cognition.live-pete-frames",
        "Run Pete's typed frames and recall with the installed model",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "xtask",
            "--test",
            "experience_episode_loop",
            "--",
            "--ignored",
            "--nocapture",
        ],
    ),
];
