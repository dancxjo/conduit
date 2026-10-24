//! Internal host stage of the supported attended display-readiness proof.
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
use std::{
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
fn read(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 {
        return Err("capture ingress capacity".into());
    }
    Ok(bytes)
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().ok_or("expected capture directory")?);
    if args.next().is_some() {
        return Err("expected one capture directory".into());
    }
    let before: serde_json::Value = serde_json::from_slice(&read(&output.join("readiness.json"))?)?;
    let after: serde_json::Value =
        serde_json::from_slice(&read(&output.join("readiness-after.json"))?)?;
    for reply in [&before, &after] {
        if reply["event"] != "native-root-offer"
            || reply["body_installed"] != false
            || reply["native_plan_installed"] != false
            || reply["display_software_ready"] != true
            || reply["sampling_software_ready"] != false
        {
            return Err("wrong Root preparation profile".into());
        }
    }
    let source = DisplayOutputSource::compile().map_err(|why| format!("Source: {why}"))?;
    let (host, grants) = source
        .observe_native_offers(
            &serde_json::from_value(before["advertisement"].clone())?,
            &read(&output.join("software-offer.json"))?,
            &serde_json::from_value(after["advertisement"].clone())?,
        )
        .map_err(|why| format!("observed output: {why:?}"))?;
    let bases: Vec<_> = core::iter::once("conduit.base/local@1".into())
        .chain(host.bases.iter().map(|base| base.implementation_id.clone()))
        .collect();
    let prepared = PreparedDisplayOutput::prepare(&source, &host, &bases, &grants)
        .map_err(|why| format!("output planning: {why:?}"))?;
    fs::write(
        output.join("uninstalled-output-plan.json"),
        serde_json::to_vec_pretty(prepared.plan())?,
    )?;
    let receipt = serde_json::json!({"event":"observed-display-plan", "host_id":host.host_id, "boot_id":host.boot_id,
        "plan_id":prepared.plan().plan_id,"nodes":prepared.lowered().nodes.len(),"cords":prepared.lowered().cords.len(),
        "fore_ports":prepared.lowered().fore_ports.len(),"host_calls":prepared.lowered().host_calls.len(),
        "installed":false,"capability_possession_issued":false,"physical_display_delivery":false});
    fs::write(
        output.join("planning.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    println!("{receipt}");
    Ok(())
}
