use super::*;
use conduit_presentation::{
    actualize_mask_journey, MaskJourneyAction, MaskJourneyEmbodiment, MaskPlanningDisposition,
    MaskShowDisposition, SelectedMaskFormRoute,
};

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskJourneyOutcome {
    pub action_id: &'static str,
    pub concrete_event: String,
    pub presentation_id: String,
    pub selected_mask_form_id: Option<String>,
    pub plan_id: conduit_core::PlanId,
    pub selected_route_id: Option<String>,
    pub show_id: Option<String>,
    pub receipt_ids: Vec<String>,
}

struct BrowserJourney<'a> {
    initial: &'a BrowserMaskObservation,
    replacement: &'a BrowserMaskRuntime,
    alternate: MaskForm,
    initial_routes: AdmittedMaskFormRoutes,
    unavailable_routes: AdmittedMaskFormRoutes,
    replacement_routes: AdmittedMaskFormRoutes,
    control: MaskWardrobeControl,
    current_show: Option<String>,
}

impl BrowserJourney<'_> {
    fn retain(
        &self,
        action: MaskJourneyAction,
        event: impl Into<String>,
        receipt_ids: Vec<String>,
    ) -> BrowserMaskJourneyOutcome {
        BrowserMaskJourneyOutcome {
            action_id: action.id(),
            concrete_event: event.into(),
            presentation_id: self.initial.presentation.identity.as_str().into(),
            selected_mask_form_id: self
                .control
                .selected
                .as_ref()
                .map(|v| v.mask_form.checked_form_id.as_str().into()),
            plan_id: self.control.active_plan_id.clone(),
            selected_route_id: self.control.selected.as_ref().map(|v| v.route_id.clone()),
            show_id: self.current_show.clone(),
            receipt_ids,
        }
    }
    fn apply(
        &mut self,
        action: MaskWardrobeAction,
        routes: &AdmittedMaskFormRoutes,
    ) -> Result<MaskWardrobeControlEvidence, String> {
        self.control
            .apply(
                self.control.scoped_wardrobe.wardrobe.revision,
                action,
                routes,
            )
            .map_err(|e| format!("browser Mask wardrobe action: {e:?}"))
    }
}

impl MaskJourneyEmbodiment for BrowserJourney<'_> {
    type Outcome = BrowserMaskJourneyOutcome;
    type Error = String;
    fn perform(&mut self, action: MaskJourneyAction) -> Result<Self::Outcome, String> {
        Ok(match action {
            MaskJourneyAction::InspectInitialShow => {
                let interaction = self
                    .initial
                    .interaction
                    .as_ref()
                    .ok_or("initial browser Mask lacks correlated interaction")?;
                self.retain(
                    action,
                    "available DOM Show and interaction inspected",
                    vec![
                        self.initial.mask_show.show.manifestation_id.as_str().into(),
                        interaction.correlation.interaction.identity.as_str().into(),
                    ],
                )
            }
            MaskJourneyAction::WearAlternateMask => {
                let routes = self.initial_routes.clone();
                let e = self.apply(
                    MaskWardrobeAction::Wear(self.alternate.form_identity.clone()),
                    &routes,
                )?;
                self.retain(
                    action,
                    format!(
                        "wardrobe revision {} wore alternate Mask",
                        e.resulting_wardrobe.revision
                    ),
                    vec![format!(
                        "wardrobe-revision/{}",
                        e.resulting_wardrobe.revision
                    )],
                )
            }
            MaskJourneyAction::PreferAlternateMask => {
                let routes = self.initial_routes.clone();
                let e = self.apply(
                    MaskWardrobeAction::Prefer(vec![self.alternate.form_identity.clone()]),
                    &routes,
                )?;
                self.control = MaskWardrobeControl::new(
                    &e.body_id,
                    self.control.scoped_wardrobe.clone(),
                    &self.initial.body_plan,
                    &self.initial_routes,
                    None,
                )
                .map_err(|x| format!("select sealed fallback: {x:?}"))?;
                if self.control.selected.as_ref().map(|v| v.route_id.as_str())
                    != Some("route/browser-graphical-fallback")
                {
                    return Err("preference selected an unsealed route".into());
                }
                self.retain(
                    action,
                    "sealed fallback selected without Plan mutation",
                    vec![format!(
                        "wardrobe-revision/{}",
                        e.resulting_wardrobe.revision
                    )],
                )
            }
            MaskJourneyAction::WithdrawSelectedRoute => {
                self.control = MaskWardrobeControl::new(
                    &self.initial.wardrobe_action.body_id,
                    self.control.scoped_wardrobe.clone(),
                    &self.initial.body_plan,
                    &self.unavailable_routes,
                    self.control.selected.clone(),
                )
                .map_err(|e| format!("withdraw route: {e:?}"))?;
                if self.control.selected.is_some() {
                    return Err("withdrawn route remained selected".into());
                }
                self.current_show = None;
                self.retain(
                    action,
                    "selected sealed route became unavailable",
                    vec!["route-observation/withdrawn".into()],
                )
            }
            MaskJourneyAction::InspectUnavailableShow => {
                let r = self
                    .control
                    .scoped_wardrobe
                    .wardrobe
                    .reconcile(
                        &self.control.active_plan_id,
                        self.unavailable_routes.routes(),
                        None,
                    )
                    .map_err(|e| format!("inspect unavailable: {e:?}"))?;
                if !matches!(r.show, MaskShowDisposition::NoCurrentShow { .. })
                    || r.planning != MaskPlanningDisposition::ReplacementRequired
                {
                    return Err("unavailable route did not require replacement".into());
                }
                self.retain(
                    action,
                    "NoShow with replacement planning required",
                    vec!["show/no-current".into()],
                )
            }
            MaskJourneyAction::AddFaceHost => self.retain(
                action,
                format!(
                    "Host {} Boot {} offered replacement realization",
                    self.replacement.play.host_id.as_str(),
                    self.replacement.play.boot_id.as_str()
                ),
                vec![self.replacement.play.boot_id.as_str().into()],
            ),
            MaskJourneyAction::AdmitReplacementPlan => {
                let old = self.control.active_plan_id.clone();
                self.control
                    .admit_replacement_plan(
                        &old,
                        &self.replacement.body_plan,
                        &self.replacement_routes,
                    )
                    .map_err(|e| format!("admit replacement: {e:?}"))?;
                if self.control.active_plan_id == old {
                    return Err("replacement reused Plan".into());
                }
                self.current_show = None;
                self.retain(
                    action,
                    "fresh replacement Plan admitted",
                    vec![old.as_str().into()],
                )
            }
            MaskJourneyAction::InspectReplannedShow => {
                if self.replacement.show.show.lifecycle != ManifestationLifecycle::Available {
                    return Err("replacement Show lacks DOM acknowledgement".into());
                }
                self.current_show = Some(self.replacement.show.show_id.as_str().into());
                self.retain(
                    action,
                    "replacement DOM Show inspected",
                    vec![self.replacement.show.show.manifestation_id.as_str().into()],
                )
            }
            MaskJourneyAction::DoffAlternateMask => {
                let routes = self.replacement_routes.clone();
                let e = self.apply(
                    MaskWardrobeAction::Doff(self.alternate.form_identity.clone()),
                    &routes,
                )?;
                self.current_show = Some(self.replacement.show.show_id.as_str().into());
                self.retain(
                    action,
                    "alternate Mask doffed; sealed initial route restored",
                    vec![format!(
                        "wardrobe-revision/{}",
                        e.resulting_wardrobe.revision
                    )],
                )
            }
            MaskJourneyAction::InspectRestoredShow => self.retain(
                action,
                "restored acknowledged DOM Show inspected",
                vec![self.replacement.show.show.manifestation_id.as_str().into()],
            ),
        })
    }
}

