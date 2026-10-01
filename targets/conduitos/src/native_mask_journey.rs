//! Native actualization of the shared Mask journey action contract.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::{BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlan, Wake};
use conduit_core::{BootId, HostId, PlanId, SignId};
use conduit_presentation::{
    AdmittedMaskFormRoutes, BodyMaskWardrobe, ManifestationLifecycle, MaskJourneyAction,
    MaskJourneyEmbodiment, MaskShow, MaskShowDisposition, MaskWardrobe, MaskWardrobeAction,
    MaskWardrobeControl, MaskWardrobeLifetime, Presentation, SealedMaskFormRoute,
    SelectedMaskFormRoute, actualize_mask_journey,
};
use serde::Serialize;

use crate::mask_control::{Adapter, MaskStage, prepare_stage};

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
    wake: &Wake,
    application_plan: &BodyPlan,
    host_id: &HostId,
    boot_id: &BootId,
    presentation: &Presentation,
    initial: &MaskStage,
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
        None,
    )?;
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
            None,
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
        None,
    )?;
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
        None,
    )?;
    let mut embodiment = NativeJourney::new(
        wake,
        application_plan,
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
    .map_err(|_| ())?;
    Ok(observations)
}

struct NativeJourney<'a> {
    presentation: &'a Presentation,
    initial: MaskStage,
    alternate: MaskStage,
    replacement: MaskStage,
    restored: MaskStage,
    initial_plan: BodyPlan,
    initial_routes: AdmittedMaskFormRoutes,
    replacement_plan: BodyPlan,
    replacement_routes: AdmittedMaskFormRoutes,
    control: MaskWardrobeControl,
    show: Option<MaskShow>,
    receipts: Vec<String>,
}

