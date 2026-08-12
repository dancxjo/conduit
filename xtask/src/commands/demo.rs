//! Repository demonstration entrances that hide package and fixture details.

use crate::cli::{GlobalOpts, PatchbayDemoArgs};
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;

pub fn run_std(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    run(
        "demo.std",
        "Run the native Signal Form",
        &[
            "run",
            "-p",
            "conduit",
            "--",
            "run",
            "examples/signal-demo.form",
            "--placements",
            "examples/std-local.placements",
        ],
        opts,
    )
}

pub fn run_triple(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    run(
        "demo.triple",
        "Run the three-sink Form locally",
        &[
            "run",
            "-p",
            "conduit",
            "--",
            "run",
            "examples/triple-signal.form",
            "--placements",
            "examples/triple-local.placements",
        ],
        opts,
    )
}

fn run(
    id: &'static str,
    description: &'static str,
    args: &'static [&'static str],
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(id, description, "cargo", args);
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_patchbay(
    args: &PatchbayDemoArgs,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let command = if args.first_run_proof {
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--form",
            "examples/default-welcome.conduit",
            "--first-run-proof",
        ][..]
    } else {
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--form",
            "examples/default-welcome.conduit",
        ][..]
    };
    let step = Step::new(
        "demo.patchbay",
        if args.first_run_proof {
            "Prove the bounded native Patchbay first-run journey"
        } else {
            "Build and launch the native Patchbay from this checkout"
        },
        "cargo",
        command,
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_body_membership(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    super::body_membership_demo::run(opts)
}

pub fn run_environment(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "demo.environment",
        "Open the bounded authored physical-environment workspace",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--environment",
            "examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}

pub fn run_prewake(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let step = Step::new(
        "demo.prewake",
        "Rehearse the canonical Form against authored simulation truth",
        "cargo",
        &[
            "run",
            "-p",
            "patchbay-native",
            "--",
            "--prewake",
            "--form",
            "examples/hello.conduit",
            "--environment",
            "examples/maker-workbench.json",
        ],
    );
    run_step(&step, &root, opts)?;
    Ok(())
}
