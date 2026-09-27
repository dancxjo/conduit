//! Native actualization of the shared Mask journey action contract.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::BodyId;
use conduit_core::{BootId, FormIdentity, HostId, PlanId, SignId};
use conduit_presentation::{
    BodyMaskWardrobe, ManifestationLifecycle, MaskJourneyAction, MaskJourneyEmbodiment, MaskShow,
    MaskWardrobe, MaskWardrobeLifetime, Presentation, actualize_mask_journey,
};
use serde::Serialize;

use crate::presenter_control::{Adapter, PresenterStage, prepare_stage};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeMaskJourneyObservation {
    pub action_id: &'static str,
    pub concrete_event: &'static str,
    pub presentation_id: String,
    pub selected_mask_form_id: Option<String>,
    pub plan_id: PlanId,
    pub selected_route_id: Option<String>,
    pub show_id: Option<String>,
    pub receipt_ids: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn actualize(
    body_id: &BodyId,
    host_id: &HostId,
    boot_id: &BootId,
    presentation: &Presentation,
    initial: &PresenterStage,
    sequence: u64,
    initial_show: &MaskShow,
    initial_receipt: &crate::native_mask_play::NativeMaskPlayReceipt,
) -> Result<Vec<NativeMaskJourneyObservation>, ()> {
    let speech_alternate = prepare_stage(
        Adapter::Speech,
        host_id,
        boot_id,
        sequence,
        "conduitos/journey-spoken",
    )
    .map_err(|_| {
        #[cfg(test)]
        panic!("alternate Mask preparation failed");
    })?;
    let alternate_is_speech =
        speech_alternate.planned_mask.mask.form_identity != initial.planned_mask.mask.form_identity;
    let alternate = if alternate_is_speech {
        speech_alternate
    } else {
        prepare_stage(
            Adapter::Native,
            host_id,
            boot_id,
            sequence,
            "conduitos/journey-native",
        )?
    };
    let replacement_host = HostId::from(format!("{}/presentation-replacement", host_id.as_str()));
    let replacement = prepare_stage(
        if alternate_is_speech {
            Adapter::Speech
        } else {
            Adapter::Native
        },
        &replacement_host,
        boot_id,
        sequence.checked_add(1).ok_or(())?,
        "conduitos/journey-spoken-replacement",
    )
    .map_err(|_| {
        #[cfg(test)]
        panic!("replacement Mask preparation failed");
    })?;
    let restored = prepare_stage(
        if alternate_is_speech {
            Adapter::Native
        } else {
            Adapter::Speech
        },
        &replacement_host,
        boot_id,
        sequence.checked_add(2).ok_or(())?,
        "conduitos/journey-native-restored",
    )
    .map_err(|_| {
        #[cfg(test)]
        panic!("restored Mask preparation failed");
    })?;
    let mut embodiment = NativeJourney::new(
        body_id,
        presentation,
        initial,
        alternate,
        replacement,
        restored,
        initial_show,
        initial_receipt,
    )?;
    let mut observations = Vec::new();
    actualize_mask_journey(&mut embodiment, |action, outcome| {
        debug_assert_eq!(action.id(), outcome.action_id);
        observations.push(outcome.clone());
    })
    .map_err(|_error| {
        #[cfg(test)]
        panic!("native Mask journey failed at {:?}", _error.action);
        #[allow(unreachable_code)]
        ()
    })?;
    Ok(observations)
}

struct NativeJourney<'a> {
    presentation: &'a Presentation,
    alternate: PresenterStage,
    replacement: PresenterStage,
    restored: PresenterStage,
    wardrobe: BodyMaskWardrobe,
    plan_id: PlanId,
    selected_mask: Option<FormIdentity>,
    selected_route: Option<String>,
    show: Option<MaskShow>,
    receipts: Vec<String>,
}

impl<'a> NativeJourney<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        body_id: &BodyId,
        presentation: &'a Presentation,
        initial: &'a PresenterStage,
        alternate: PresenterStage,
        replacement: PresenterStage,
        restored: PresenterStage,
        initial_show: &MaskShow,
        initial_receipt: &crate::native_mask_play::NativeMaskPlayReceipt,
    ) -> Result<Self, ()> {
        let initial_mask = initial.planned_mask.mask.form_identity.clone();
        let wardrobe = MaskWardrobe::new(
            MaskWardrobeLifetime::Body,
            vec![initial_mask.clone()],
            vec![initial_mask.clone()],
        )
        .map_err(|_| ())?;
        let mut state = Self {
            presentation,
            alternate,
            replacement,
            restored,
            wardrobe: BodyMaskWardrobe::new(body_id.clone(), None, wardrobe).map_err(|_| ())?,
            plan_id: initial.planned_mask.plan.plan_id.clone(),
            selected_mask: Some(initial_mask),
            selected_route: Some(route_id(initial)),
            show: None,
            receipts: Vec::new(),
        };
        state.show = Some(initial_show.clone());
        state.receipts = vec![
            initial_receipt.active_play_id.as_str().into(),
            initial_receipt.show_value_id.clone(),
        ];
        Ok(state)
    }

    fn observation(
        &self,
        action: MaskJourneyAction,
        concrete_event: &'static str,
    ) -> NativeMaskJourneyObservation {
        NativeMaskJourneyObservation {
            action_id: action.id(),
            concrete_event,
            presentation_id: self.presentation.identity.as_str().into(),
            selected_mask_form_id: self
                .selected_mask
                .as_ref()
                .map(|mask| mask.expanded_form_id.as_str().into()),
            plan_id: self.plan_id.clone(),
            selected_route_id: self.selected_route.clone(),
            show_id: self.show.as_ref().map(|show| show.show_id.as_str().into()),
            receipt_ids: self.receipts.clone(),
        }
    }
}

