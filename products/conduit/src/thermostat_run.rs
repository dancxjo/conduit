//! Ordinary CLI Play for the exact authored Thermostat scan, with typed stdin controls.
use crate::plot_source::CanonicalSource;
use conduit_core::{
    port_id, BaseImplementationId, ConnectionTrack, PlannedActivationEntry, PortDirection,
};
use conduit_std_host::{StdHost, StdHostConfig};
use conduit_thermostat_plot::{Command, Fan, Mode, Preset, ThermostatState};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
};

pub(crate) fn run(
    source: &CanonicalSource,
    plot: &conduit_plot::ExpandedAuthoringPlot,
    report_path: Option<&std::path::Path>,
    artifacts: Option<&std::path::Path>,
) -> Result<(), String> {
    if artifacts.is_some_and(|path| path.exists()) {
        return Err("execution artifact destination already exists".into());
    }
    let (host, plan) = prepare(source, plot)?;
    let advertisement = host.advertisement().clone();
    let source_plan = plan.clone();
    let mut execution = conduit_thermostat_app::execution::Execution::from_plan(host, plan)?;
    let body_plan = execution.body_plan().clone();
    let mut output = std::io::stdout().lock();
    let mut input = std::io::stdin().lock();
    let mut line_buffer = Vec::with_capacity(1025);
    while let Some(line) = bounded_control_line(&mut input, &mut line_buffer)? {
        if line.trim().is_empty() {
            continue;
        }
        let state = execution.execute(parse_command(line)?)?.state;
        let json = serde_json::json!({"revision":state.revision,"target":state.target,"mode":format!("{:?}",state.mode).to_lowercase(),"fan":format!("{:?}",state.fan).to_lowercase(),"preset":format!("{:?}",state.preset).to_lowercase(),"measured":state.measured});
        writeln!(&mut output, "{json}").map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
    let report = execution.finish()?;
    let child_signs = report.scan_child_signs.as_ref().map_err(|e| format!("child Signs: {e:?}"))?.iter().map(|child| serde_json::json!({
        "parent_active_play_id": child.parent_active_play_id, "activation_id":child.activation_id,
        "selected_plan_id":child.selected_plan_id,"invocation":child.invocation,"child_host_id":child.child_host_id,
        "child_active_play_id":child.child_active_play_id,"events":child.events.iter().map(event_json).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    let native = serde_json::json!({"schema":"conduit.thermostat/native-body-run@1","proof_class":"local-hosted",
        "authored_source":source.source, "advertisement": advertisement, "source_plan": source_plan, "body_plan":body_plan,
        "play":report.play,"terminal":report.terminal,"terminal_sign":report.terminal_sign,
        "kernel_events":report.kernel_events.iter().map(event_json).collect::<Vec<_>>(),"scan_child_signs":child_signs,
        "failure":report.failure,"cleanup_failure":report.cleanup_failure,
        "scan_cancellation_failed":report.scan_cancellation_failed,"scan_output_completion_failed":report.scan_output_completion_failed,
        "outputs":report.fore_deliveries.iter().map(|delivery| serde_json::json!({"front_port_id":delivery.front_port_id,"value_kind":delivery.value_kind,"sequence":delivery.sequence,"track":delivery.track,"bytes":delivery.bytes})).collect::<Vec<_>>()});
    if let Some(path) = report_path {
        write_json(path, &native)?;
    }
    if let Some(directory) = artifacts {
        std::fs::create_dir(directory).map_err(|e| e.to_string())?;
        std::fs::write(directory.join("source.conduit"), &source.source)
            .map_err(|e| e.to_string())?;
        for (file, value) in [
            (
                "plan.json",
                serde_json::json!({"schema":"conduit.thermostat/source-plan@1","plan":source_plan}),
            ),
            (
                "body-plan.json",
                serde_json::json!({"schema":"conduit.thermostat/body-plan@1","body_plan":body_plan}),
            ),
            (
                "play.json",
                serde_json::json!({"schema":"conduit.thermostat/body-play@1","identity":report.play}),
            ),
            (
                "sign.json",
                serde_json::json!({"schema":"conduit.thermostat/body-sign@1","identity":report.terminal_sign,"scan_child_signs":child_signs}),
            ),
            ("report.json", native),
        ] {
            write_json(&directory.join(file), &value)?;
        }
    }
    if report.cleanup_failure.is_some()
        || report.scan_cancellation_failed
        || report.scan_output_completion_failed
    {
        return Err(
            "Thermostat Play cleanup or output completion failed; see native report".into(),
        );
    }
    if report.terminal != conduit_core::TerminalDisposition::Completed {
        return Err(format!("Thermostat Play ended: {:?}", report.terminal));
    }
    Ok(())
}
fn bounded_control_line<'a>(
    reader: &mut impl BufRead,
    buffer: &'a mut Vec<u8>,
) -> Result<Option<&'a str>, String> {
    buffer.clear();
    let read = (&mut *reader)
        .take(1025)
        .read_until(b'\n', buffer)
        .map_err(|e| e.to_string())?;
    if read == 0 {
        return Ok(None);
    }
    if read > 1024 {
        return Err("Thermostat control line exceeds 1024 bytes".into());
    }
    std::str::from_utf8(buffer)
        .map(Some)
        .map_err(|e| format!("Thermostat control UTF-8: {e}"))
}

fn write_json(path: &std::path::Path, value: &serde_json::Value) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn event_json(event: &conduit_kernel::KernelEvent) -> serde_json::Value {
    serde_json::json!({"sequence":event.sequence,"node":event.node.0,"port":event.port.map(|p|p.0),"request":event.request.map(|r|r.0),"kind":format!("{:?}",event.kind)})
}

fn prepare(
    source: &CanonicalSource,
    plot: &conduit_plot::ExpandedAuthoringPlot,
) -> Result<(StdHost, conduit_core::Plan), String> {
    if plot.expanded.name != "thermostat/main" || plot.expanded.activations.len() != 1 {
        return Err("CLI Thermostat Play requires the exact thermostat/main scan".into());
    }
    let activation = &plot.expanded.activations[0];
    if activation.selected_plot != "thermostat/transition" {
        return Err("CLI Thermostat Play requires thermostat/transition".into());
    }
    let conduit_plot::ActivationSyntax::Scan { maximum_items, .. } = &activation.mode else {
        return Err("CLI Thermostat Play requires scan".into());
    };
    let bytes = activation
        .initial_accumulator_bytes
        .as_deref()
        .ok_or("Thermostat initial Form is missing")?;
    let initial =
        ThermostatState::decode(bytes).map_err(|e| format!("Thermostat initial Form: {e:?}"))?;
    if initial.encode().map_err(|e| format!("{e:?}"))?.as_slice() != bytes {
        return Err("Thermostat initial Form is not canonical".into());
    }
    let host = StdHost::new_for_thermostat_scan(
        StdHostConfig {
            host_id: conduit_core::HostId::from("std/thermostat-cli"),
            boot_id: format!(
                "boot/thermostat-cli/{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos()
            )
            .into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &initial,
        *maximum_items,
    )?;
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&plot.expanded, &hosts)
        .map_err(|e| format!("{e:?}"))?;
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let boundaries = BTreeMap::from([
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: port_id("commands"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_thermostat_plot::COMMAND_BYTES as u32,
            },
        ),
        (
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: port_id("states"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_thermostat_plot::STATE_BYTES as u32,
            },
        ),
    ]);
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        &source.check()?,
        plot,
        source.authoring_catalog(),
        &conduit_plot::CanonicalBackCatalog::new(),
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|e| format!("{e:?}"))?;
    let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
        return Err("Thermostat Plan lost scan activation".into());
    };
    let child = scan
        .selected_plan
        .fragments
        .first()
        .filter(|_| scan.selected_plan.fragments.len() == 1)
        .and_then(|fragment| {
            fragment
                .placements
                .first()
                .filter(|_| fragment.placements.len() == 1)
        })
        .ok_or("Thermostat scan requires one exact child Gear")?;
    if child.kind_id.as_str() != conduit_thermostat_plot::THERMOSTAT_KIND {
        return Err("Thermostat child Kind is not exact".into());
    }
    Ok((host, plan))
}

