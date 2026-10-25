//! Shared finite Mask topology validation over rich or Core-verified metadata.
use super::*;
pub(super) fn validate_mask_views<'a>(
    workset: &BodyWorkset,
    plots: impl Iterator<Item = views::PlotRef<'a>> + Clone,
    topologies: impl ExactSizeIterator<Item = views::MaskRef<'a>> + Clone,
) -> Result<(), BodyPlanError> {
    if topologies.len() > MAX_BODY_MASK_TOPOLOGIES {
        return Err(BodyPlanError::MaskTopologyCapacityExceeded);
    }
    for (index, topology) in topologies.clone().enumerate() {
        let valid_source = match (&topology.face.plot, &topology.face.source_placement_id) {
            (None, None) => true,
            (Some(plot), None) => workset.contains(plot),
            (Some(plot), Some(source)) => {
                workset.contains(plot)
                    && plots.clone().any(|partition| {
                        partition.plot == plot
                            && partition
                                .plan
                                .metadata()
                                .fragments
                                .iter()
                                .flat_map(|fragment| &fragment.placements)
                                .any(|placement| &placement.placement_id == source)
                    })
            }
            _ => false,
        };
        if !valid_source
            || topology.chains().len() == 0
            || topology.chains().len() > MAX_BODY_MASK_CHAINS
        {
            return Err(BodyPlanError::InvalidMaskChain);
        }
        if topologies
            .clone()
            .skip(index + 1)
            .any(|other| other.face == topology.face)
        {
            return Err(BodyPlanError::DuplicateMaskTopology);
        }
        for chain in topology.chains() {
            if !chain.plan.valid()
                || chain.stages.is_empty()
                || chain.stages.len() > MAX_BODY_MASK_STAGES
            {
                return Err(BodyPlanError::InvalidMaskChain);
            }
            let placements = chain
                .plan
                .metadata()
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .collect::<Vec<_>>();
            for (stage_index, stage_id) in chain.stages.iter().enumerate() {
                if chain.stages[..stage_index].contains(stage_id)
                    || placements
                        .iter()
                        .filter(|placement| &placement.placement_id == stage_id)
                        .count()
                        != 1
                {
                    return Err(BodyPlanError::InvalidMaskChain);
                }
                if let Some(next_id) = chain.stages.get(stage_index + 1) {
                    let connected = chain
                        .plan
                        .metadata()
                        .fragments
                        .iter()
                        .flat_map(|f| &f.connections)
                        .any(|cord| {
                            &cord.source_placement_id == stage_id
                                && &cord.sink_placement_id == next_id
                        });
                    if !connected {
                        return Err(BodyPlanError::InvalidMaskChain);
                    }
                }
            }
        }
    }
    Ok(())
}
