use conduit_std_host::{load_checked_form, load_placements, StdHost, ThreadTimer};
use std::env;
use std::io;

fn run_with_placements(path: &str, placements_path: Option<&str>) -> Result<(), String> {
    let form = load_checked_form(path).map_err(|err| err.to_string())?;
    let placements = load_placements(placements_path).map_err(|err| err.to_string())?;
    let mut host = StdHost::new();
    let plan = host
        .plan_local(&form, placements.as_ref())
        .map_err(|err| err.to_string())?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == host.advertisement().host_id)
        .ok_or_else(|| "no local fragment for std host".to_string())?;
    let mut stdout = io::stdout().lock();
    host.run_fragment_to(fragment, &mut stdout, &mut ThreadTimer)?;
    Ok(())
}

fn main() {
    let mut args = env::args();
    let _program = args.next();
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: conduit <form-file> [--placements <placements-file>]");
            std::process::exit(2);
        }
    };

    let placements_path = match (args.next().as_deref(), args.next()) {
        (Some("--placements"), value) => value,
        (Some(other), _) => {
            eprintln!(
                "usage: conduit <form-file> [--placements <placements-file>]\nunexpected argument: {other}"
            );
            std::process::exit(2);
        }
        (None, _) => None,
    };

    if let Err(err) = run_with_placements(&path, placements_path.as_deref()) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
