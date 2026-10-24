use std::{error::Error, path::Path, process::Command};
mod commands {
    use super::*;
    pub fn execute(command: &mut Command) -> Result<(), Box<dyn Error>> {
        eprintln!("ACTUAL_ENVIRONMENT_HELPER_COMMAND {command:?}");
        let status = command.status()?;
        if !status.success() {
            return Err(format!("actual helper command failed: {status}").into());
        }
        Ok(())
    }
    #[path = "/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit/work/6418-optional-custody-pressure/tools/xtask/src/commands/environment_face_publication.rs"]
    mod environment_face_publication;
    pub fn run(root: &Path) -> Result<(), Box<dyn Error>> {
        environment_face_publication::prove(root)
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    commands::run(Path::new(&std::env::args().nth(1).ok_or("source root required")?))
}
