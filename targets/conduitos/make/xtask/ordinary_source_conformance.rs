//! Compare the sealed ordinary specimen's meaning without conflating Host IDs.
use super::ConduitosError;
use conduit_core::{ExpectedSign, ExpectedTerminal, Plan};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn capture(plan: &Plan) -> Result<Value, ConduitosError> {
    let shape = semantic_shape(plan)?;
    if shape != semantic_shape(&reference_plan()?)? {
        return Err(refusal(
            "sealed ordinary Plan differs from the exact checked Source specimen",
        ));
    }
    let fragment = &plan.fragments[0];
    if fragment.execution_regions.iter().any(|region| {
        let text = region.region_id.as_str() == "region/text";
        region.preemption_required != text
            || region.isolation_required != text
            || region.execution_profile_id.as_str()
                != if text {
                    conduitos::ordinary_plan::PROTECTED_REGION_PROFILE
                } else {
                    conduitos::ordinary_plan::COOPERATIVE_REGION_PROFILE
                }
    }) {
        return Err(refusal(
            "ordinary region protection disposition disagrees with the admitted specimen",
        ));
    }
    let bytes = serde_json::to_vec(&shape).map_err(|error| refusal(error.to_string()))?;
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(
        json!({"schema":"conduit.conduitos/ordinary-source-plan-conformance@1",
        "source_document_id":plan.source_document_id,"checked_plot_id":plan.checked_plot_id,
        "expanded_plot_id":plan.expanded_plot_id,"plan_id":plan.plan_id,
        "semantic_shape_sha256":digest,"placements":5,"cords":3,"regions":2,
        "protected_text_region":true,"proof_class":"sealed-plan-conformance"}),
    )
}

fn reference_plan() -> Result<Plan, ConduitosError> {
    let identities = conduitos::identity::BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = conduitos::offer::HostOffer::new(
        &identities,
        "conformance-reference",
        conduitos::offer::CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        512 * 1024,
    );
    conduitos::dual_region_plan::prepare(&identities, &offer, "conformance-reference")
        .map(|prepared| prepared.plan)
        .map_err(|error| refusal(error.as_str()))
}

