//! Target-owned realization of the body's selected Mask chains.

use alloc::{string::String, vec, vec::Vec};
use conduit_body::{
    BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlan, BodyPlayIdentity, ResidentPlot,
};
pub use conduit_conduitos_mask_offer::native_host_advertisement;
pub(super) use conduit_conduitos_mask_offer::{Adapter, MaskStage, prepare_stage};
use conduit_core::HostId;
use conduit_presentation::{
    BodyMaskWardrobe, MaskShow, MaskWardrobe, MaskWardrobeLifetime, Presentation,
};
use patchbay_application::{PatchbayMaskMode, PatchbayMaskStage, PatchbayMaskTopology};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeMaskEvidence {
    pub schema: &'static str,
    pub body_id: String,
    pub wake_id: String,
    pub actions: Vec<&'static str>,
    pub mask_actions: Vec<serde_json::Value>,
    pub wardrobe_revision: u64,
    pub worn_mask_plots: Vec<conduit_core::PlotIdentity>,
    pub preference: Vec<conduit_core::PlotIdentity>,
    pub application_plan_id: conduit_core::PlanId,
    pub mask_plan_ids: Vec<conduit_core::PlanId>,
    pub route_disposition: &'static str,
    pub planning_disposition: &'static str,
    pub show_id: Option<String>,
    pub manifestation_id: Option<String>,
    pub presentation_id: Option<String>,
    pub presentation_revision: Option<u64>,
    pub shows: Vec<NativeMaskShowCorrelation>,
    pub kernel_signs: u16,
    pub fore_endpoints: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeMaskShowCorrelation {
    pub mask_plan_id: conduit_core::PlanId,
    pub mask_active_play_id: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub show_value_id: String,
    pub mask_show_id: String,
    pub manifestation_id: String,
    pub surface_possession:
        Option<crate::native_surface_possession::NativeSurfacePossessionReceipt>,
}

#[derive(Clone)]
pub(super) struct MaskControl {
    host_id: HostId,
    boot_id: conduit_core::BootId,
    graphical: Option<MaskStage>,
    speech: Option<MaskStage>,
    sequence: u64,
    presentation: Option<Presentation>,
    shows: Vec<MaskShow>,
    evidence: Option<NativeMaskEvidence>,
    surface_provider: Option<crate::native_surface_provider::NativeSurfaceProvider>,
}

impl MaskControl {
    pub(super) fn graphical(
        host_id: HostId,
        boot_id: conduit_core::BootId,
        surface_provider: Option<crate::native_surface_provider::NativeSurfaceProvider>,
    ) -> Result<Self, ()> {
        #[cfg(test)]
        let surface_provider =
            surface_provider.or_else(|| Some(crate::product_bases::fixture_surface_provider()));
        let graphical = prepare_stage(
            Adapter::Native,
            &host_id,
            &boot_id,
            1,
            surface_provider.as_ref().map(|provider| &provider.entry),
        )
        .map_err(|_| ())?;
        Ok(Self {
            host_id,
            boot_id,
            graphical: Some(graphical),
            speech: None,
            sequence: 2,
            presentation: None,
            shows: Vec::new(),
            evidence: None,
            surface_provider,
        })
    }

    pub(super) fn request(&mut self, mode: PatchbayMaskMode) -> Result<(), ()> {
        if matches!(
            mode,
            PatchbayMaskMode::Graphical | PatchbayMaskMode::GraphicalAndSpeech
        ) && self.graphical.is_none()
        {
            self.graphical = Some(
                prepare_stage(
                    Adapter::Native,
                    &self.host_id,
                    &self.boot_id,
                    self.sequence,
                    self.surface_provider
                        .as_ref()
                        .map(|provider| &provider.entry),
                )
                .map_err(|_| ())?,
            );
            self.sequence = self.sequence.checked_add(1).ok_or(())?;
        }
        if matches!(
            mode,
            PatchbayMaskMode::Speech | PatchbayMaskMode::GraphicalAndSpeech
        ) && self.speech.is_none()
        {
            self.speech = Some(
                prepare_stage(
                    Adapter::Speech,
                    &self.host_id,
                    &self.boot_id,
                    self.sequence,
                    None,
                )
                .map_err(|_| ())?,
            );
            self.sequence = self.sequence.checked_add(1).ok_or(())?;
        }
        if mode == PatchbayMaskMode::Graphical {
            self.speech = None;
        } else if mode == PatchbayMaskMode::Speech {
            self.graphical = None;
        }
        self.presentation = None;
        self.shows.clear();
        self.evidence = None;
        Ok(())
    }

    pub(super) fn topology(&self, selector: BodyFaceSelector) -> Result<BodyMaskTopology, ()> {
        let mut chains = Vec::new();
        for stage in self.graphical.iter().chain(self.speech.iter()) {
            chains.push(BodyMaskChainPlan {
                plan: stage.planned_mask.plan.clone(),
                // BodyMaskChainPlan is the linear projection used by
                // the Patchbay topology view. The ordinary Mask Plan above owns
                // the complete branched renderer/interaction graph; this path
                // names only its exact Show-producing stage.
                stage_placement_ids: vec![stage.planned_mask.show_placement().placement_id.clone()],
            });
        }
        if chains.is_empty() {
            return Err(());
        }
        Ok(BodyMaskTopology {
            face: selector,
            chains,
        })
    }

    pub(super) fn activate(
        &mut self,
        wake: &conduit_body::Wake,
        body_plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<PatchbayMaskTopology, ()> {
        let selected = body_plan.mask_topologies.first().ok_or(())?;
        if body_plan.mask_topologies.len() != 1
            || selected.chains.len() != self.graphical.iter().chain(self.speech.iter()).count()
        {
            return Err(());
        }
        for stage in self.graphical.iter().chain(self.speech.iter()) {
            if !selected
                .chains
                .iter()
                .any(|chain| chain.plan.plan_id == stage.planned_mask.plan.plan_id)
            {
                return Err(());
            }
        }
        let mode = match (self.graphical.is_some(), self.speech.is_some()) {
            (true, false) => PatchbayMaskMode::Graphical,
            (true, true) => PatchbayMaskMode::GraphicalAndSpeech,
            (false, true) => PatchbayMaskMode::Speech,
            (false, false) => return Err(()),
        };
        let available_masks = self
            .graphical
            .iter()
            .chain(self.speech.iter())
            .map(|stage| stage.planned_mask.mask.plot_identity.clone())
            .collect::<Vec<_>>();
        let mask_plan_ids = self
            .graphical
            .iter()
            .chain(self.speech.iter())
            .map(|stage| stage.planned_mask.plan.plan_id.clone())
            .collect::<Vec<_>>();
        let mut wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, Vec::new(), Vec::new())
            .map_err(|_| ())?;
        let mut actions = Vec::new();
        for mask in &available_masks {
            wardrobe = wardrobe
                .wear(wardrobe.revision, mask.clone())
                .map_err(|_| ())?;
            actions.push("wear");
        }
        if mode == PatchbayMaskMode::Speech {
            let native = prepare_stage(
                Adapter::Native,
                &self.host_id,
                &self.boot_id,
                self.sequence,
                self.surface_provider
                    .as_ref()
                    .map(|provider| &provider.entry),
            )
            .map_err(|_| ())?
            .planned_mask
            .mask
            .plot_identity;
            wardrobe = wardrobe
                .wear(wardrobe.revision, native.clone())
                .and_then(|next| next.doff(next.revision, &native))
                .map_err(|_| ())?;
            actions.extend(["wear", "doff"]);
        }
        wardrobe = wardrobe
            .prefer(wardrobe.revision, wardrobe.worn.clone())
            .map_err(|_| ())?;
        actions.push("prefer");
        let scoped =
            BodyMaskWardrobe::new(body_plan.body_id.clone(), None, wardrobe).map_err(|_| ())?;
        let mut stages = Vec::new();
        for stage in self.graphical.iter().chain(self.speech.iter()) {
            let planned = stage.planned_mask.show_placement();
            let resource = planned
                .resources
                .first()
                .or_else(|| {
                    stage
                        .planned_mask
                        .plan
                        .fragments
                        .iter()
                        .flat_map(|fragment| &fragment.connections)
                        .find_map(|connection| connection.resource.as_ref())
                        .map(|resource| &resource.source_binding)
                })
                .ok_or(())?;
            stages.push(PatchbayMaskStage {
                manifestation_id: "not-shown".into(),
                implementation_id: planned.implementation_id.as_str().into(),
                host_id: planned.host_id.as_str().into(),
                boot_id: planned.boot_id.as_str().into(),
                resource_pool_id: resource.pool_id.as_str().into(),
                resource_class_id: resource.class_id.as_str().into(),
                reserved_units: resource.units,
                maximum_active_instances: planned.limits.max_active_instances,
                maximum_queue_items: planned.limits.max_queue_items,
                maximum_queue_bytes: planned.limits.max_queue_bytes,
                available: false,
            });
        }
        self.sequence = self.sequence.checked_add(1).ok_or(())?;
        self.presentation = None;
        self.shows.clear();
        self.evidence = Some(NativeMaskEvidence {
            schema: "conduit.conduitos/native-mask-control@1",
            body_id: body_plan.body_id.as_str().into(),
            wake_id: wake.wake_id.as_str().into(),
            actions,
            mask_actions: Vec::new(),
            wardrobe_revision: scoped.wardrobe.revision,
            worn_mask_plots: scoped.wardrobe.worn.clone(),
            preference: scoped.wardrobe.preference.clone(),
            application_plan_id: body_plan.plan_id.clone(),
            mask_plan_ids,
            route_disposition: "planned-route-awaiting-show",
            planning_disposition: "not-required",
            show_id: None,
            manifestation_id: None,
            presentation_id: None,
            presentation_revision: None,
            shows: Vec::new(),
            kernel_signs: 0,
            fore_endpoints: 0,
        });
        Ok(PatchbayMaskTopology {
            presentation_id: "none".into(),
            body_plan_id: body_plan.plan_id.clone(),
            active_play_id: play.active_play_id.as_str().into(),
            mode,
            stages,
        })
    }

    pub(super) fn mask_evidence(&self) -> Option<&NativeMaskEvidence> {
        self.evidence.as_ref()
    }
}

pub(super) fn patchbay_selector(plan: &BodyPlan) -> Result<BodyFaceSelector, ()> {
    patchbay_partition_selector(&plan.plots)
}

pub(super) fn patchbay_partition_selector(
    partitions: &[conduit_body::BodyPlotPlan],
) -> Result<BodyFaceSelector, ()> {
    let resident = crate::native_workset::resident(crate::native_workset::NativePlot::Patchbay)
        .map_err(|_| ())?;
    let partition = partitions
        .iter()
        .find(|partition| partition.plot == resident)
        .ok_or(())?;
    let placement = partition
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| {
            placement.kind_id.as_str()
                == conduit_semantic_catalog::APPLICATION_VIEW_PRESENTATION_KIND
        })
        .ok_or(())?;
    Ok(BodyFaceSelector {
        plot: Some(ResidentPlot::new(
            resident.source_document_id,
            resident.checked_plot_id,
        )),
        source_placement_id: Some(placement.placement_id.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_offer_and_native_mask_use_the_same_observed_surface() {
        let host = HostId::from("conduitos/test-host");
        let boot = conduit_core::BootId::from("conduitos/test-boot");
        let provider = crate::product_bases::fixture_surface_provider();
        let offer = native_host_advertisement(&host, &boot, 1, &provider.entry);
        let planned = prepare_stage(Adapter::Native, &host, &boot, 1, Some(&provider.entry))
            .unwrap()
            .planned_mask;

        assert_eq!(offer.bases.len(), 1);
        assert_eq!(offer.bases[0].base_id, provider.entry.base_id);
        assert_eq!(
            offer.bases[0].resource_pool_ids,
            vec![provider.entry.resources[0].pool_id.clone()]
        );
        let reserved_surface = planned
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find_map(|connection| connection.resource.as_ref())
            .unwrap();
        assert_eq!(
            reserved_surface.source_binding.pool_id,
            provider.entry.resources[0].pool_id
        );
    }

    #[test]
    fn native_mask_is_the_exact_ordinary_mask_plot_and_keeps_its_branched_plan() {
        let host = HostId::from("conduitos/test-host");
        let boot = conduit_core::BootId::from("conduitos/test-boot");
        let control = MaskControl::graphical(host, boot, None).unwrap();
        let stage = control.graphical.as_ref().unwrap();

        assert_eq!(stage.planned_mask.mask.plot_name, "native-graphical");
        assert_eq!(
            stage.planned_mask.plan.source_document_id,
            stage.planned_mask.mask.plot_identity.source_document_id
        );
        assert_eq!(
            stage.planned_mask.plan.checked_plot_id,
            stage.planned_mask.mask.plot_identity.checked_plot_id
        );
        assert_eq!(
            stage.planned_mask.plan.expanded_plot_id,
            stage.planned_mask.mask.plot_identity.expanded_plot_id
        );
        assert_eq!(
            [
                &stage.planned_mask.mask.face_input,
                &stage.planned_mask.mask.interaction_output,
                &stage.planned_mask.mask.show_output,
            ]
            .len(),
            3,
            "the Mask retains one Face input and its interaction and Show outputs"
        );
        assert_eq!(
            stage
                .planned_mask
                .plan
                .fragments
                .iter()
                .map(|fragment| fragment.connections.len())
                .sum::<usize>(),
            4,
            "the planned Mask retains both Presentation branches, Show correlation, and the local show-resource Cord"
        );
        let resource_connections = stage
            .planned_mask
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .filter(|connection| connection.resource.is_some())
            .collect::<Vec<_>>();
        assert_eq!(resource_connections.len(), 1);
        assert_eq!(
            resource_connections[0]
                .resource
                .as_ref()
                .unwrap()
                .source_binding
                .class_id
                .as_str(),
            conduit_presentation::SHOW_RESOURCE_CLASS
        );

        let topology = control
            .topology(BodyFaceSelector {
                plot: None,
                source_placement_id: None,
            })
            .unwrap();
        assert_eq!(topology.chains.len(), 1);
        assert_eq!(topology.chains[0].stage_placement_ids.len(), 1);
        assert_eq!(
            topology.chains[0].stage_placement_ids[0],
            stage.planned_mask.show_placement().placement_id
        );
        assert_eq!(topology.chains[0].plan, stage.planned_mask.plan);
        assert!(control.evidence.is_none());
        assert!(control.shows.is_empty());
    }

    #[test]
    fn spoken_mask_is_the_exact_ordinary_mask_plot_and_keeps_its_branched_plan() {
        let host = HostId::from("conduitos/test-host");
        let boot = conduit_core::BootId::from("conduitos/test-boot");
        let mut control = MaskControl::graphical(host, boot, None).unwrap();
        control.request(PatchbayMaskMode::Speech).unwrap();
        let stage = control.speech.as_ref().unwrap();

        assert!(control.graphical.is_none());
        assert_eq!(stage.planned_mask.mask.plot_name, "spoken");
        assert_eq!(
            stage.planned_mask.plan.source_document_id,
            stage.planned_mask.mask.plot_identity.source_document_id
        );
        assert_eq!(
            stage.planned_mask.plan.checked_plot_id,
            stage.planned_mask.mask.plot_identity.checked_plot_id
        );
        assert_eq!(
            stage.planned_mask.plan.expanded_plot_id,
            stage.planned_mask.mask.plot_identity.expanded_plot_id
        );
        assert_eq!(
            stage
                .planned_mask
                .plan
                .fragments
                .iter()
                .map(|fragment| fragment.connections.len())
                .sum::<usize>(),
            3,
            "the planned spoken Mask retains both Presentation branches and its Show correlation"
        );
    }
}
