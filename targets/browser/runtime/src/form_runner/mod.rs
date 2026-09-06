//! Inline Forms executed by the ordinary finite browser Host installation.

pub(crate) mod abi;
mod compact_patchbay;
mod engine;
mod gallery;
mod host_abi;
mod multihost;
mod protocol;
mod session_cancellation;
mod session_effects;

#[cfg(test)]
use crate::installed_browser::{advertisement, catalogs};
use crate::installed_browser::{backs, local_bases};
use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, Plan, PlanFragment, PresentationIdentity,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};
pub(super) use protocol::refusal;
use protocol::{
    decode_manifestation, receipt, TourBackEvidence, TourButtonTransitionEffect, TourEffect,
    TourGearEvidence, TourHostEffect, TourKeyEventEffect, TourProgress, TourReceipt,
    TourTimerEffect,
};
use std::collections::BTreeMap;

struct TourSession {
    cancellation: Option<conduit_kernel::scheduler::HostOperationCancellation>,
    scheduler: engine::TourScheduler,
    pending: Vec<engine::PendingHostEffect>,
    fragment: PlanFragment,
    active_play_id: conduit_core::ActivePlayId,
    latest_presentation: Option<PresentationIdentity>,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    realization: MorseRealization,
    expanded_gears: Vec<TourGearEvidence>,
    realization_backs: Vec<TourBackEvidence>,
    source_interaction: Option<crate::source_interaction::SourceInteractionEvidence>,
    timer_completions: u32,
    manifestation_completions: u32,
}

impl TourSession {
    fn prepare(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
    ) -> Result<(Self, TourHostEffect), String> {
        Self::prepare_with_realization(
            host_id,
            boot_id,
            source,
            play_sequence,
            MorseRealization::Direct,
        )
    }

    fn prepare_recursive(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
    ) -> Result<(Self, TourHostEffect), String> {
        Self::prepare_with_realization(
            host_id,
            boot_id,
            source,
            play_sequence,
            MorseRealization::Recursive,
        )
    }

    fn prepare_with_realization(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
        realization: MorseRealization,
    ) -> Result<(Self, TourHostEffect), String> {
        Self::prepare_with_profile(
            host_id,
            boot_id,
            source,
            play_sequence,
            realization,
            crate::installed_browser::PresentationProfile::Annotation,
        )
    }

