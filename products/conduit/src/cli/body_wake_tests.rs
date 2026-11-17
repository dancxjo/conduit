//! Explicit operator Wake keeps duration-limited Start separate.
use super::*;
#[test]
fn operator_wake_requires_its_installed_state_and_rejects_a_run_deadline() {
    assert!(Cli::try_parse_from(["conduit", "body", "wake"]).is_err());
    assert!(matches!(
        Cli::try_parse_from(["conduit", "body", "wake", "--state-dir", "state"])
            .unwrap()
            .command,
        Some(Command::Body {
            command: Some(BodyCommand::Wake { .. })
        })
    ));
    assert!(Cli::try_parse_from([
        "conduit",
        "body",
        "wake",
        "--state-dir",
        "state",
        "--maximum-millis",
        "1000"
    ])
    .is_err());
}
