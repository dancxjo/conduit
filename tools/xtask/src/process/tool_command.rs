//! Tool resolution shared by the full and dependency-light xtask entrances.
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

/// Build a command for a tool, accepting Cargo-installed binaries even when the
/// invoking shell did not source Cargo's PATH setup.
pub fn command_for(program: &str) -> Command {
    Command::new(resolve_program(program))
}

fn resolve_program(program: &str) -> PathBuf {
    let program_path = Path::new(program);
    if program_path.components().count() > 1 || path_contains_program(program) {
        return program_path.to_path_buf();
    }

    if let Some(path) = cargo_bin_program(program).filter(|path| path.is_file()) {
        return path;
    }

    program_path.to_path_buf()
}

fn path_contains_program(program: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|path| path.join(program).is_file()))
}

fn cargo_bin_program(program: &str) -> Option<PathBuf> {
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))?;
    Some(cargo_home.join("bin").join(program))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_paths_are_never_replaced_by_cargo_tools() {
        for program in ["./cargo", "/definitely-absent/conduit-tool"] {
            assert_eq!(command_for(program).get_program(), program);
        }
    }

    #[test]
    fn unavailable_bare_tool_retains_normal_spawn_failure() {
        let tool = "conduit-deliberately-absent-tool-for-resolution-test";
        assert_eq!(command_for(tool).get_program(), tool);
        assert_eq!(
            command_for(tool).status().unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
}