impl<'a> NativeJourney<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        wake: &Wake,
        application_plan: &BodyPlan,
        presentation: &'a Presentation,
        initial: &'a MaskStage,
        alternate: MaskStage,
        replacement: MaskStage,
        restored: MaskStage,
        initial_show: &MaskShow,
        initial_receipt: &crate::native_mask_play::NativeMaskPlayReceipt,
    ) -> Result<Self, ()> {
        let initial_mask = initial.planned_mask.mask.form_identity.clone();
        let selector = application_plan
            .mask_topologies
            .first()
            .ok_or(())?
            .face
            .clone();
        let initial_plan = body_plan_with_masks(
            wake,
            application_plan,
            selector.clone(),
            &[initial, &alternate],
        )?;
        let initial_routes = admitted_routes(&initial_plan, &[initial, &alternate], &[true, true])?;
        let replacement_plan =
            body_plan_with_masks(wake, application_plan, selector, &[&replacement, &restored])?;
        let replacement_routes =
            admitted_routes(&replacement_plan, &[&replacement, &restored], &[true, true])?;
        let wardrobe = MaskWardrobe::new(
            MaskWardrobeLifetime::Body,
            vec![initial_mask.clone()],
            vec![initial_mask.clone()],
        )
        .map_err(|_| ())?;
        let selected = SelectedMaskFormRoute {
            route_id: route_id(initial),
            mask_form: initial_mask,
            plan_id: initial_plan.plan_id.clone(),
        };
        let control = MaskWardrobeControl::new(
            &wake.body_id,
            BodyMaskWardrobe::new(wake.body_id.clone(), None, wardrobe).map_err(|_| ())?,
            &initial_plan,
            &initial_routes,
            Some(selected),
        )
        .map_err(|_| ())?;
        let mut state = Self {
            presentation,
            initial: initial.clone(),
            alternate,
            replacement,
            restored,
            initial_plan,
            initial_routes,
            replacement_plan,
            replacement_routes,
            control,
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
        let retains_fresh_show = matches!(
            action,
            MaskJourneyAction::InspectInitialShow
                | MaskJourneyAction::PreferAlternateMask
                | MaskJourneyAction::InspectReplannedShow
                | MaskJourneyAction::InspectRestoredShow
        );
        NativeMaskJourneyObservation {
            action_id: action.id(),
            concrete_event,
            presentation_id: self.presentation.identity.as_str().into(),
            selected_mask_form_id: self
                .control
                .selected
                .as_ref()
                .map(|selected| selected.mask_form.expanded_form_id.as_str().into()),
            plan_id: self.control.active_plan_id.clone(),
            selected_route_id: self
                .control
                .selected
                .as_ref()
                .map(|selected| selected.route_id.clone()),
            show_id: retains_fresh_show
                .then(|| self.show.as_ref().map(|show| show.show_id.as_str().into()))
                .flatten(),
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
                self.control
                    .apply(
                        self.control.scoped_wardrobe.wardrobe.revision,
                        MaskWardrobeAction::Wear(mask),
                        &self.initial_routes,
                    )
                    .map_err(|_| ())?;
                "wore-alternate-mask"
            }
            MaskJourneyAction::PreferAlternateMask => {
                let mask = self.alternate.planned_mask.mask.form_identity.clone();
                // The primary route became unavailable within the already
                // sealed Body Plan. Preference now selects its admitted peer.
                self.initial_routes = admitted_routes(
                    &self.initial_plan,
                    &[&self.initial, &self.alternate],
                    &[false, true],
                )?;
                let evidence = self
                    .control
                    .apply(
                        self.control.scoped_wardrobe.wardrobe.revision,
                        MaskWardrobeAction::Prefer(vec![mask]),
                        &self.initial_routes,
                    )
                    .map_err(|_| ())?;
                if !matches!(
                    evidence.reconciliation.show,
                    MaskShowDisposition::SelectSealed { .. }
                ) {
                    return Err(());
                }
                let (show, receipts) = execute(
                    &self.alternate,
                    self.presentation,
                    self.presentation.revision.checked_add(1).ok_or(())?,
                    "same-plan-alternate",
                )?;
                self.show = Some(show);
                self.receipts = receipts;
                "preferred-and-selected-sealed-alternate-route"
            }
            MaskJourneyAction::WithdrawSelectedRoute => {
                self.initial_routes = admitted_routes(
                    &self.initial_plan,
                    &[&self.initial, &self.alternate],
                    &[false, false],
                )?;
                self.control = MaskWardrobeControl::new(
                    &self.initial_plan.body_id,
                    self.control.scoped_wardrobe.clone(),
                    &self.initial_plan,
                    &self.initial_routes,
                    self.control.selected.clone(),
                )
                .map_err(|_| ())?;
                if self.control.selected.is_some() {
                    return Err(());
                }
                self.show = None;
                self.receipts = vec!["native-route-withdrawn".into()];
                "withdrew-selected-native-route"
            }
            MaskJourneyAction::InspectUnavailableShow => "inspected-no-current-show",
            MaskJourneyAction::AddFaceHost => {
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
                let basis = self.control.active_plan_id.clone();
                let reconciliation = self
                    .control
                    .admit_replacement_plan(
                        &basis,
                        &self.replacement_plan,
                        &self.replacement_routes,
                    )
                    .map_err(|_| ())?;
                if !matches!(
                    reconciliation.show,
                    MaskShowDisposition::SelectSealed { .. }
                ) {
                    return Err(());
                }
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
                let evidence = self
                    .control
                    .apply(
                        self.control.scoped_wardrobe.wardrobe.revision,
                        MaskWardrobeAction::Doff(alternate),
                        &self.replacement_routes,
                    )
                    .map_err(|_| ())?;
                if !matches!(
                    evidence.reconciliation.show,
                    MaskShowDisposition::SelectSealed { .. }
                ) {
                    return Err(());
                }
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

fn body_plan_with_masks(
    wake: &Wake,
    application_plan: &BodyPlan,
    face: BodyFaceSelector,
    stages: &[&MaskStage],
) -> Result<BodyPlan, ()> {
    let chains = stages
        .iter()
        .map(|stage| BodyMaskChainPlan {
            plan: stage.planned_mask.plan.clone(),
            stage_placement_ids: vec![stage.planned_mask.show_placement().placement_id.clone()],
        })
        .collect();
    BodyPlan::seal_with_masks(
        wake,
        application_plan.forms.clone(),
        vec![BodyMaskTopology { face, chains }],
    )
    .map_err(|_| ())
}

fn admitted_routes(
    body_plan: &BodyPlan,
    stages: &[&MaskStage],
    availability: &[bool],
) -> Result<AdmittedMaskFormRoutes, ()> {
    if stages.len() != availability.len() {
        return Err(());
    }
    let routes = stages
        .iter()
        .zip(availability)
        .map(|(stage, available)| SealedMaskFormRoute {
            route_id: route_id(stage),
            mask_form: stage.planned_mask.mask.form_identity.clone(),
            plan_id: body_plan.plan_id.clone(),
            placement_ids: stage
                .planned_mask
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .map(|placement| placement.placement_id.clone())
                .collect(),
            currently_available: *available,
        })
        .collect();
    let masks = stages
        .iter()
        .map(|stage| stage.planned_mask.clone())
        .collect::<Vec<_>>();
    AdmittedMaskFormRoutes::new(body_plan, &masks, routes).map_err(|_| ())
}

fn route_id(stage: &MaskStage) -> String {
    format!("mask-route/{}", stage.planned_mask.plan.plan_id.as_str())
}

fn execute(
    stage: &MaskStage,
    presentation: &Presentation,
    play_sequence: u64,
    label: &str,
) -> Result<(MaskShow, Vec<String>), ()> {
    let receipt = crate::native_mask_play::run(&stage.planned_mask, presentation, play_sequence)
        .map_err(|_| ())?;
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
