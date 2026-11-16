//! Closed native graphical preparation from an original admitted IMAGE template.
//! This selects immutable metadata, not reservations, provider possession or Start.
use conduit_conduitos_mask_offer::CanonicalNativeGraphicalTemplate;
use conduit_core::*;
use conduit_face::PlannedMaskPlot;
use conduit_std_host::body_execution::{ImagePreparedValue, ImageRootPreparationOwner};
use std::mem::size_of;

#[path = "graphical_template/preflight.rs"]
mod preflight;
use preflight::{hex, line_index, preflight};

struct Checked<'a> {
    bases: [Option<&'a BaseProviderAdvertisement>; 4],
    lines: [&'a LineOffer; 3],
    retained: usize,
    peak: usize,
}

/// Reuses only the canonical four-Gear graphical closure. Unsupported provider,
/// resource or Source shapes refuse; no parser or general planner runs here.
#[allow(clippy::too_many_arguments)]
pub(super) fn instantiate(
    memory: &ImageRootPreparationOwner,
    template: &ImagePreparedValue<CanonicalNativeGraphicalTemplate>,
    target_actual: &HostAdvertisement,
    owner_actual: &HostAdvertisement,
    face_line: &LineOffer,
    return_line: &LineOffer,
    interaction_line: &LineOffer,
) -> Result<ImagePreparedValue<PlannedMaskPlot>, String> {
    memory.verify_prepared(template)?;
    let canonical = template.value().planned_mask();
    let checked = preflight(
        canonical,
        target_actual,
        owner_actual,
        [face_line, return_line, interaction_line],
    )
    .ok_or("native-template-preflight-refused")?;
    // SAFETY: preflight visits actual owned template capacities and every new
    // string/Line owner before allocation. Its closed peak includes working clone, ordinary admission's second clone,
    // identity remapping and verification scratch. Borrowed inputs and the
    // original template remain independently owned throughout this synchronous
    // callback; no workers, media IO, mutable backing or escaped references exist.
    unsafe {
        memory.prepare_value(checked.peak, checked.retained, || {
            let result = build(canonical, &checked, target_actual)?;
            // Defense in depth only; the preflight is the admission proof.
            let actual = conduit_std_host::native_mask_plan_backing_bytes(&result)
                .and_then(|n| n.checked_add(size_of::<PlannedMaskPlot>()))
                .ok_or("native-template-result-shape-refused")?;
            if actual > checked.retained {
                return Err("native-template-bound-exceeded".into());
            }
            Ok(result)
        })
    }
}

fn build(
    template: &PlannedMaskPlot,
    checked: &Checked<'_>,
    target_actual: &HostAdvertisement,
) -> Result<PlannedMaskPlot, String> {
    let mut planned = template.clone();
    let f = &mut planned.plan.fragments[0];
    let old = &template.plan.fragments[0];
    let ids: [PlacementId; 4] = std::array::from_fn(|i| {
        let g = &old.placements[i];
        conduit_planner::canonical_placement_id(
            &planned.plan.checked_plot_id,
            &g.gear_id,
            &target_actual.host_id,
            &g.capability_id,
        )
    });
    for (i, g) in f.placements.iter_mut().enumerate() {
        g.placement_id = ids[i].clone();
        g.host_id = target_actual.host_id.clone();
        g.boot_id = target_actual.boot_id.clone();
        g.offer_generation = target_actual.offer_generation;
        g.base = checked.bases[i].map(BaseProviderAdvertisement::binding);
        for r in &mut g.resources {
            r.pool_id = target_actual
                .resources
                .iter()
                .find(|v| v.class_id == r.class_id)
                .expect("preflight unique resource class")
                .pool_id
                .clone();
        }
        g.resources.sort();
        if !g.authority.is_empty() {
            let grant = conduit_conduitos_mask_offer::native_mask_present_grant(
                &target_actual.host_id,
                &target_actual.boot_id,
            );
            g.authority[0] = AuthorityBinding {
                grant_id: grant.grant_id,
                contract_id: grant.contract_id,
                host_call_contract_id: grant.host_call_contract_id,
                subject_kind: grant.subject_kind,
                host_id: grant.host_id,
                boot_id: grant.boot_id,
                capability_id: grant.capability_id,
            };
        }
    }
    let remap = |id: &mut PlacementId| {
        let i = old
            .placements
            .iter()
            .position(|g| g.placement_id == *id)
            .expect("preflight closed reference");
        *id = ids[i].clone();
    };
    for c in &mut f.connections {
        remap(&mut c.source_placement_id);
        remap(&mut c.sink_placement_id);
        if let Some(r) = &mut c.resource {
            remap(&mut r.owner_placement_id);
            r.source_binding.pool_id = target_actual
                .resources
                .iter()
                .find(|v| v.class_id == r.source_binding.class_id)
                .expect("preflight unique resource class")
                .pool_id
                .clone();
        }
    }
    for fore in &mut f.fore_ports {
        remap(&mut fore.placement_id);
        let i = line_index(&planned.mask, fore).expect("preflight three exact Fores");
        fore.selected_line = Some(checked.lines[i].admitted_line());
    }
    for d in &mut f.startup_dependencies {
        remap(&mut d.prerequisite_placement_id);
        remap(&mut d.dependent_placement_id);
    }
    f.startup_dependencies.sort();
    let order = conduit_planner::fixed_stateless_startup_order(&f.placements, &f.connections)
        .ok_or("native-template-startup-refused")?;
    for (slot, i) in f.startup_order.iter_mut().zip(order) {
        *slot = ids[i].clone();
    }
    for t in &mut f.expected_terminals {
        if let ExpectedTerminal::PlacementCompleted(id) = t {
            remap(id);
        }
    }
    for s in &mut f.expected_sign {
        match s {
            ExpectedSign::PlacementPrepared(id) | ExpectedSign::PlacementTerminal(id) => remap(id),
            _ => {}
        }
    }
    f.host_id = target_actual.host_id.clone();
    f.boot_id = target_actual.boot_id.clone();
    f.offer_generation = target_actual.offer_generation;
    f.fragment_id = compute_fragment_id(f);
    let digest = PlainPlotPlanSeal::capture(&planned.plan)
        .and_then(|p| {
            p.digest(&[PlotFragmentSeal {
                host_id: target_actual.host_id.as_str(),
                fragment_id: planned.plan.fragments[0].fragment_id.as_str(),
            }])
        })
        .map_err(|_| "native-template-seal-refused")?;
    let plan_id = PlanId::from(hex(digest));
    planned.plan.plan_id = plan_id.clone();
    let f = &mut planned.plan.fragments[0];
    f.plan_id = plan_id;
    f.plan_fragments[0] = FragmentCommitment {
        host_id: target_actual.host_id.clone(),
        fragment_id: f.fragment_id.clone(),
    };
    let result = PlannedMaskPlot::admit(&planned.mask, &planned.plan)
        .map_err(|_| "native-template-admit-refused".to_string())?;
    Ok(result)
}

#[cfg(test)]
#[path = "graphical_template/tests.rs"]
mod tests;
