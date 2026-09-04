use std::process::Command;

pub(super) fn run(arguments: &[String]) -> Result<(), String> {
    for argument in arguments.iter().skip(2) {
        if argument != "--locked" {
            return Err(format!(
                "unsupported ci pages-resolver-proof argument: {argument}"
            ));
        }
    }
    let root = crate::workspace::workspace_root().map_err(|error| error.to_string())?;
    let status = Command::new("node")
        .current_dir(root)
        .args([
            "--test",
            "proof/ci/pages-product-run-selection.spec.mjs",
            "proof/ci/pages-workflow-paths.spec.mjs",
        ])
        .status()
        .map_err(|error| format!("cannot launch Pages resolver proof: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Pages resolver proof failed with {status}"))
    }
}
