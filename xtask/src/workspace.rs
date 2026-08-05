use std::path::{Path, PathBuf};

/// Walk up from the current directory to find the workspace root.
pub fn workspace_root() -> Result<PathBuf, String> {
    let start = std::env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    for dir in start.ancestors() {
        let manifest = dir.join("Cargo.toml");
        if manifest.exists() && is_workspace_manifest(&manifest) {
            return Ok(dir.to_path_buf());
        }
    }
    Err("could not locate workspace root (no Cargo.toml with [workspace] found)".into())
}

fn is_workspace_manifest(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|s| s.contains("[workspace]"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_workspace_root_from_nested_directory() {
        let root = workspace_root().expect("workspace root not found");
        assert!(root.join("Cargo.toml").exists());
    }

    #[test]
    fn cargo_config_defines_xtask_alias() {
        let root = workspace_root().expect("workspace root not found");
        let config_path = root.join(".cargo").join("config.toml");
        let config = std::fs::read_to_string(&config_path)
            .expect("could not read .cargo/config.toml");
        assert!(config.contains("xtask = \"run --package xtask --\""));
    }
}
