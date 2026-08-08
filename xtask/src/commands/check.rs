use crate::{
    cli::{CheckArgs, CheckSuite, GlobalOpts},
    process::{run_suite, StepError},
    suites::check::{
        BROWSER_CHECK_STEPS, FORM_S3_STEPS, KERNEL_TAKEOVER_STEPS, OBSERVATORY_READINESS_STEPS,
        PLANNING_S2_STEPS, REALM_READINESS_STEPS, SIM_READINESS_STEPS, STD_CATALOG_READINESS_STEPS,
        WORKSPACE_STEPS,
    },
    suites::pico_compositions::PICO_COMPOSITION_STEPS,
    workspace::workspace_root,
};

pub fn run(args: CheckArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|error| StepError::prereq("workspace-root", error))?;

    match args.suite {
        CheckSuite::Workspace => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)
        }
        CheckSuite::Browser | CheckSuite::BrowserHost => {
            run_suite(BROWSER_CHECK_STEPS, &root, opts)
        }
        CheckSuite::Sim => run_suite(SIM_READINESS_STEPS, &root, opts),
        CheckSuite::KernelTakeover => run_suite(KERNEL_TAKEOVER_STEPS, &root, opts),
        CheckSuite::PlanningS2 => run_suite(PLANNING_S2_STEPS, &root, opts),
        CheckSuite::FormS3 => run_suite(FORM_S3_STEPS, &root, opts),
        CheckSuite::Realm => run_suite(REALM_READINESS_STEPS, &root, opts),
        CheckSuite::Observatory => run_suite(OBSERVATORY_READINESS_STEPS, &root, opts),
        CheckSuite::StdCatalog => run_suite(STD_CATALOG_READINESS_STEPS, &root, opts),
        CheckSuite::All => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)?;
            run_suite(BROWSER_CHECK_STEPS, &root, opts)
        }
    }
}
