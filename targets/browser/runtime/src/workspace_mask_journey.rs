use super::*;
use conduit_presentation::{
    actualize_mask_journey, MaskJourneyAction, MaskJourneyEmbodiment, MaskPlanningDisposition,
    MaskShowDisposition, SelectedMaskPlotRoute,
};

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskJourneyOutcome {
    pub action_id: &'static str,
    pub concrete_event: String,
    pub presentation_id: String,
    pub selected_mask_plot_id: Option<String>,
    pub plan_id: conduit_core::PlanId,
    pub selected_route_id: Option<String>,
    pub show_id: Option<String>,
    pub receipt_ids: Vec<String>,
}

struct BrowserJourney<'a> {
    initial: &'a BrowserMaskObservation,
    alternate_show: &'a BrowserMaskObservation,
    replacement: &'a BrowserMaskObservation,
    restored: &'a BrowserMaskObservation,
    alternate: MaskPlot,
    initial_routes: AdmittedMaskPlotRoutes,
    unavailable_routes: AdmittedMaskPlotRoutes,
    replacement_routes: AdmittedMaskPlotRoutes,
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
        let retains_fresh_show = matches!(
            action,
            MaskJourneyAction::InspectInitialShow
                | MaskJourneyAction::PreferAlternateMask
                | MaskJourneyAction::InspectReplannedShow
                | MaskJourneyAction::InspectRestoredShow
        );
        BrowserMaskJourneyOutcome {
            action_id: action.id(),
            concrete_event: event.into(),
            presentation_id: self.initial.presentation.identity.as_str().into(),
            selected_mask_plot_id: self
                .control
                .selected
                .as_ref()
                .map(|v| v.mask_plot.checked_plot_id.as_str().into()),
            plan_id: self.control.active_plan_id.clone(),
            selected_route_id: self.control.selected.as_ref().map(|v| v.route_id.clone()),
            show_id: retains_fresh_show
                .then(|| self.current_show.clone())
                .flatten(),
            receipt_ids,
        }
    }
    fn apply(
        &mut self,
        action: MaskWardrobeAction,
        routes: &AdmittedMaskPlotRoutes,
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
                    MaskWardrobeAction::Wear(self.alternate.plot_identity.clone()),
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
                    MaskWardrobeAction::Prefer(vec![self.alternate.plot_identity.clone()]),
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
                self.current_show = Some(self.alternate_show.mask_show.show_id.as_str().into());
                self.retain(
                    action,
                    "sealed fallback executed as a fresh acknowledged DOM Show without Plan mutation",
                    vec![
                        format!("wardrobe-revision/{}", e.resulting_wardrobe.revision),
                        self.alternate_show
                            .mask_show
                            .show
                            .manifestation_id
                            .as_str()
                            .into(),
                    ],
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
                    self.replacement.mask_play.host_id.as_str(),
                    self.replacement.mask_play.boot_id.as_str()
                ),
                vec![self.replacement.mask_play.boot_id.as_str().into()],
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
                if self.replacement.mask_show.show.lifecycle != ManifestationLifecycle::Available {
                    return Err("replacement Show lacks DOM acknowledgement".into());
                }
                self.current_show = Some(self.replacement.mask_show.show_id.as_str().into());
                self.retain(
                    action,
                    "replacement DOM Show inspected",
                    vec![self
                        .replacement
                        .mask_show
                        .show
                        .manifestation_id
                        .as_str()
                        .into()],
                )
            }
            MaskJourneyAction::DoffAlternateMask => {
                let routes = self.replacement_routes.clone();
                let e = self.apply(
                    MaskWardrobeAction::Doff(self.alternate.plot_identity.clone()),
                    &routes,
                )?;
                self.current_show = None;
                self.retain(
                    action,
                    "alternate Mask doffed; sealed initial route restored",
                    vec![format!(
                        "wardrobe-revision/{}",
                        e.resulting_wardrobe.revision
                    )],
                )
            }
            MaskJourneyAction::InspectRestoredShow => {
                if self.restored.mask_show.show.lifecycle != ManifestationLifecycle::Available {
                    return Err("restored Show lacks DOM acknowledgement".into());
                }
                self.current_show = Some(self.restored.mask_show.show_id.as_str().into());
                self.retain(
                    action,
                    "restored acknowledged DOM Show inspected",
                    vec![self
                        .restored
                        .mask_show
                        .show
                        .manifestation_id
                        .as_str()
                        .into()],
                )
            }
        })
    }
}

pub(super) fn actualize(
    observations: &[BrowserMaskObservation],
) -> Result<Vec<BrowserMaskJourneyOutcome>, String> {
    let [initial, alternate_show, replacement, restored] = observations else {
        return Err("browser Mask journey requires four acknowledged Show occurrences".into());
    };
    if observations.iter().any(|observation| {
        observation.presentation.identity != initial.presentation.identity
            || observation.presentation.revision != initial.presentation.revision
            || observation.mask_show.show.lifecycle != ManifestationLifecycle::Available
    }) {
        return Err(
            "browser Mask journey changed Face truth or retained an unacknowledged Show".into(),
        );
    }
    if initial.body_plan.plan_id != alternate_show.body_plan.plan_id
        || initial.body_plan.plan_id == replacement.body_plan.plan_id
        || replacement.body_plan.plan_id != restored.body_plan.plan_id
    {
        return Err(
            "browser Mask journey did not preserve its same-Plan and replacement boundaries".into(),
        );
    }
    if observations
        .iter()
        .map(|observation| observation.mask_show.show_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != observations.len()
    {
        return Err("browser Mask journey reused a retained Show occurrence".into());
    }
    if initial.body_plan.plan_id == replacement.body_plan.plan_id {
        return Err("replacement reused Plan".into());
    }
    let initial_mask = initial.planned_mask.mask.clone();
    let alternate = initial.alternate.mask.clone();
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
        &replacement.alternate,
        &replacement.planned_mask,
        true,
        true,
    )?;
    let wardrobe = MaskWardrobe::new(
        MaskWardrobeLifetime::Body,
        vec![initial_mask.plot_identity.clone()],
        vec![],
    )
    .map_err(|e| format!("initial wardrobe: {e:?}"))?;
    let scoped = BodyMaskWardrobe::new(initial.wardrobe_action.body_id.clone(), None, wardrobe)
        .map_err(|e| format!("scope wardrobe: {e:?}"))?;
    let selected = SelectedMaskPlotRoute {
        route_id: "route/browser-graphical".into(),
        mask_plot: initial_mask.plot_identity.clone(),
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
        alternate_show,
        replacement,
        restored,
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
