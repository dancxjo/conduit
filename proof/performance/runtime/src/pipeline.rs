//! Actual checked Plot/Plan/std Back/kernel execution with acknowledged Fores.
//! This finite 64-item trial is not claimed as a million-item steady Play.
use crate::{artifact, config::Config, timing};
use conduit_core::{BaseImplementationId, ConnectionTrack, Plan, PortDirection};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost,
    StdHostComposition, StdHostConfig, TimerAdapter,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    error::Error,
    io::Write,
    time::{Duration, Instant},
};

const ITEMS: usize = 64;
const ADDS: usize = 32;
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {}
}
struct Collector {
    values: Vec<ExternalForeDelivery>,
    first: Option<Instant>,
    last: Option<Instant>,
}
impl Collector {
    fn new() -> Self {
        Self {
            values: Vec::with_capacity(ITEMS),
            first: None,
            last: None,
        }
    }
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if self.values.len() >= ITEMS {
            return Err("acknowledged Fore capacity exceeded".into());
        }
        if self.values.is_empty() {
            self.first = Some(Instant::now());
        }
        if self.values.len() == ITEMS - 1 {
            self.last = Some(Instant::now());
        }
        self.values.push(value);
        Ok(())
    }
}
struct Prepared {
    host: StdHost,
    plan: Plan,
    inputs: Vec<ExternalForeInput>,
    phases: Vec<(&'static str, timing::Metrics)>,
}
fn source(gears: usize) -> String {
    let expression = format!(".{}", " + 1".repeat(ADDS / gears));
    format!(
        "plot baseline (\n >> input: U64...|\n output: U64...| >>\n) {{\n input{} >> output\n}}\n",
        format!(" >> ({expression})").repeat(gears)
    )
}
fn prepare(gears: usize, source: &str, boot: &str) -> Result<Prepared, Box<dyn Error>> {
    let mut phases = Vec::with_capacity(6);
    let (syntax, m) = timing::measure(|| parse_syntax_document(source));
    phases.push(("parse", m));
    if !syntax.diagnostics.is_empty() {
        return Err(format!("syntax: {:?}", syntax.diagnostics).into());
    }
    let (checked, m) = timing::measure(|| check_syntax_document(&syntax, &StartupCatalog::new()));
    phases.push(("check", m));
    let checked = checked.map_err(|e| format!("check: {e:?}"))?;
    let (authoring, m) = timing::measure(|| {
        expand_canonical_plot_for_authoring(&checked, "baseline", &ProfileCatalog::new())
    });
    phases.push(("expand", m));
    let authoring = authoring.map_err(|e| format!("expand: {e:?}"))?;
    if authoring.expanded.gears.len() != gears {
        return Err("expanded Gear count differs".into());
    }
    let (host, m) = timing::measure(|| -> Result<StdHost, String> {
        let base = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "host/runtime-performance".into(),
                boot_id: boot.into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal(),
        );
        let mut advertisement = base.advertisement().clone();
        for gear in &authoring.expanded.gears {
            let conduit_core::ConfigurationValue::Text(program) = &gear.configuration[0].value
            else {
                return Err("ordinary expression configuration missing".into());
            };
            let program = PortableExpressionProgram::from_canonical_hex(program)
                .map_err(|e| format!("program: {e:?}"))?;
            let offer =
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .map_err(|e| format!("offer: {e:?}"))?;
            if !gear.accepts_realization(&offer) {
                return Err("std expression offer violates checked meaning".into());
            }
            if !advertisement
                .capabilities
                .iter()
                .any(|e| e.capability_id == offer.capability_id)
            {
                advertisement.capabilities.push(offer);
            }
        }
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        StdHost::from_advertisement(advertisement)
    });
    phases.push(("host-offer-preparation", m));
    let host = host?;
    let hosts = [host.advertisement().clone()];
    let (placements, m) =
        timing::measure(|| default_expanded_placements(&authoring.expanded, &hosts));
    phases.push(("realization-selection", m));
    let placements = placements.map_err(|e| format!("selection: {e:?}"))?;
    let boundaries = [
        (PortDirection::Input, "input"),
        (PortDirection::Output, "output"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 8,
            },
        )
    })
    .collect();
    let (plan, m) = timing::measure(|| {
        plan_expanded_authoring_with_options(
            &authoring,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 8,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
    });
    phases.push(("plan-realization-validation-and-sealing", m));
    let plan = plan.map_err(|e| format!("Plan: {e:?}"))?;
    if !conduit_core::verify_plan(&plan) {
        return Err("Plan seal invalid".into());
    }
    let inputs = (0..ITEMS as u64)
        .map(|i| ExternalForeInput {
            front_port_id: "input".into(),
            track: ConnectionTrack::Payload,
            bytes: i.to_le_bytes().to_vec(),
        })
        .collect();
    Ok(Prepared {
        host,
        plan,
        inputs,
        phases,
    })
}
fn validate(values: &[ExternalForeDelivery]) -> Result<String, Box<dyn Error>> {
    if values.len() != ITEMS {
        return Err("pipeline lost or duplicated an output".into());
    }
    let mut ordered = [0_u8; ITEMS * 8];
    for (index, value) in values.iter().enumerate() {
        if value.sequence != index as u64
            || value.front_port_id.as_str() != "output"
            || value.track != ConnectionTrack::Payload
            || value.value_kind != conduit_core::kind_id("value/u64")
            || value.bytes != (index as u64 + ADDS as u64).to_le_bytes()
        {
            return Err("pipeline output violates ordered native meaning".into());
        }
        ordered[index * 8..index * 8 + 8].copy_from_slice(&value.bytes);
    }
    Ok(artifact::digest(&ordered))
}
fn native(inputs: &[ExternalForeInput], outputs: &mut Vec<[u8; 8]>) -> Result<(), String> {
    for input in inputs {
        let input = u64::from_le_bytes(
            input
                .bytes
                .as_slice()
                .try_into()
                .map_err(|_| "native canonical input width")?,
        );
        let mut value = std::hint::black_box(input);
        for _ in 0..ADDS {
            value = value.checked_add(1).ok_or("native overflow")?;
        }
        outputs.push(value.to_le_bytes());
    }
    Ok(())
}
pub fn run(
    config: &Config,
    raw: &mut artifact::Raw,
    summaries: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    let run_identity = artifact::digest(config.output.to_string_lossy().as_bytes());
    for gears in [1, 8, 32] {
        let id = format!("hosted-pipeline-{gears}-gears-{ADDS}-adds");
        let source = source(gears);
        artifact::new_file(&config.output.join("sources").join(format!("{id}.plot")))?
            .write_all(source.as_bytes())?;
        for repetition in 0..config.repetitions {
            // Each newly constructed Host is a distinct Boot; repeated trials
            // must not reset the Play sequence under a reused Boot identity.
            let boot = format!("boot/runtime-performance/{run_identity}/{gears}/{repetition}");
            let mut prepared = prepare(gears, &source, &boot)?;
            artifact::json(
                &config
                    .output
                    .join("sources")
                    .join(format!("{id}-{repetition}-plan.json")),
                &prepared.plan,
            )?;
            artifact::json(
                &config
                    .output
                    .join("sources")
                    .join(format!("{id}-{repetition}-host.json")),
                prepared.host.advertisement(),
            )?;
            raw.row(&json!({"case":id,"phase":"cold-plot-plan","repetition":repetition,"source_sha256":artifact::digest(source.as_bytes()),"phases":prepared.phases}))?;
            let mut collector = Collector::new();
            let mut native_outputs = Vec::with_capacity(ITEMS);
            // Alternate native/production order without measuring output validation or file I/O.
            let native_first = repetition % 2 == 0;
            let mut native_metrics = None;
            if native_first {
                let (result, m) = timing::measure(|| native(&prepared.inputs, &mut native_outputs));
                result?;
                native_metrics = Some(m);
            }
            let fragment = prepared.plan.fragments[0].clone();
            let started = Instant::now();
            let (report, full_call) = timing::measure(|| {
                prepared.host.run_external_plot_sequence_to(
                    fragment,
                    &prepared.inputs,
                    &mut collector,
                    &mut std::io::sink(),
                    &mut NoTimer,
                )
            });
            let mut report = report?;
            if !native_first {
                let (result, m) = timing::measure(|| native(&prepared.inputs, &mut native_outputs));
                result?;
                native_metrics = Some(m);
            }
            let digest = validate(&collector.values)?;
            if report.external_fore_deliveries != collector.values {
                return Err("retained Fore receipt differs from acknowledgements".into());
            }
            let native_bytes = native_outputs.iter().flatten().copied().collect::<Vec<_>>();
            if artifact::digest(&native_bytes) != digest {
                return Err("native and production ordered outputs differ".into());
            }
            let kernel = report
                .kernel
                .take()
                .ok_or("production kernel receipt absent")?;
            let play = &kernel.active_play;
            if play.plan_id != prepared.plan.plan_id
                || play.boot_id.as_str() != boot
                || kernel.active_play_id != play.active_play_id
                || *play
                    != conduit_core::bind_active_play(
                        &play.plan_id,
                        &play.host_id,
                        &play.boot_id,
                        play.play_sequence,
                    )
            {
                return Err("kernel receipt lost exact Plan/Boot/Play custody".into());
            }
            for observation in &report.observations {
                if observation.host_id != play.host_id
                    || observation.boot_id != play.boot_id
                    || observation
                        .plan_id
                        .as_ref()
                        .is_some_and(|p| p != &play.plan_id)
                    || observation
                        .active_play_id
                        .as_ref()
                        .is_some_and(|p| p != &play.active_play_id)
                {
                    return Err("retained observation has foreign execution custody".into());
                }
            }
            if kernel.value_allocation_capacity_before != kernel.value_allocation_capacity_after {
                return Err("value storage grew during Play".into());
            }
            let signs = kernel.kernel_sign.iter().map(|e| json!({"sequence":e.sequence,"node":e.node.0,"port":e.port.map(|p|p.0),"request":e.request.map(|r|r.0),"kind":format!("{:?}",e.kind)})).collect::<Vec<_>>();
            if kernel
                .kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count()
                != ITEMS
            {
                return Err("mandatory output Signs missing".into());
            }
            let first = collector.first.ok_or("first acknowledgement absent")?;
            let last = collector.last.ok_or("last acknowledgement absent")?;
            let row = json!({"case":id,"phase":"finite-ordinary-hosted-pipeline","repetition":repetition,"gears":gears,"values":ITEMS,"adds_per_value":ADDS,
                "source_sha256":artifact::digest(source.as_bytes()),"ordered_output_sha256":digest,"full_host_call":full_call,"native_canonical_loop":native_metrics,
                "first_acknowledgement_from_call_start_ns":timing::ns(first.duration_since(started)),"first_to_last_acknowledgement_ns":timing::ns(last.duration_since(first)),"completion_window_items":ITEMS-1,
                "measurement_scope":"full call includes reservation, Back preparation, Play, Sign/report translation and teardown; completion window spans 63 additional acknowledged outputs; two callback wall samples retained; native uses identical canonical inputs/outputs with checked adds and legal compiler fusion",
                "kernel_decisions":kernel.decisions,"retained_kernel_events":kernel.kernel_events,"active_play":play,"plan_id":prepared.plan.plan_id.as_str(),"signs":signs,"observations":report.observations,
                "value_capacity_before":kernel.value_allocation_capacity_before,"value_capacity_after":kernel.value_allocation_capacity_after});
            raw.row(&row)?;
            summaries.push(json!({"case":id,"repetition":repetition,"values":ITEMS,"gears":gears,"full_host_call":full_call,"ordered_output_sha256":digest}));
            let (_, m) = timing::measure(|| {
                drop(collector);
                drop(prepared);
                drop(native_outputs);
                drop(kernel);
                drop(report);
            });
            raw.row(&json!({"case":id,"phase":"host-and-harness-teardown","repetition":repetition,"metrics":m}))?;
            eprintln!("validated {id} repetition {}", repetition + 1);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "pipeline/tests.rs"]
mod tests;
