//! Reviewed std-only physical dispatch of 2..=32 exact local expression Gears.
//! Every logical Cord is materialized and every logical Step retains its Sign.
use conduit_core::{ConfigurationValue, HostAdvertisement, Kind};
use conduit_planner::FusionRealizationOffer;
use conduit_plot::{ExpandedAuthoringPlot, PortableExpressionProgram};

pub const PURE_CHAIN_FUSION_PROFILE: &str = "std/pure-chain-dispatch@1";
pub const PURE_CHAIN_FUSION_IMPLEMENTATION: &str = "std/kernel-pure-chain-dispatch@1";
pub const PURE_CHAIN_FUSION_ARTIFACT: &str = "conduit-kernel/logical-step-chain@1";

/// An immutable current-Boot alternative. The Planner still owns selection and
/// required observation/authority refusals. Unsupported targets have no such Back.
pub fn pure_chain_fusion_offer(
    authoring: &ExpandedAuthoringPlot,
    host: &HostAdvertisement,
) -> Result<(FusionRealizationOffer, Vec<Kind>), String> {
    let plot = &authoring.expanded;
    plot.validate_expansion().map_err(|e| e.to_string())?;
    if !(2..=32).contains(&plot.gears.len())
        || plot.connections.len() + 1 != plot.gears.len()
        || !plot.activations.is_empty()
        || !plot.shared_pools.is_empty()
    {
        return Err("pure-chain fusion requires one finite 2..=32-Gear chain".into());
    }
    let mut kinds = Vec::with_capacity(plot.gears.len());
    for gear in &plot.gears {
        let [entry] = gear.configuration.as_slice() else {
            return Err("pure-chain program absent".into());
        };
        let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value)
        else {
            return Err("pure-chain program malformed".into());
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded)
            .map_err(|e| format!("pure-chain program: {e:?}"))?;
        let expected = match gear.inputs.first().map(|p| p.temporal) {
            Some(conduit_core::PortTemporal::Flow { closes: true }) => {
                conduit_std_offers::pure_expression_local_flow_offer(&program)
            }
            Some(conduit_core::PortTemporal::Value) => {
                conduit_std_offers::pure_expression_local_offer(&program)
            }
            _ => return Err("pure-chain requires Value or closing Flow".into()),
        }
        .map_err(|e| format!("pure-chain Offer: {e:?}"))?;
        if !gear.accepts_realization(&expected) || !host.capabilities.iter().any(|c| c == &expected)
        {
            return Err("pure-chain requires the exact installed computational Offer".into());
        }
        let kind = validate_program_profile(&program, gear.inputs[0].temporal)?;
        kinds.push(kind);
    }
    // Recover order from authored Cords, never lexicographic Gear/Placement IDs.
    let mut ordered = Vec::with_capacity(plot.gears.len());
    let starts: Vec<_> = plot
        .gears
        .iter()
        .filter(|gear| {
            !plot
                .connections
                .iter()
                .any(|c| c.sink_gear_id == gear.gear_id)
        })
        .collect();
    if starts.len() != 1 {
        return Err("pure-chain has no unique entry".into());
    }
    let mut current = &starts[0].gear_id;
    loop {
        if ordered.contains(current) {
            return Err("pure-chain cycle".into());
        }
        ordered.push(current.clone());
        let next: Vec<_> = plot
            .connections
            .iter()
            .filter(|c| &c.source_gear_id == current)
            .collect();
        if next.is_empty() {
            break;
        }
        if next.len() != 1
            || plot
                .connections
                .iter()
                .filter(|c| c.sink_gear_id == next[0].sink_gear_id)
                .count()
                != 1
        {
            return Err("pure-chain fan-out or merge boundary".into());
        }
        current = &next[0].sink_gear_id;
    }
    if ordered.len() != plot.gears.len() {
        return Err("pure-chain disconnected graph".into());
    }
    if authoring.input_bindings.len() != 1
        || authoring.output_bindings.len() != 1
        || authoring.input_bindings[0].gear_id != ordered[0]
        || Some(&authoring.output_bindings[0].gear_id) != ordered.last()
        || authoring.input_bindings[0].track != conduit_core::ConnectionTrack::Payload
        || authoring.output_bindings[0].track != conduit_core::ConnectionTrack::Payload
    {
        return Err("pure-chain fusion forbids independent Fore or observation boundaries inside the region".into());
    }
    Ok((
        FusionRealizationOffer {
            fusion_id: format!("std/pure-chain/{}", plot.expanded_plot_id.as_str()),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            execution_profile_id: PURE_CHAIN_FUSION_PROFILE.into(),
            implementation_id: PURE_CHAIN_FUSION_IMPLEMENTATION.into(),
            artifact_id: PURE_CHAIN_FUSION_ARTIFACT.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            gear_ids: ordered,
            internal_cords: plot
                .connections
                .iter()
                .map(|c| (c.source_gear_id.clone(), c.sink_gear_id.clone()))
                .collect(),
            preserves_typed_ports: true,
            preserves_atomic_pressure: true,
            preserves_cancellation: true,
            preserves_required_evidence: true,
        },
        kinds,
    ))
}

/// The first reviewed profile is an exact U64-to-U64 map, not a blanket
/// endorsement of every expression type or numeric representation.
pub(crate) fn validate_program_profile(
    program: &PortableExpressionProgram,
    temporal: conduit_core::PortTemporal,
) -> Result<Kind, String> {
    let u64_type = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))
        .map_err(|e| format!("pure-chain type: {e:?}"))?;
    if program.input_type != u64_type || program.output_type != u64_type {
        return Err("std pure-chain fusion supports exact U64 input/output only".into());
    }
    let kind = conduit_semantic_catalog::pure_expression_contract(program, temporal)
        .map_err(|e| format!("pure-chain Kind: {e:?}"))?;
    let back = conduit_core::Back {
        capability_id: "std/pure-chain-proof".into(),
        execution_profile_id: PURE_CHAIN_FUSION_PROFILE.into(),
        implementation_id: PURE_CHAIN_FUSION_IMPLEMENTATION.into(),
        artifact_id: PURE_CHAIN_FUSION_ARTIFACT.into(),
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
    };
    conduit_core::derive_transformation_eligibility(&kind, &back)
        .map_err(|e| format!("pure-chain eligibility: {e:?}"))?
        .require(conduit_core::WorkTransformation::Fusion)
        .map_err(|e| format!("pure-chain refusal: {e:?}"))?;
    Ok(kind)
}