    fn prepare_with_profile(
        host_id: &str,
        boot_id: &str,
        source: &str,
        play_sequence: u64,
        realization: MorseRealization,
        presentation: crate::installed_browser::PresentationProfile,
    ) -> Result<(Self, TourHostEffect), String> {
        let (startup, catalog) = crate::installed_browser::catalogs_for_presentation(presentation)?;
        let syntax = conduit_form::parse_syntax_document(source);
        if let Some(diagnostic) = syntax.diagnostics.first() {
            return Err(format!(
                "parse executable-tour Form: {}",
                diagnostic.message
            ));
        }
        let checked = conduit_form::check_syntax_document(&syntax, &startup)
            .map_err(|error| format!("check executable-tour Form: {error:?}"))?;
        let entry = checked
            .forms
            .last()
            .ok_or_else(|| "executable-tour source has no Form".to_string())?
            .name
            .clone();
        let form = match realization {
            MorseRealization::Direct => {
                conduit_form::expand_canonical_form(&checked, &entry, &catalog)
                    .map_err(|error| format!("expand executable-tour Form: {error:?}"))?
            }
            MorseRealization::Recursive => conduit_form::expand_canonical_form_with_backs(
                &checked,
                &entry,
                &catalog,
                &backs(&startup, &catalog)?,
            )
            .map_err(|error| format!("expand recursive executable-tour Form: {error:?}"))?,
        };
        let host = crate::installed_browser::advertisement_for_presentation(
            host_id.into(),
            boot_id.into(),
            presentation,
        );
        let hosts = [host];
        let placements = default_expanded_placements(&form, &hosts)
            .map_err(|error| format!("place executable-tour Form: {error:?}"))?;
        let bases = local_bases();
        let plan = plan_expanded_canonical_with_options(
            &form,
            &hosts,
            &placements,
            &bases,
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES
                    as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|error| format!("plan executable-tour Form: {error:?}"))?;
        let realization_backs = plan
            .realization_backs
            .iter()
            .map(|back| TourBackEvidence {
                invocation_path: back.invocation_path.clone(),
                kind_id: back.kind_id.as_str().into(),
                checked_form_id: back.checked_form_id.as_str().into(),
            })
            .collect();
        let fragment = exact_fragment(&plan)?;
        let expanded_gears = fragment
            .placements
            .iter()
            .map(|placement| TourGearEvidence {
                gear_id: placement.gear_id.as_str().into(),
                kind_id: placement.kind_id.as_str().into(),
                implementation_id: placement.implementation_id.as_str().into(),
            })
            .collect();
        let mut pending_effects =
            Vec::with_capacity(crate::installed_browser::BROWSER_PENDING_REQUESTS);
        let (scheduler, pending) = engine::prepare(fragment)?;
        pending_effects.push(pending);
        let active = bind_active_play(
            &plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let mut session = Self {
            cancellation: None,
            scheduler,
            pending: pending_effects,
            fragment: fragment.clone(),
            active_play_id: active.active_play_id,
            latest_presentation: None,
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            realization,
            expanded_gears,
            realization_backs,
            source_interaction: None,
            timer_completions: 0,
            manifestation_completions: 0,
        };
        let effect = session.project_pending_effect(0)?;
        Ok((session, effect))
    }

    fn attach_source_interaction(
        &mut self,
        effect: &mut TourHostEffect,
        source_interaction: crate::source_interaction::SourceInteractionEvidence,
    ) {
        self.source_interaction = Some(source_interaction.clone());
        effect.attach_source_interaction(source_interaction);
    }

    #[cfg(test)]
    fn complete(mut self) -> Result<TourReceipt, String> {
        match self.advance()? {
            TourProgress::Receipt(receipt) => Ok(*receipt),
            TourProgress::Effect(_)
            | TourProgress::Waiting { .. }
            | TourProgress::Cancellation { .. } => {
                Err("Tour Play requested another Host effect before completion".into())
            }
        }
    }

    fn cancel(mut self) -> Result<TourReceipt, String> {
        self.scheduler
            .cancel()
            .map_err(|error| format!("{error:?}"))?;
        let sign = bind_sign(&self.host_id, &self.boot_id, Some(&self.active_play_id), 0);
        Ok(receipt(
            "cancelled",
            &self.active_play_id,
            self.latest_presentation.as_ref(),
            &sign,
            self.timer_completions,
            self.manifestation_completions,
        ))
    }

    fn project_pending_effect(&mut self, index: usize) -> Result<TourHostEffect, String> {
        let pending = self
            .pending
            .get(index)
            .ok_or("pending browser effect is absent")?;
        let placement = self
            .fragment
            .placements
            .get(usize::from(pending.request.node.0))
            .ok_or_else(|| "Host effect has no planned placement".to_string())?;
        match &pending.effect {
            engine::BrowserHostEffect::Snapshot { .. } => {
                let request = engine::resource_effect::describe(&self.scheduler, pending)?;
                Ok(TourHostEffect::Snapshot(Box::new(
                    protocol::SnapshotEffect {
                        schema: "conduit.browser/resource-effect@1",
                        effect_kind: request.effect_kind,
                        active_play_id: self.active_play_id.as_str().into(),
                        placement_id: placement.placement_id.as_str().into(),
                        host_id: self.host_id.as_str().into(),
                        boot_id: self.boot_id.as_str().into(),
                        request_sequence: pending.request.request.0,
                        key: request.key.into(),
                        record: request.record.map(<[u8]>::to_vec),
                        source_interaction: self.source_interaction.clone(),
                    },
                )))
            }
            engine::BrowserHostEffect::Timer { duration_millis } => {
                Ok(TourHostEffect::Timer(Box::new(TourTimerEffect {
                    schema: "conduit.tour/timer-effect@1",
                    effect_kind: "timer",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    duration_millis: *duration_millis,
                    source_interaction: self.source_interaction.clone(),
                })))
            }
            engine::BrowserHostEffect::PointerEvent => {
                Ok(TourHostEffect::PointerEvent(Box::new(TourKeyEventEffect {
                    schema: "conduit.browser/pointer-event-effect@1",
                    effect_kind: "pointer-event",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES
                        as u32,
                    source_interaction: self.source_interaction.clone(),
                })))
            }
            engine::BrowserHostEffect::ClockObservation => Ok(TourHostEffect::ClockObservation(
                Box::new(TourKeyEventEffect {
                    schema: "conduit.browser/clock-observation-effect@1",
                    effect_kind: "clock-observation",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: 8,
                    source_interaction: self.source_interaction.clone(),
                }),
            )),
            engine::BrowserHostEffect::KeyEvent => {
                Ok(TourHostEffect::KeyEvent(Box::new(TourKeyEventEffect {
                    schema: "conduit.tour/key-event-effect@1",
                    effect_kind: "key-event",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
                    source_interaction: self.source_interaction.clone(),
                })))
            }
            engine::BrowserHostEffect::ButtonTransition => Ok(TourHostEffect::ButtonTransition(
                Box::new(TourButtonTransitionEffect {
                    schema: "conduit.tour/button-transition-effect@1",
                    effect_kind: "button-transition",
                    active_play_id: self.active_play_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    request_sequence: pending.request.request.0,
                    maximum_output_bytes: conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
                    source_interaction: self.source_interaction.clone(),
                }),
            )),
            engine::BrowserHostEffect::Manifestation(manifestation) => {
                let observation_sequence = pending.request.request.0;
                let presentation = bind_presentation(
                    &self.active_play_id,
                    &placement.placement_id,
                    u64::from(observation_sequence),
                );
                let (unit_millis, segments, text) = decode_manifestation(manifestation)?;
                let effect = TourEffect {
                    schema: "conduit.tour/manifestation-effect@3",
                    effect_kind: "manifestation",
                    source_document_id: self.fragment.source_document_id.as_str().into(),
                    checked_form_id: self.fragment.checked_form_id.as_str().into(),
                    expanded_form_id: self.fragment.expanded_form_id.as_str().into(),
                    plan_id: self.fragment.plan_id.as_str().into(),
                    fragment_id: self.fragment.fragment_id.as_str().into(),
                    active_play_id: self.active_play_id.as_str().into(),
                    presentation_id: presentation.presentation_id.as_str().into(),
                    placement_id: placement.placement_id.as_str().into(),
                    host_id: self.host_id.as_str().into(),
                    boot_id: self.boot_id.as_str().into(),
                    presentation_kind: manifestation.kind_id.into(),
                    observation_sequence,
                    realization: self.realization.as_str(),
                    expanded_gears: self.expanded_gears.clone(),
                    realization_backs: self.realization_backs.clone(),
                    unit_millis,
                    segments,
                    text,
                    source_interaction: self.source_interaction.clone(),
                };
                self.latest_presentation = Some(presentation);
                Ok(TourHostEffect::Manifestation(Box::new(effect)))
            }
        }
    }

    fn completed_receipt(&self) -> TourReceipt {
        let sign = bind_sign(&self.host_id, &self.boot_id, Some(&self.active_play_id), 0);
        receipt(
            "completed",
            &self.active_play_id,
            self.latest_presentation.as_ref(),
            &sign,
            self.timer_completions,
            self.manifestation_completions,
        )
    }
}

#[derive(Clone, Copy)]
enum MorseRealization {
    Direct,
    Recursive,
}

impl MorseRealization {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Recursive => "recursive",
        }
    }
}

fn exact_fragment(plan: &Plan) -> Result<&PlanFragment, String> {
    if plan.fragments.len() != 1 {
        return Err("executable-tour Plan must contain exactly one browser fragment".into());
    }
    plan.fragments
        .first()
        .ok_or_else(|| "executable-tour Plan has no fragment".into())
}

#[cfg(test)]
mod quantity_output_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod normalized_presentation_tests;

#[cfg(test)]
mod comparison_presentation_tests;
