use super::*;
use clap::Parser;
#[test]
fn browser_window_key_pin_can_replace_first_host_name_but_never_authorization() {
    let base = [
        "conduit",
        "body",
        "browser-window",
        "--state-dir",
        "owner-state",
    ];
    assert!(Cli::try_parse_from(base.iter().copied().chain(["--authorize-window"])).is_err());
    let key = serde_json::to_string(&[7_u8; 32]).unwrap();
    let parsed = Cli::try_parse_from(base.iter().copied().chain([
        "--new-host-verifying-key",
        &key,
        "--authorize-window",
    ]))
    .unwrap();
    assert!(matches!(
        parsed.command,
        Some(Command::Body {
            command: Some(BodyCommand::BrowserWindow {
                expected_host_id: None,
                new_host_verifying_key: Some(_),
                ..
            })
        })
    ));
    assert!(Cli::try_parse_from(
        base.iter()
            .copied()
            .chain(["--new-host-verifying-key", &key])
    )
    .is_err());
    let parsed = Cli::try_parse_from(base.iter().copied().chain([
        "--expected-host-id",
        "host/known",
        "--authorize-window",
    ]))
    .unwrap();
    assert!(matches!(
        parsed.command,
        Some(Command::Body {
            command: Some(BodyCommand::BrowserWindow {
                expected_host_id: Some(_),
                new_host_verifying_key: None,
                ..
            })
        })
    ));
}
