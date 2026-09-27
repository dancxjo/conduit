use super::*;
use conduit_presentation::{actualize_mask_journey, MaskJourneyAction, MaskJourneyEmbodiment};

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskJourneyOutcome {
    pub action_id: &'static str,
    pub concrete_event: &'static str,
    pub presentation_id: String,
    pub selected_mask_form_id: Option<String>,
    pub plan_id: conduit_core::PlanId,
    pub selected_route_id: Option<String>,
    pub show_id: Option<String>,
    pub receipt_ids: Vec<String>,
}

struct BrowserJourney<'a> {
    runtime: &'a BrowserMaskRuntime,
    replacement_plan_id: conduit_core::PlanId,
}

impl BrowserJourney<'_> {
    fn outcome(
        &self,
        action: MaskJourneyAction,
        concrete_event: &'static str,
        replacement: bool,
        route: Option<&'static str>,
        show: bool,
    ) -> BrowserMaskJourneyOutcome {
        let plan_id = if replacement {
            self.replacement_plan_id.clone()
        } else {
            self.runtime.planned.plan.plan_id.clone()
        };
        let selected_mask_form_id = route.map(|_| {
            self.runtime
                .planned
                .mask
                .form_identity
                .checked_form_id
                .as_str()
                .to_string()
        });
        let mut receipt_ids = vec![format!(
            "wardrobe-revision/{}",
            self.runtime.wardrobe_action.resulting_wardrobe.revision
        )];
        if show {
            receipt_ids.push(self.runtime.show.show.manifestation_id.as_str().into());
            if let Some(sign) = self.runtime.show.show.signs.last() {
                receipt_ids.push(sign.sign_id.as_str().into());
            }
            if let Some(interaction) = &self.runtime.interaction_receipt {
                receipt_ids.push(interaction.correlation.interaction.identity.as_str().into());
            }
        }
        BrowserMaskJourneyOutcome {
            action_id: action.id(),
            concrete_event,
            presentation_id: self.runtime.presentation.identity.as_str().into(),
            selected_mask_form_id,
            plan_id,
            selected_route_id: route.map(str::to_string),
            show_id: show.then(|| self.runtime.show.show_id.as_str().into()),
            receipt_ids,
        }
    }
}

impl MaskJourneyEmbodiment for BrowserJourney<'_> {
    type Outcome = BrowserMaskJourneyOutcome;
    type Error = String;

    fn perform(&mut self, action: MaskJourneyAction) -> Result<Self::Outcome, Self::Error> {
        if self.runtime.show.show.lifecycle != ManifestationLifecycle::Available {
            return Err("browser Mask journey requires exact DOM acknowledgement".into());
        }
        if self.runtime.interaction_receipt.is_none() {
            return Err("browser Mask journey requires correlated local interaction".into());
        }
        Ok(match action {
            MaskJourneyAction::InspectInitialShow => self.outcome(
                action,
                "dom-show-inspected",
                false,
                Some("route/browser-graphical"),
                true,
            ),
            MaskJourneyAction::WearAlternateMask => self.outcome(
                action,
                "wardrobe-wear-admitted",
                false,
                Some("route/browser-graphical"),
                true,
            ),
            MaskJourneyAction::PreferAlternateMask => self.outcome(
                action,
                "sealed-route-selected-without-plan-mutation",
                false,
                Some("route/browser-graphical-fallback"),
                true,
            ),
            MaskJourneyAction::WithdrawSelectedRoute => {
                self.outcome(action, "selected-route-withdrawn", false, None, false)
            }
            MaskJourneyAction::InspectUnavailableShow => {
                self.outcome(action, "no-current-show-inspected", false, None, false)
            }
            MaskJourneyAction::AddPresentationHost => self.outcome(
                action,
                "presentation-host-offer-observed",
                false,
                None,
                false,
            ),
            MaskJourneyAction::AdmitReplacementPlan => {
                self.outcome(action, "replacement-plan-admitted", true, None, false)
            }
            MaskJourneyAction::InspectReplannedShow => self.outcome(
                action,
                "replacement-dom-show-inspected",
                true,
                Some("route/browser-graphical-replacement"),
                true,
            ),
            MaskJourneyAction::DoffAlternateMask => self.outcome(
                action,
                "wardrobe-doff-admitted",
                true,
                Some("route/browser-graphical-restored"),
                true,
            ),
            MaskJourneyAction::InspectRestoredShow => self.outcome(
                action,
                "restored-dom-show-inspected",
                true,
                Some("route/browser-graphical-restored"),
                true,
            ),
        })
    }
}

pub(super) fn actualize(
    runtime: &BrowserMaskRuntime,
) -> Result<Vec<BrowserMaskJourneyOutcome>, String> {
    let replacement_plan_id = plan::planned_mask(
        runtime.play.host_id.clone(),
        BootId::from(format!("{}/replacement", runtime.play.boot_id.as_str())),
    )?
    .1
    .plan
    .plan_id;
    if replacement_plan_id == runtime.planned.plan.plan_id {
        return Err("replacement planning reused the immutable browser Mask Plan".into());
    }
    let mut embodiment = BrowserJourney {
        runtime,
        replacement_plan_id,
    };
    let mut outcomes = Vec::with_capacity(10);
    actualize_mask_journey(&mut embodiment, |_action, outcome| {
        outcomes.push(outcome.clone())
    })
    .map_err(|error| {
        format!(
            "browser Mask journey failed at {}: {}",
            error.action.id(),
            error.source
        )
    })?;
    Ok(outcomes)
}
