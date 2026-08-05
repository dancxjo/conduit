use crate::{
    cli::{CheckArgs, CheckSuite, GlobalOpts},
    process::{run_steps, Step, StepError},
    suites::{browser, catalog, form, kernel, observatory, planning, realm, simulation},
    workspace::workspace_root,
};

pub fn run(args: CheckArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root()
        .map_err(|e| StepError::prereq("workspace-root", e))?;

    let steps: &[Step] = match args.suite {
        CheckSuite::All => {
            run_steps(kernel::KERNEL_S1, &root, opts)?;
            run_steps(kernel::KERNEL_TAKEOVER, &root, opts)?;
            run_steps(planning::PLANNING_S2, &root, opts)?;
            run_steps(form::FORM_S3, &root, opts)?;
            run_steps(browser::BROWSER_S4, &root, opts)?;
            run_steps(realm::REALM, &root, opts)?;
            run_steps(observatory::OBSERVATORY, &root, opts)?;
            run_steps(catalog::STD_CATALOG, &root, opts)?;
            run_steps(simulation::SIMULATION, &root, opts)?;
            return Ok(());
        }
        CheckSuite::KernelS1 => kernel::KERNEL_S1,
        CheckSuite::KernelTakeover => kernel::KERNEL_TAKEOVER,
        CheckSuite::PlanningS2 => planning::PLANNING_S2,
        CheckSuite::FormS3 => form::FORM_S3,
        CheckSuite::BrowserS4 => browser::BROWSER_S4,
        CheckSuite::Realm => realm::REALM,
        CheckSuite::Observatory => observatory::OBSERVATORY,
        CheckSuite::StdCatalog => catalog::STD_CATALOG,
        CheckSuite::Simulation => simulation::SIMULATION,
    };
    run_steps(steps, &root, opts)?;
    Ok(())
}