fn semantic_shape(plan: &Plan) -> Result<Value, ConduitosError> {
    if !conduit_core::verify_plan(plan)
        || plan.fragments.len() != 1
        || !plan.realization_backs.is_empty()
        || !plan.activations.is_empty()
        || !plan.activation_preparations.is_empty()
    {
        return Err(refusal("one sealed primitive ordinary fragment required"));
    }
    let fragment = &plan.fragments[0];
    if fragment.placements.len() != 5
        || fragment.connections.len() != 3
        || fragment.execution_regions.len() != 2
        || !fragment.states.is_empty()
        || !fragment.fore_ports.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.realization_backs.is_empty()
    {
        return Err(refusal(
            "ordinary specimen topology or extension set changed",
        ));
    }
    let gears = fragment
        .placements
        .iter()
        .map(|placement| (placement.placement_id.as_str(), placement.gear_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    if gears.len() != 5 || gears.values().copied().collect::<BTreeSet<_>>().len() != 5 {
        return Err(refusal(
            "ordinary placement/Gear correspondence is not bijective",
        ));
    }
    let gear = |id: &str| {
        gears
            .get(id)
            .copied()
            .ok_or_else(|| refusal("reference to an absent ordinary placement"))
    };
    let startup = fragment
        .startup_order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect::<BTreeMap<_, _>>();
    if fragment.startup_order.len() != gears.len()
        || startup.len() != gears.len()
        || startup.keys().any(|id| !gears.contains_key(id))
    {
        return Err(refusal(
            "ordinary startup order is not a placement permutation",
        ));
    }
    let mut dependencies = Vec::new();
    for dependency in &fragment.startup_dependencies {
        let prerequisite = dependency.prerequisite_placement_id.as_str();
        let dependent = dependency.dependent_placement_id.as_str();
        if startup
            .get(prerequisite)
            .zip(startup.get(dependent))
            .is_none_or(|(a, b)| a >= b)
        {
            return Err(refusal(
                "ordinary startup order violates the sealed dependency graph",
            ));
        }
        dependencies.push(json!([gear(prerequisite)?, gear(dependent)?]));
    }
    sort(&mut dependencies);
    if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(refusal("duplicate ordinary startup dependency"));
    }
    let mut placements = Vec::new();
    for placement in &fragment.placements {
        if !placement.authority.is_empty() || !placement.pool_references.is_empty() {
            return Err(refusal(
                "ordinary specimen acquired authority or shared-pool semantics",
            ));
        }
        // Keep every semantic/configuration/Fore field. Only the reviewed
        // selected-Back facts differ across machine realizations. This is an
        // inspection projection, never a substitute executable Plan.
        let mut value =
            serde_json::to_value(placement).map_err(|error| refusal(error.to_string()))?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| refusal("malformed placement projection"))?;
        for field in [
            "placement_id",
            "execution_profile_id",
            "host_id",
            "boot_id",
            "offer_generation",
            "capability_id",
            "implementation_id",
            "artifact_id",
            "base",
            "resources",
            "realization_characteristics",
            "realization_properties",
        ] {
            object.remove(field);
        }
        placements.push(value);
    }
    placements.sort_by(|a, b| a["gear_id"].as_str().cmp(&b["gear_id"].as_str()));
    let mut cords = Vec::new();
    for cord in &fragment.connections {
        if cord.resource.is_some()
            || cord.selected_line.is_some()
            || !cord.admitted_lines.is_empty()
        {
            return Err(refusal(
                "ordinary specimen acquired resource/Line semantics",
            ));
        }
        let mut value = serde_json::to_value(cord).map_err(|error| refusal(error.to_string()))?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| refusal("malformed Cord projection"))?;
        object.remove("source_placement_id");
        object.remove("sink_placement_id");
        object.insert(
            "source_gear_id".into(),
            json!(gear(cord.source_placement_id.as_str())?),
        );
        object.insert(
            "sink_gear_id".into(),
            json!(gear(cord.sink_placement_id.as_str())?),
        );
        cords.push(value);
    }
    cords.sort_by(|a, b| {
        a["connection_id"]
            .as_str()
            .cmp(&b["connection_id"].as_str())
    });
    let mut regions = Vec::new();
    for region in &fragment.execution_regions {
        let mut members = region
            .admitted_placements
            .iter()
            .map(|id| gear(id.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        members.sort();
        let mut requirements = serde_json::to_value(region.requirements)
            .map_err(|error| refusal(error.to_string()))?;
        requirements
            .as_object_mut()
            .ok_or_else(|| refusal("malformed region requirements"))?
            .remove("runtime_memory_bytes");
        regions.push(
            json!({"region_id":region.region_id,"admitted_gears":members,
            "scheduling":region.scheduling,"lane_count":region.lane_count,
            "requirements":requirements}),
        );
    }
    regions.sort_by(|a, b| a["region_id"].as_str().cmp(&b["region_id"].as_str()));
    let mut terminals = fragment
        .expected_terminals
        .iter()
        .map(|terminal| {
            Ok(match terminal {
                ExpectedTerminal::PlacementCompleted(id) => {
                    json!(["placement-completed", gear(id.as_str())?])
                }
                ExpectedTerminal::ConnectionCompleted(id) => json!(["connection-completed", id]),
                ExpectedTerminal::PlanCompleted => json!(["plan-completed"]),
            })
        })
        .collect::<Result<Vec<_>, ConduitosError>>()?;
    sort(&mut terminals);
    let mut signs = fragment
        .expected_sign
        .iter()
        .map(|sign| {
            Ok(match sign {
                ExpectedSign::PlanFragmentReceived => json!(["fragment-received"]),
                ExpectedSign::PlacementPrepared(id) => {
                    json!(["placement-prepared", gear(id.as_str())?])
                }
                ExpectedSign::PlacementTerminal(id) => {
                    json!(["placement-terminal", gear(id.as_str())?])
                }
                ExpectedSign::ConnectionTerminal(id) => json!(["connection-terminal", id]),
                ExpectedSign::PlanTerminal => json!(["plan-terminal"]),
            })
        })
        .collect::<Result<Vec<_>, ConduitosError>>()?;
    sort(&mut signs);
    let mut shape = json!({"source_document_id":plan.source_document_id,"checked_plot_id":plan.checked_plot_id,
        "expanded_plot_id":plan.expanded_plot_id,"completion_policy":plan.completion_policy,
        "placements":placements,"cords":cords,"regions":regions,
        "startup_dependencies":dependencies,"cancellation_policy":fragment.cancellation_policy,
        "terminal_policy":fragment.terminal_policy,"expected_terminals":terminals,
        "expected_signs":signs,"sign_storage_budget":fragment.sign_storage_budget});
    canonical_keys(&mut shape);
    Ok(shape)
}
fn canonical_keys(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.sort_keys();
            for child in object.values_mut() {
                canonical_keys(child);
            }
        }
        Value::Array(array) => {
            for child in array {
                canonical_keys(child);
            }
        }
        _ => {}
    }
}
fn sort(values: &mut [Value]) {
    values.sort_by_cached_key(Value::to_string);
}
fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("ordinary-source-plan-conformance-invalid", detail)
}
#[cfg(test)]
mod tests;
