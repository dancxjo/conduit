//! Exact preparation shared by the stdin Fore and complete-owner observation.
use crate::plot_source::CanonicalSource;
use conduit_core::*;
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::collections::BTreeMap;

pub(crate) fn prepare(
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
