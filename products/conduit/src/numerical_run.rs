//! Ordinary Source -> installed numerical selection -> Body, driven by stdin Fore.
mod evidence;
use crate::plot_source::CanonicalSource;
use conduit_core::*;
use conduit_std_host::{numerical_fore::NumericalFore, StdHost, StdHostComposition, StdHostConfig};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
    path::Path,
};

pub(crate) fn run(
    source: &CanonicalSource,
    report_path: Option<&Path>,
    artifacts: Option<&Path>,
) -> Result<(), String> {
    if artifacts.is_some_and(Path::exists) {
        return Err("execution artifact destination already exists".into());
    }
    let checked = source.check()?;
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "quantity-projection/main",
        source.authoring_catalog(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let (host, plan) = prepare(source, &checked, &authored)?;
    let metadata = (report_path.is_some() || artifacts.is_some())
        .then(|| (host.advertisement().clone(), plan.clone()));
    let mut execution = NumericalFore::from_plan(host, plan)?;
    let body_plan = metadata.as_ref().map(|_| execution.body_plan().clone());
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut buffer = Vec::with_capacity(1025);
    let mut failure = None;
    loop {
        buffer.clear();
        let length = match (&mut input).take(1025).read_until(b'\n', &mut buffer) {
            Ok(n) => n,
            Err(e) => {
                failure = Some(e.to_string());
                break;
            }
        };
        if length == 0 {
            break;
        }
        let result = (|| -> Result<(), String> {
            if length > 1024 {
                return Err("numerical Fore line exceeds 1024 bytes".into());
            }
            let line = std::str::from_utf8(&buffer)
                .map_err(|e| e.to_string())?
                .trim();
            if line.is_empty() {
                return Ok(());
            }
            let quantity =
                Quantity::parse_plot_literal(line).map_err(|e| format!("Quantity: {e:?}"))?;
            let result = execution.execute(quantity)?;
            let record = match result.value.representation() {
                VerifiedNearRepresentation::ExactRational => {
                    serde_json::json!({"source":line,"representation":"exact","within_declared_limit":true})
                }
                VerifiedNearRepresentation::Binary32 { target, .. } => {
                    serde_json::json!({"source":line,"representation":"binary32","value":f32::from_bits(target.bits()),"within_declared_limit":true})
                }
            };
            writeln!(&mut output, "{record}").map_err(|e| e.to_string())?;
            output.flush().map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(why) = result {
            failure = Some(why);
            break;
        }
    }
    let report = execution.finish()?;
    // Mandatory child evidence is checked even for reportless invocation.
    report
        .each_child_signs
        .as_ref()
        .map_err(|e| format!("numerical child evidence: {e:?}"))?;
    if let Some((advertisement, plan)) = metadata {
        let body = body_plan
            .as_ref()
            .ok_or("numerical Body evidence basis is missing")?;
        let native = evidence::render(source, &advertisement, &plan, body, &report)?;
        if let Some(path) = report_path {
            write_json(path, &native)?;
        }
        if let Some(directory) = artifacts {
            std::fs::create_dir(directory).map_err(|e| e.to_string())?;
            std::fs::write(directory.join("source.conduit"), &source.source)
                .map_err(|e| e.to_string())?;
            for (name, value) in [
                (
                    "plan.json",
                    serde_json::to_value(&plan).map_err(|e| e.to_string())?,
                ),
                (
                    "body-plan.json",
                    serde_json::to_value(body).map_err(|e| e.to_string())?,
                ),
                (
                    "play.json",
                    serde_json::to_value(&report.play).map_err(|e| e.to_string())?,
                ),
                ("sign.json", native["each_child_signs"].clone()),
                ("report.json", native),
            ] {
                write_json(&directory.join(name), &value)?;
            }
        }
    }
    if let Some(why) = failure {
        return Err(why);
    }
    if report.terminal != TerminalDisposition::Completed
        || report.each_cancellation_failed
        || report.each_output_completion_failed
    {
        return Err(format!(
            "numerical Body ended: {:?}; {:?}",
            report.terminal, report.failure
        ));
    }
    Ok(())
}
fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn prepare(
    source: &CanonicalSource,
    checked: &conduit_plot::CheckedSyntaxDocument,
    authored: &conduit_plot::ExpandedAuthoringPlot,
) -> Result<(StdHost, Plan), String> {
    if authored.expanded.name != "quantity-projection/main"
        || authored.expanded.activations.len() != 1
    {
        return Err(
            "numerical stdin Fore requires one authored quantity-projection/main Each".into(),
        );
    }
    let activation = &authored.expanded.activations[0];
    let child = conduit_plot::expand_canonical_plot_for_authoring(
        checked,
        &activation.selected_plot,
        source.authoring_catalog(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "std/quantity-projection-cli".into(),
            boot_id: format!(
                "boot/numerical-cli/{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos()
            )
            .into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    host.prepare_near_each_realization(activation, &child.expanded)?;
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&authored.expanded, &hosts)
        .map_err(|e| format!("{e:?}"))?;
    let boundaries = authored
        .input_bindings
        .iter()
        .map(|binding| {
            (
                PortDirection::Input,
                binding.front_port_id.clone(),
                QUANTITY_VALUE_MAX_ENCODED_BYTES as u32,
            )
        })
        .chain(authored.output_bindings.iter().map(|binding| {
            (
                PortDirection::Output,
                binding.front_port_id.clone(),
                NEAR_QUANTITY_EVIDENCE_MAX_BYTES as u32,
            )
        }))
        .map(|(direction, front_port_id, byte_capacity)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id,
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        checked,
        authored,
        source.authoring_catalog(),
        &conduit_plot::CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: NEAR_QUANTITY_EVIDENCE_MAX_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|e| format!("{e:?}"))?;
    Ok((host, plan))
}

#[cfg(test)]
mod tests;
