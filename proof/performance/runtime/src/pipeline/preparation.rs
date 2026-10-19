//! Checked Source, exact Host offers, Planner selection and finite fixture inputs.
use super::*;

pub(super) fn prepare(gears: usize, source: &str, boot: &str) -> Result<Prepared, Box<dyn Error>> {
    prepare_profile(gears, source, boot, false)
}
pub(super) fn prepare_profile(
    gears: usize,
    source: &str,
    boot: &str,
    local: bool,
) -> Result<Prepared, Box<dyn Error>> {
    prepare_mode(gears, source, boot, local, false)
}
pub(super) fn prepare_mode(
    gears: usize,
    source: &str,
    boot: &str,
    local: bool,
    fused: bool,
) -> Result<Prepared, Box<dyn Error>> {
    prepare_queue_mode(
        gears,
        source,
        boot,
        if local {
            BackProfile::Local
        } else {
            BackProfile::Hosted
        },
        fused,
        1,
        8,
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_queue_mode(
    gears: usize,
    source: &str,
    boot: &str,
    back: BackProfile,
    fused: bool,
    item_capacity: u16,
    byte_capacity: u32,
) -> Result<Prepared, Box<dyn Error>> {
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
            let offer = if matches!(back, BackProfile::ScalarTape) {
                conduit_std_offers::pure_scalar_tape_std_offer(&program, gear.inputs[0].temporal)?
            } else {
                let local = match back {
                    BackProfile::Local => true,
                    #[cfg(test)]
                    BackProfile::NativeLocal => true,
                    _ => false,
                };
                let offer = if local {
                    conduit_std_offers::pure_expression_local_flow_offer(&program)
                } else {
                    conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                };
                offer.map_err(|e| format!("offer: {e:?}"))?
            };
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
    let boundaries = authoring
        .input_bindings
        .iter()
        .map(|binding| (PortDirection::Input, binding))
        .chain(
            authoring
                .output_bindings
                .iter()
                .map(|binding| (PortDirection::Output, binding)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity,
                    byte_capacity,
                },
            )
        })
        .collect();
    let (selection, m) = timing::measure(|| -> Result<_, Box<dyn Error>> {
        if fused && gears > 1 {
            #[cfg(test)]
            if matches!(back, BackProfile::NativeLocal) {
                return Ok(Some(fusion::select_profile(
                    &authoring,
                    &hosts,
                    &placements,
                    1,
                    &[],
                    true,
                )?));
            }
            Ok(Some(fusion::select(
                &authoring,
                &hosts,
                &placements,
                1,
                &[],
            )?))
        } else {
            Ok(None)
        }
    });
    phases.push(("physical-fusion-selection", m));
    let selection = selection?;
    let (plan, m) = timing::measure(|| {
        let options = PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: item_capacity,
            connection_byte_capacity: byte_capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        };
        if let Some(selection) = &selection {
            conduit_planner::plan_expanded_selected_optimization(
                &authoring,
                &hosts,
                selection,
                &[BaseImplementationId::from("conduit.base/local@1")],
                options,
                &boundaries,
            )
            .map(|optimized| optimized.plan)
        } else {
            plan_expanded_authoring_with_options(
                &authoring,
                &hosts,
                &placements,
                &[BaseImplementationId::from("conduit.base/local@1")],
                options,
                &boundaries,
            )
        }
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
        selection_evidence: selection.as_ref().map(|selection| json!({
            "selected_candidate":selection.selected_candidate_id,
            "cost_basis":"frozen useful-addition and dispatch work assumptions; timing and transport accounting are separate",
            "candidates":selection.considered.iter().map(|c| json!({"candidate":c.candidate_id,"disposition":format!("{:?}",c.disposition),"compute_work":c.compute_work_units,"transport_work":c.transport_work_units,"total_work":c.total_work_units})).collect::<Vec<_>>(),
        })),
    })
}
