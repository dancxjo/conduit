//! Notebook contracts, installed recovery and optional actual packaged browser acceptance.
use crate::{
    cli::GlobalOpts,
    process::{run_suite, Step},
    workspace::workspace_root,
};

pub fn run(
    opts: &GlobalOpts,
    handbook: Option<&std::path::Path>,
    output: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    run_suite(
        &[
            Step::new(
                "notebook.text-law",
                "Check bounded revisioned UTF-8 document transitions and refusal",
                "cargo",
                &["test", "--locked", "--jobs", "2", "-p", "conduit-semantic-catalog", "--lib", "text_state"],
            ),
            Step::new(
                "notebook.installed-owner",
                "Check notebook Source custody, owner edits, committed file selection and failure preservation",
                "cargo",
                &["test", "--locked", "--jobs", "2", "-p", "conduit", "--bin", "conduit", "notebook"],
            ),
            Step::new(
                "notebook.body-file-contracts",
                "Check actual Body execution, authorized file publication, fresh-Boot read and stale-edit refusal",
                "cargo",
                &["test", "--locked", "--jobs", "2", "-p", "conduit-notebook-conformance"],
            ),
        ],
        &root,
        opts,
    )?;
    if cfg!(target_os = "linux") {
        run_suite(
            &[
                Step::new(
                    "notebook.product-binary",
                    "Build the optimized product executable within the installed image bound",
                    "cargo",
                    &["build", "--locked", "--release", "--jobs", "2", "-p", "conduit", "--bin", "conduit"],
                ),
                Step::new(
                    "notebook.installed-process",
                    "Prove Save and same-Body committed recovery through public commands in fresh Linux processes",
                    "python3",
                    &["proof/notebook/installed.py", "--binary", "target/release/conduit"],
                ),
            ],
            &root,
            opts,
        )?;
    }
    if let (Some(handbook), Some(output)) = (handbook, output) {
        if !cfg!(target_os = "linux") {
            return Err("notebook browser owner proof requires Linux".into());
        }
        if opts.dry_run {
            println!(
                "node proof/notebook/browser.mjs target/release/conduit {} {}",
                handbook.display(),
                output.display()
            );
        } else {
            let status = std::process::Command::new("node")
                .args(["proof/notebook/browser.mjs", "target/release/conduit"])
                .arg(handbook)
                .arg(output)
                .current_dir(&root)
                .status()?;
            if !status.success() {
                return Err(format!("notebook browser proof failed: {status}").into());
            }
        }
    }
    Ok(())
}
