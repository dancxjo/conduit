use crate::cli::GlobalOpts;

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would project pinned Netherwick configuration without actuator authority");
        }
        return Ok(());
    }
    let projection = conduit_netherwick::describe_projection();
    conduit_observatory::validate_snapshot(&projection.snapshot)?;
    conduit_netherwick::observation_plan()?;
    conduit_netherwick::attempt_actuator_plan()
        .expect_err("describe-only profile must refuse actuator placement");
    if opts.json {
        println!("{}", serde_json::to_string_pretty(&projection)?);
    } else if !opts.quiet {
        let report = conduit_observatory::build_report(&projection.snapshot)?;
        print!("{}", conduit_observatory::render_text_report(&report));
        println!("actuator-command: refused before Plan (no offer or authority)");
    }
    Ok(())
}
