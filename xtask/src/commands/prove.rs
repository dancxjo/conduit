use crate::{
    cli::{GlobalOpts, ProveArgs, ProveName},
    process::{run_steps, StepError},
    suites::browser,
    workspace::workspace_root,
};

pub fn run(args: ProveArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root()
        .map_err(|e| StepError::prereq("workspace-root", e))?;

    let steps = match args.name {
        ProveName::StdBrowserS4 | ProveName::StdBrowser => browser::PROVE_STD_BROWSER_S4,
    };
    run_steps(steps, &root, opts)?;
    Ok(())
}
