//! Deterministic cognition contracts; fake-provider evidence is explicitly fenced.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new("pete-cognition.lint", "Lint the changed cognition libraries and shared Host route", "cargo",
        &["clippy", "--locked", "-p", "conduit-pete", "-p", "conduit-semantic-catalog", "-p", "conduit-human", "-p", "conduit-std-offers", "-p", "conduit-std-host", "--lib", "--", "-D", "warnings"]),
    Step::new("pete-cognition.state-startup", "Check exact initialized State cuts and ordinary cycle refusal", "cargo",
        &["test", "--locked", "-p", "conduit-planner", "--lib", "startup::tests"]),
    Step::new("pete-cognition.adapters", "Check Pete's typed experience adapters", "cargo",
        &["test", "--locked", "-p", "conduit-pete", "--lib", "pete_"]),
    Step::new("pete-cognition.evidence", "Check bounded explicit evidence projection", "cargo",
        &["test", "--locked", "-p", "conduit-semantic-catalog", "--features", "plot-catalog", "--test", "experience_evidence", "--test", "experience_integration", "--test", "experience_prompt", "--test", "experience_autobiography", "--test", "experience_autobiography_recall"]),
    Step::new("pete-cognition.installed-evidence", "Check the installed allocation-free evidence projection Back", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "experience_evidence"]),
    Step::new("pete-cognition.body-model-route", "Check ordinary model Host Calls in a real shared Body Plan", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "model_route_tests"]),
    Step::new("pete-cognition.autobiographical-body", "Prove automatic typed autobiography feedback and recall in one Body", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--lib", "autobiography_tests"]),
    Step::new("pete-cognition.large-state", "Check separately admitted finite large typed State", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--test", "large_typed_state_back"]),
    Step::new("pete-cognition.episode-contracts", "Check finite episode state and provenance", "cargo",
        &["test", "--locked", "-p", "conduit-chat", "--features", "plot-catalog", "--lib", "episode_loop"]),
    Step::new("pete-cognition.installed-episodes", "Exercise two episodes and failures through the installed std kernel with a fenced fake model", "cargo",
        &["test", "--locked", "-p", "conduit-std-host", "--test", "episode_loop_execution", "--test", "episode_loop_failure"]),
];
