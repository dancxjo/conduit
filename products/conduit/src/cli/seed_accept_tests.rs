use super::*;
use clap::Parser;
#[test]
fn installed_seed_acceptance_requires_separate_membership_request_authorization() {
    let base = [
        "conduit",
        "body",
        "accept-seed",
        "--state-dir",
        "fresh-state",
    ];
    assert!(Cli::try_parse_from(base).is_err());
    let cli = Cli::try_parse_from(base.into_iter().chain(["--authorize-join"])).unwrap();
    assert!(
        matches!(cli.command, Some(Command::Body {command: Some(BodyCommand::AcceptSeed { state_dir, authorize_join:true })}) if state_dir == std::path::Path::new("fresh-state"))
    );
    assert!(Cli::try_parse_from(["conduit", "body", "accept-seed", "--authorize-join"]).is_err());
}
