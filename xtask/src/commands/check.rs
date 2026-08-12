use crate::{
    cli::{CheckArgs, CheckSuite, GlobalOpts},
    process::{run_step, run_suite, StepError},
    suites::check::{
        BROWSER_CHECK_STEPS, FORM_S3_STEPS, INPUT_SEMANTICS_STEPS, KERNEL_TAKEOVER_STEPS,
        OBSERVATORY_READINESS_STEPS, PLANNING_S2_STEPS, SIM_READINESS_STEPS,
        STD_CATALOG_READINESS_STEPS, WORKSPACE_STEPS,
    },
    suites::network_capability::NETWORK_CAPABILITY_STEPS,
    suites::pico_compositions::PICO_COMPOSITION_STEPS,
    suites::workspace_shards::WorkspaceShard,
    workspace::workspace_root,
};

pub fn run(args: CheckArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|error| StepError::prereq("workspace-root", error))?;

    match args.suite {
        CheckSuite::Workspace => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(NETWORK_CAPABILITY_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)
        }
        CheckSuite::WorkspaceLint => run_workspace_shard(WorkspaceShard::Lint, &root, opts),
        CheckSuite::WorkspaceTest => run_workspace_shard(WorkspaceShard::Test, &root, opts),
        CheckSuite::WorkspacePortable => run_workspace_shard(WorkspaceShard::Portable, &root, opts),
        CheckSuite::WorkspacePico => run_workspace_shard(WorkspaceShard::Pico, &root, opts),
        CheckSuite::Browser | CheckSuite::BrowserHost => {
            run_suite(BROWSER_CHECK_STEPS, &root, opts)
        }
        CheckSuite::Sim => run_suite(SIM_READINESS_STEPS, &root, opts),
        CheckSuite::KernelTakeover => run_suite(KERNEL_TAKEOVER_STEPS, &root, opts),
        CheckSuite::PlanningS2 => run_suite(PLANNING_S2_STEPS, &root, opts),
        CheckSuite::FormS3 => run_suite(FORM_S3_STEPS, &root, opts),
        CheckSuite::Observatory => run_suite(OBSERVATORY_READINESS_STEPS, &root, opts),
        CheckSuite::StdCatalog => run_suite(STD_CATALOG_READINESS_STEPS, &root, opts),
        CheckSuite::InputSemantics => run_suite(INPUT_SEMANTICS_STEPS, &root, opts),
        CheckSuite::All => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(NETWORK_CAPABILITY_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)?;
            run_suite(BROWSER_CHECK_STEPS, &root, opts)
        }
    }
}

fn run_workspace_shard(
    shard: WorkspaceShard,
    root: &std::path::Path,
    opts: &GlobalOpts,
) -> Result<(), StepError> {
    for step in WORKSPACE_STEPS
        .iter()
        .chain(NETWORK_CAPABILITY_STEPS)
        .chain(PICO_COMPOSITION_STEPS)
        .filter(|step| shard.owns(step))
    {
        run_step(step, root, opts)?;
    }
    Ok(())
}
