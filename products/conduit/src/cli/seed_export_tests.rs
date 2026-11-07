use super::*;
use clap::Parser;
#[test]
fn explicit_local_seed_export_requires_state_and_destination() {
    let parsed = Cli::try_parse_from([
        "conduit",
        "body",
        "export-seed",
        "--state-dir",
        "owner-state",
        "seed.zip",
    ])
    .unwrap();
    assert!(
        matches!(parsed.command,Some(Command::Body{command:Some(BodyCommand::ExportSeed{state_dir,destination})}) if state_dir==std::path::Path::new("owner-state") && destination==std::path::Path::new("seed.zip"))
    );
    assert!(Cli::try_parse_from(["conduit", "body", "export-seed", "seed.zip"]).is_err());
    assert!(Cli::try_parse_from([
        "conduit",
        "body",
        "export-seed",
        "--state-dir",
        "owner-state"
    ])
    .is_err());
}
