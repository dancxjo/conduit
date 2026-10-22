//! Select resident Patchbay Mask chains before sealing the authoritative Body Plan.
use conduit_body::{BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlotPlan};
use conduit_core::{BootId, HostAdvertisement, HostId};

pub(crate) fn masks(
    plots: &[BodyPlotPlan],
    hosts: &[HostAdvertisement],
    host_id: &HostId,
    boot_id: &BootId,
) -> Result<Vec<BodyMaskTopology>, String> {
    let residents = plots
        .iter()
        .filter(|part| {
            part.plan.fragments.iter().any(|fragment| {
                &fragment.host_id == host_id
                    && &fragment.boot_id == boot_id
                    && fragment.placements.iter().any(|placement| {
                        placement.kind_id.as_str()
                            == conduit_semantic_catalog::RETAINED_APPLICATION_KIND
                            && placement.configuration.iter().any(|entry| {
                                entry.key == "application"
                                    && entry.value
                                        == conduit_core::ConfigurationValue::Text("patchbay".into())
                            })
                    })
            })
        })
        .collect::<Vec<_>>();
    if residents.is_empty() {
        return Ok(Vec::new());
    }
    let host = hosts
        .iter()
        .find(|host| &host.host_id == host_id && &host.boot_id == boot_id)
        .ok_or_else(|| String::from("current browser Host offer is absent"))?;
    let chains = [
        (
            crate::workspace_mask::plan::MASK_SOURCE,
            "browser-graphical",
        ),
        (
            crate::workspace_mask::plan::ALTERNATE_MASK_SOURCE,
            "browser-graphical-alternate",
        ),
    ]
    .into_iter()
    .map(|(source, name)| {
        let planned = crate::workspace_mask::plan::planned_mask(host, source, name)
            .map_err(|error| format!("{error:?}"))?;
        Ok(BodyMaskChainPlan {
            stage_placement_ids: planned
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .map(|placement| placement.placement_id.clone())
                .collect(),
            plan: planned.plan,
        })
    })
    .collect::<Result<Vec<_>, String>>()?;
    Ok(residents
        .into_iter()
        .map(|part| BodyMaskTopology {
            face: BodyFaceSelector {
                plot: Some(part.plot.clone()),
                source_placement_id: None,
            },
            chains: chains.clone(),
        })
        .collect())
}