impl MaskJourneyEmbodiment for NativeJourney<'_> {
    type Outcome = NativeMaskJourneyObservation;
    type Error = ();

    fn perform(&mut self, action: MaskJourneyAction) -> Result<Self::Outcome, Self::Error> {
        let event = match action {
            MaskJourneyAction::InspectInitialShow => "inspected-executed-native-show",
            MaskJourneyAction::WearAlternateMask => {
                let mask = self.alternate.planned_mask.mask.form_identity.clone();
                self.wardrobe.wardrobe = self
                    .wardrobe
                    .wardrobe
                    .wear(self.wardrobe.wardrobe.revision, mask)
                    .map_err(|_| ())?;
                "wore-alternate-mask"
            }
            MaskJourneyAction::PreferAlternateMask => {
                let mask = self.alternate.planned_mask.mask.form_identity.clone();
                self.wardrobe.wardrobe = self
                    .wardrobe
                    .wardrobe
                    .prefer(self.wardrobe.wardrobe.revision, vec![mask])
                    .map_err(|_| ())?;
                // Preference is planning input. The valid current Show and its
                // immutable Plan remain selected until an observed withdrawal.
                "preferred-alternate-retained-current-show"
            }
            MaskJourneyAction::WithdrawSelectedRoute => {
                self.show = None;
                self.selected_mask = None;
                self.selected_route = None;
                self.receipts = vec!["native-route-withdrawn".into()];
                "withdrew-selected-native-route"
            }
            MaskJourneyAction::InspectUnavailableShow => "inspected-no-current-show",
            MaskJourneyAction::AddPresentationHost => {
                self.receipts = vec![format!(
                    "host-offer:{}",
                    self.replacement
                        .planned_mask
                        .plan
                        .fragments
                        .first()
                        .ok_or(())?
                        .host_id
                        .as_str()
                )];
                "added-presentation-host-without-selecting-unsealed-route"
            }
            MaskJourneyAction::AdmitReplacementPlan => {
                self.plan_id = self.replacement.planned_mask.plan.plan_id.clone();
                self.selected_mask = Some(self.replacement.planned_mask.mask.form_identity.clone());
                self.selected_route = Some(route_id(&self.replacement));
                let (show, receipts) = execute(
                    &self.replacement,
                    self.presentation,
                    self.presentation.revision.checked_add(1).ok_or(())?,
                    "replacement",
                )?;
                self.show = Some(show);
                self.receipts = receipts;
                "admitted-and-executed-replacement-plan"
            }
            MaskJourneyAction::InspectReplannedShow => "inspected-replacement-plan-show",
            MaskJourneyAction::DoffAlternateMask => {
                let alternate = self.alternate.planned_mask.mask.form_identity.clone();
                self.wardrobe.wardrobe = self
                    .wardrobe
                    .wardrobe
                    .doff(self.wardrobe.wardrobe.revision, &alternate)
                    .map_err(|_| ())?;
                self.plan_id = self.restored.planned_mask.plan.plan_id.clone();
                self.selected_mask = Some(self.restored.planned_mask.mask.form_identity.clone());
                self.selected_route = Some(route_id(&self.restored));
                let (show, receipts) = execute(
                    &self.restored,
                    self.presentation,
                    self.presentation.revision.checked_add(2).ok_or(())?,
                    "restored",
                )?;
                self.show = Some(show);
                self.receipts = receipts;
                "doffed-alternate-and-executed-restored-plan"
            }
            MaskJourneyAction::InspectRestoredShow => "inspected-restored-show",
        };
        Ok(self.observation(action, event))
    }
}

fn route_id(stage: &PresenterStage) -> String {
    format!("mask-route/{}", stage.planned_mask.plan.plan_id.as_str())
}

fn execute(
    stage: &PresenterStage,
    presentation: &Presentation,
    play_sequence: u64,
    label: &str,
) -> Result<(MaskShow, Vec<String>), ()> {
    let receipt = crate::native_mask_play::run(&stage.planned_mask, presentation, play_sequence)
        .map_err(|_error| {
            #[cfg(test)]
            panic!("native Mask execution failed: {_error:?}");
            #[allow(unreachable_code)]
            ()
        })?;
    let fragment = stage.planned_mask.plan.fragments.first().ok_or(())?;
    let active = conduit_core::bind_active_play(
        &stage.planned_mask.plan.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        play_sequence,
    );
    if receipt.active_play_id != active.active_play_id {
        return Err(());
    }
    let show = MaskShow::prepared(
        &stage.planned_mask,
        presentation,
        active,
        "conduitos/patchbay/self".into(),
        stage.target.clone(),
        SignId::from(format!("conduitos/native-mask-journey/{label}/prepared")),
    )
    .and_then(|show| {
        show.transition(
            ManifestationLifecycle::Available,
            SignId::from(format!("conduitos/native-mask-journey/{label}/available")),
        )
    })
    .map_err(|_| ())?;
    Ok((
        show,
        vec![
            receipt.active_play_id.as_str().into(),
            receipt.show_value_id,
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::MASK_JOURNEY_ACTIONS;

    #[test]
    fn retained_action_ids_are_the_shared_complete_order() {
        let ids = MASK_JOURNEY_ACTIONS.map(MaskJourneyAction::id);
        assert_eq!(ids.len(), 10);
        assert_eq!(ids[0], "mask.inspect-initial-show");
        assert_eq!(ids[9], "mask.inspect-restored-show");
    }
}
