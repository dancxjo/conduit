//! Inline Forms executed by the ordinary finite browser Host installation.

pub(crate) mod abi;
mod audio;
mod body_start;
mod compact_patchbay;
mod engine;
mod gallery;
mod host_abi;
mod host_outcomes;
mod multihost;
mod protocol;
#[cfg(test)]
mod remote_execution;
mod session_cancellation;
mod session_effects;
mod session_projection;
mod session_signs;
#[cfg(feature = "creche-surface")]
pub(crate) mod workspace;

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
    /// Logical resource reservations retained for the lifetime of a Body Play.
    _resource_admissions: Option<conduit_core::ResourceAdmissionOwner>,
    cancellation: Option<conduit_kernel::scheduler::HostOperationCancellation>,
    scheduler: engine::TourScheduler,
    pending: Vec<engine::PendingHostEffect>,
    host_outcomes: host_outcomes::HostOutcomes,
    fragments: Vec<PlanFragment>,
    active_play_id: conduit_core::ActivePlayId,
    terminal_sign_sequence: u64,
    latest_presentation: Option<PresentationIdentity>,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    realization: MorseRealization,
    expanded_gears: Vec<Vec<TourGearEvidence>>,
    realization_backs: Vec<Vec<TourBackEvidence>>,
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
        let (startup, mut catalog) =
            crate::installed_browser::catalogs_for_presentation(presentation)?;
        let syntax = conduit_form::parse_syntax_document(source);
        if let Some(diagnostic) = syntax.diagnostics.first() {
            return Err(format!(
                "parse executable-tour Form: {}",
                diagnostic.message
            ));
        }
        let checked = conduit_form::check_syntax_document(&syntax, &startup)
            .map_err(|error| format!("check executable-tour Form: {error:?}"))?;
        let selector_offers =
            crate::installed_browser::catalogs::install_checked_structured_selectors(
                &checked,
                &mut catalog,
            )?;
        let entry = executable_entry(&checked)?;
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
        let mut host = crate::installed_browser::advertisement_for_presentation(
            host_id.into(),
            boot_id.into(),
            presentation,
        );
        host.capabilities.extend(selector_offers);
        host.capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
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
            _resource_admissions: None,
            cancellation: None,
            scheduler,
            host_outcomes: host_outcomes::HostOutcomes::new(),
            pending: pending_effects,
            fragments: vec![fragment.clone()],
            active_play_id: active.active_play_id,
            terminal_sign_sequence: 0,
            latest_presentation: None,
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            realization,
            expanded_gears: vec![expanded_gears],
            realization_backs: vec![realization_backs],
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
        let sign = bind_sign(
            &self.host_id,
            &self.boot_id,
            Some(&self.active_play_id),
            self.terminal_sign_sequence,
        );
        Ok(self.with_kernel_signs(receipt(
            "cancelled",
            &self.active_play_id,
            self.latest_presentation.as_ref(),
            &sign,
            self.timer_completions,
            self.manifestation_completions,
        )))
    }

    fn completed_receipt(&self) -> TourReceipt {
        let sign = bind_sign(
            &self.host_id,
            &self.boot_id,
            Some(&self.active_play_id),
            self.terminal_sign_sequence,
        );
        self.with_kernel_signs(receipt(
            "completed",
            &self.active_play_id,
            self.latest_presentation.as_ref(),
            &sign,
            self.timer_completions,
            self.manifestation_completions,
        ))
    }
}

fn executable_entry(checked: &conduit_form::CheckedSyntaxDocument) -> Result<String, String> {
    checked
        .forms
        .iter()
        .rev()
        .find(|form| {
            let face = form.checked_face();
            face.inputs().is_empty()
                && face.outputs().is_empty()
                && face
                    .startup_parameters()
                    .iter()
                    .all(|parameter| parameter.has_default)
        })
        .map(|form| form.name.clone())
        .ok_or_else(|| "executable-tour source has no closed root Form".to_string())
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

/// Numeric nodes follow the contiguous partition order established before Play.
fn placement_in_fragments(
    fragments: &[PlanFragment],
    node: conduit_kernel::NodeId,
) -> Option<(&PlanFragment, &conduit_core::PlannedGear)> {
    let mut index = usize::from(node.0);
    for fragment in fragments {
        if let Some(placement) = fragment.placements.get(index) {
            return Some((fragment, placement));
        }
        index = index.checked_sub(fragment.placements.len())?;
    }
    None
}

#[cfg(test)]
mod clock_tests;
#[cfg(test)]
mod firefly_choir_tests;
#[cfg(test)]
mod measurement_observation_tests;
#[cfg(test)]
mod quantity_output_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod normalized_presentation_tests;

#[cfg(test)]
mod comparison_presentation_tests;

#[cfg(test)]
mod startup_chime_tests;

#[cfg(test)]
mod continuous_lifecycle_tests;
