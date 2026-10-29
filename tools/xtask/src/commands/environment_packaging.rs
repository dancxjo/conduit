//! Build hygiene for the bounded member profile; this grants no runtime authority.
use std::{collections::BTreeSet, error::Error, path::Path, process::Command};

// Every Conduit runtime dependency needs an explicit packaging decision here.
// Build-host Source generation is a separate graph and deliberately excluded.
const ALLOWED: &[&str] = &[
    "conduit-assigned-plan",
    "conduit-body",
    "conduit-computation",
    "conduit-core",
    "conduit-environment-face",
    "conduit-environment-http",
    "conduit-environment-measurement",
    "conduit-environment-runtime",
    "conduit-esp32-network-switch",
    "conduit-esp32-pulse-capture",
    "conduit-esp32-tm1637-probe",
    "conduit-face",
    "conduit-human",
    "conduit-kernel",
    "conduit-language",
    "conduit-monotonic-wait",
    "conduit-net",
    "conduit-network-join-call",
    "conduit-plan-lowering",
    "conduit-planner",
    "conduit-plot",
    "conduit-pulse-capture",
    "conduit-rp2040-network-realization",
    "conduit-seven-segment-mask",
    "conduit-time",
    "conduit-tm1637-wire",
];

pub(super) fn check_packages(
    package: &Path,
    channel: &str,
    profile: &str,
) -> Result<(), Box<dyn Error>> {
    let output = Command::new("cargo")
        .current_dir(package)
        .arg(format!("+{channel}"))
        .args([
            "tree",
            "--locked",
            "--target",
            "xtensa-esp32-none-elf",
            "--features",
            profile,
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "member runtime dependency inspection failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let text = String::from_utf8(output.stdout)?;
    let packages: BTreeSet<_> = text
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    validate_packages(&packages)?;
    println!("member packaging: {} runtime packages, {} reviewed Conduit packages; local planner retained",
        packages.len(), packages.iter().filter(|name| name.starts_with("conduit-")).count());
    Ok(())
}

fn validate_packages(packages: &BTreeSet<&str>) -> Result<(), Box<dyn Error>> {
    for name in packages {
        if name.starts_with("conduit-") && !ALLOWED.contains(name) {
            return Err(format!("unreviewed member runtime dependency: {name}; keep broad catalogues and Make tools on the build host").into());
        }
    }
    for required in [
        "conduit-body",
        "conduit-kernel",
        "conduit-planner",
        "conduit-computation",
    ] {
        if !packages.contains(required) {
            return Err(format!("member runtime lost required package {required}").into());
        }
    }
    Ok(())
}

pub(super) fn check_linked(nm: &Path, image: &Path) -> Result<(), Box<dyn Error>> {
    inspect_linked(nm, image, false)
}
pub(super) fn check_maintenance_linked(nm: &Path, image: &Path) -> Result<(), Box<dyn Error>> {
    inspect_linked(nm, image, true)
}
fn inspect_linked(nm: &Path, image: &Path, maintenance: bool) -> Result<(), Box<dyn Error>> {
    let output = Command::new(nm).arg("--demangle").arg(image).output()?;
    if !output.status.success() {
        return Err(format!(
            "member linked-symbol inspection failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let symbols = String::from_utf8(output.stdout)?;
    if maintenance {
        validate_maintenance_symbols(&symbols)?;
        println!("maintenance packaging: typed custody linked; autonomous execution absent");
        return Ok(());
    }
    validate_symbols(&symbols)?;
    println!(
        "member packaging: indexed task planner linked; Source parsing/checking/expansion absent"
    );
    Ok(())
}

fn validate_symbols(symbols: &str) -> Result<(), Box<dyn Error>> {
    if ![
        "conduit_planner::indexed_source::plan_with_connection_limits",
        "conduit_planner::indexed_source::compact::plan_compact_consuming_with_connection_limits",
    ]
    .iter()
    .any(|planner| symbols.contains(planner))
    {
        return Err("member image lost its local indexed task planner".into());
    }
    validate_no_authoring(symbols)
}
fn validate_maintenance_symbols(symbols: &str) -> Result<(), Box<dyn Error>> {
    for required in [
        "native_biography_uart::",
        "native_restart::biography::retain_receipt",
    ] {
        if !symbols.contains(required) {
            return Err(format!("maintenance image lost typed custody: {required}").into());
        }
    }
    for execution in [
        "::start_local_appliance",
        "native_worker::advance",
        "::install_linked_body",
    ] {
        if symbols.contains(execution) {
            return Err(
                format!("maintenance image retained autonomous execution: {execution}").into(),
            );
        }
    }
    validate_no_authoring(symbols)
}
fn validate_no_authoring(symbols: &str) -> Result<(), Box<dyn Error>> {
    for authoring in [
        "conduit_plot::parse_syntax_document",
        "conduit_plot::check_syntax_document",
        "conduit_plot::canonical_expansion::",
    ] {
        if symbols.contains(authoring) {
            return Err(
                format!("member image retained build-host authoring code: {authoring}").into(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packaging_refuses_catalogue_growth_and_planner_removal() {
        let mut packages: BTreeSet<_> = ALLOWED.iter().copied().collect();
        validate_packages(&packages).unwrap();
        packages.insert("conduit-semantic-catalog");
        assert!(validate_packages(&packages).is_err());
        packages.remove("conduit-semantic-catalog");
        packages.remove("conduit-planner");
        assert!(validate_packages(&packages).is_err());
    }
    #[test]
    fn linked_check_keeps_task_planning_without_source_authoring() {
        let planner = "conduit_planner::indexed_source::plan_with_connection_limits";
        validate_symbols(planner).unwrap();
        validate_symbols("conduit_planner::indexed_source::compact::plan_compact_consuming_with_connection_limits").unwrap();
        assert!(validate_symbols("").is_err());
        assert!(
            validate_symbols(&format!("{planner}\nconduit_plot::parse_syntax_document")).is_err()
        );
    }
    #[test]
    fn maintenance_requires_custody_and_refuses_autonomous_execution() {
        let custody = "native_biography_uart::offer\nnative_restart::biography::retain_receipt";
        validate_maintenance_symbols(custody).unwrap();
        assert!(validate_maintenance_symbols("").is_err());
        assert!(
            validate_maintenance_symbols(&format!("{custody}\nnative_worker::advance")).is_err()
        );
        assert!(validate_maintenance_symbols(&format!(
            "{custody}\nconduit_plot::parse_syntax_document"
        ))
        .is_err());
    }
}
