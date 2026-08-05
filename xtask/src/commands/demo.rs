use crate::{
    cli::{DemoArgs, DemoName, GlobalOpts},
    process::{run_steps, Step, StepError},
    workspace::workspace_root,
};

const DEMO_STD: &[Step] = &[Step::new(
    "demo-std",
    "Run std signal demo",
    "cargo",
    &[
        "run",
        "-p",
        "conduit",
        "--",
        "examples/signal-demo.form",
        "--placements",
        "examples/std-local.placements",
    ],
)];

const DEMO_TRIPLE_LOCAL: &[Step] = &[Step::new(
    "demo-triple-local",
    "Run triple-signal local demo",
    "cargo",
    &[
        "run",
        "-p",
        "conduit",
        "--",
        "examples/triple-signal.form",
        "--placements",
        "examples/triple-local.placements",
    ],
)];

pub fn run(args: DemoArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root()
        .map_err(|e| StepError::prereq("workspace-root", e))?;

    let steps: &[Step] = match args.name {
        DemoName::Std => DEMO_STD,
        DemoName::TripleLocal => DEMO_TRIPLE_LOCAL,
    };
    run_steps(steps, &root, opts)?;
    Ok(())
}
