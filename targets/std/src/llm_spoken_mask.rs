//! Owner-held LLM-assisted spoken Mask planning and current model observation.
//! The Plot is shared with the producer journey, but this path plans and runs
//! against the installed owner's selected Host Boot, never a replay Host.

use conduit_core::{
    AuthorityGrant, BaseImplementationId, ConnectionTrack, Plan, PoolRealizationEnvelope,
    PoolRealizationHealth, PoolRealizationObservation, PortDirection, ResourceHealth, SignId,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{MaskPlot, PlannedMaskPlot, Presentation};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_MODEL_OBSERVATION: AtomicU64 = AtomicU64::new(0);

pub struct PreparedLlmSpokenMask {
    pub plan: Plan,
    pub mask: MaskPlot,
    pub planned_mask: PlannedMaskPlot,
    pub authority_grants: Vec<AuthorityGrant>,
}

impl crate::StdHost {
    pub fn prepare_llm_spoken_mask(
        &self,
        face: &Presentation,
    ) -> Result<PreparedLlmSpokenMask, String> {
        face.validate()
            .map_err(|error| format!("prepare LLM spoken Face: {error:?}"))?;
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        conduit_presentation::install_mask_plot_value_aliases(&mut startup)?;
        conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles)?;
        conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
        conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
        conduit_tongues::install_speech_commit_catalog(&mut startup, &mut profiles)?;
        conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
        let plot_name = "owner_llm_spoken_mask";
        let source = crate::spoken_mask_journey::graph::source(plot_name, 1_323_000, true);
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
            .map_err(|error| format!("check owner LLM spoken Mask: {error:?}"))?;
        let authoring = expand_canonical_plot_for_authoring(&checked, plot_name, &profiles)
            .map_err(|error| format!("expand owner LLM spoken Mask: {error:?}"))?;
        let mask = MaskPlot::admit(&authoring)
            .map_err(|error| format!("admit owner LLM spoken Mask: {error:?}"))?;
        let hosts = [self.advertisement().clone()];
        let placements = default_expanded_placements(&authoring.expanded, &hosts)
            .map_err(|error| format!("place owner LLM spoken Mask: {error:?}"))?;
        let boundary_limits = authoring
            .front
            .inputs()
            .iter()
            .map(|port| (PortDirection::Input, port))
            .chain(
                authoring
                    .front
                    .outputs()
                    .iter()
                    .map(|port| (PortDirection::Output, port)),
            )
            .map(|(direction, port)| {
                (
                    ForeBoundaryKey {
                        direction,
                        front_port_id: port.port_id.clone(),
                        track: ConnectionTrack::Payload,
                    },
                    ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: 524_288,
                    },
                )
            })
            .collect();
        let authority_grants = vec![
            self.spoken_mask_artifact_authority_grant("grant/owner-llm-speech/artifact")?,
            self.streaming_speech_authority_grant()?,
        ];
        let plan = plan_expanded_authoring_with_options(
            &authoring,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 16_384,
                authority_grants: &authority_grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundary_limits,
        )
        .map_err(|error| format!("plan owner LLM spoken Mask: {error:?}"))?;
        let planned_mask = PlannedMaskPlot::admit(&mask, &plan)
            .map_err(|error| format!("seal owner LLM spoken Mask: {error:?}"))?;
        self.observe_planned_llm_provider(&planned_mask)?;
        Ok(PreparedLlmSpokenMask {
            plan,
            mask,
            planned_mask,
            authority_grants,
        })
    }

    /// Refresh the exact model content and resource possession for a sealed
    /// placement. An installed offer alone never makes the route live.
    pub fn observe_planned_llm_provider(
        &self,
        planned: &PlannedMaskPlot,
    ) -> Result<PoolRealizationObservation, String> {
        let placement = planned
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| {
                placement.kind_id.as_str() == conduit_ai::LLM_PRESENT_KIND
                    && placement.implementation_id.as_str()
                        == conduit_ai::LOCAL_MODEL_IMPLEMENTATION
            })
            .ok_or("LLM spoken Mask has no selected local-model placement")?;
        let sequence = NEXT_MODEL_OBSERVATION.fetch_add(1, Ordering::Relaxed);
        let sign = |index: &str| {
            SignId::from(format!(
                "sign/{}/model-observation/{sequence}/{index}",
                planned.plan.plan_id.as_str()
            ))
        };
        let realization = PoolRealizationEnvelope {
            host_id: placement.host_id.clone(),
            boot_id: placement.boot_id.clone(),
            offer_generation: placement.offer_generation,
            capability_id: placement.capability_id.clone(),
            implementation_id: placement.implementation_id.clone(),
            artifact_id: placement.artifact_id.clone(),
            member_capacity: 1,
            resources: placement.resources.clone(),
            admitted_lines: vec![],
        };
        let resource_signs = (0..realization.resources.len())
            .map(|index| sign(&format!("resource/{index}")))
            .collect::<Vec<_>>();
        let observation = self.observe_local_model_pool_realization(
            &realization,
            sign("provider"),
            &resource_signs,
        )?;
        if observation.health != PoolRealizationHealth::Ready
            || observation
                .resources
                .iter()
                .zip(&realization.resources)
                .any(|(current, binding)| {
                    current.health != ResourceHealth::Ready
                        || current.unreserved_units < binding.units
                })
        {
            return Err("selected local-model provider or resources are unavailable".into());
        }
        Ok(observation)
    }
}