pub(super) fn actualize(
    initial: &BrowserMaskObservation,
    replacement: &BrowserMaskRuntime,
) -> Result<Vec<BrowserMaskJourneyOutcome>, String> {
    if initial.presentation.identity != replacement.presentation.identity
        || initial.presentation.revision != replacement.presentation.revision
    {
        return Err("replacement changed Presentation truth".into());
    }
    if initial.body_plan.plan_id == replacement.body_plan.plan_id {
        return Err("replacement reused Plan".into());
    }
    let initial_mask = initial.planned_mask.mask.clone();
    let alternate = replacement.alternate.mask.clone();
    let initial_routes = plan::admitted_routes(
        &initial.body_plan,
        &initial.planned_mask,
        &initial.alternate,
        true,
        true,
    )?;
    let unavailable_routes = plan::admitted_routes(
        &initial.body_plan,
        &initial.planned_mask,
        &initial.alternate,
        false,
        false,
    )?;
    let replacement_routes = plan::admitted_routes(
        &replacement.body_plan,
        &replacement.planned,
        &replacement.alternate,
        true,
        true,
    )?;
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Body,
        vec![initial_mask.form_identity.clone()],
        vec![],
    )
    .map_err(|e| format!("initial wardrobe: {e:?}"))?;
    let scoped = BodyMaskWardrobe::new(initial.wardrobe_action.body_id.clone(), None, wardrobe)
        .map_err(|e| format!("scope wardrobe: {e:?}"))?;
    let selected = SelectedMaskFormRoute {
        route_id: "route/browser-graphical".into(),
        mask_form: initial_mask.form_identity.clone(),
        plan_id: initial.body_plan.plan_id.clone(),
    };
    let control = MaskWardrobeControl::new(
        &initial.wardrobe_action.body_id,
        scoped,
        &initial.body_plan,
        &initial_routes,
        Some(selected),
    )
    .map_err(|e| format!("start control: {e:?}"))?;
    let mut embodiment = BrowserJourney {
        initial,
        replacement,
        alternate,
        initial_routes,
        unavailable_routes,
        replacement_routes,
        control,
        current_show: Some(initial.mask_show.show_id.as_str().into()),
    };
    let mut outcomes = Vec::with_capacity(10);
    actualize_mask_journey(&mut embodiment, |_, outcome| outcomes.push(outcome.clone())).map_err(
        |e| {
            format!(
                "browser Mask journey failed at {}: {}",
                e.action.id(),
                e.source
            )
        },
    )?;
    Ok(outcomes)
}