#[derive(serde::Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
enum InputCommand {
    #[serde(rename = "set-target")]
    Target { target: i16 },
    #[serde(rename = "set-mode")]
    Mode { mode: String },
    #[serde(rename = "set-fan")]
    Fan { fan: String },
    #[serde(rename = "set-preset")]
    Preset { preset: String },
}
fn parse_command(line: &str) -> Result<Command, String> {
    Ok(
        match serde_json::from_str(line).map_err(|e| format!("Thermostat control: {e}"))? {
            InputCommand::Target { target } => Command::SetTarget(target),
            InputCommand::Mode { mode } => Command::SetMode(match mode.as_str() {
                "off" => Mode::Off,
                "heat" => Mode::Heat,
                "cool" => Mode::Cool,
                "auto" => Mode::Auto,
                _ => return Err("mode must be off, heat, cool, or auto".into()),
            }),
            InputCommand::Fan { fan } => Command::SetFan(match fan.as_str() {
                "auto" => Fan::Auto,
                "on" => Fan::On,
                _ => return Err("fan must be auto or on".into()),
            }),
            InputCommand::Preset { preset } => Command::SetPreset(match preset.as_str() {
                "comfort" => Preset::Comfort,
                "eco" => Preset::Eco,
                "sleep" => Preset::Sleep,
                _ => return Err("preset must be comfort, eco, or sleep".into()),
            }),
        },
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdin_control_ingress_stops_at_its_finite_line_bound() {
        let mut input = std::io::Cursor::new(vec![b'x'; 4096]);
        let mut buffer = Vec::with_capacity(1025);
        assert!(bounded_control_line(&mut input, &mut buffer).is_err());
        assert_eq!(input.position(), 1025);
        assert_eq!(buffer.len(), 1025);
        let mut input = std::io::Cursor::new(b"{}\n");
        assert_eq!(
            bounded_control_line(&mut input, &mut buffer).unwrap(),
            Some("{}\n")
        );
        assert_eq!(bounded_control_line(&mut input, &mut buffer).unwrap(), None);
    }
    #[test]
    fn ordinary_cli_thermostat_source_plans_with_exact_scoped_host() {
        let source =
            crate::plot_source::parse(include_str!("../../../plots/thermostat/main.conduit"))
                .unwrap();
        let plot = source.expand_entry_for_authoring().unwrap();
        let (_, plan) = prepare(&source, &plot).unwrap();
        assert!(conduit_core::verify_plan(&plan));
        assert_eq!(plan.activations.len(), 1);
        assert_eq!(
            parse_command(r#"{"command":"set-mode","mode":"heat"}"#).unwrap(),
            Command::SetMode(Mode::Heat)
        );
        assert!(parse_command(r#"{"command":"set-mode","mode":"invalid"}"#).is_err());
    }
}
