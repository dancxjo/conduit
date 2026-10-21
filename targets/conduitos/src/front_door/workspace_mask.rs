//! Observational facts and native Mask transport; effect and Show admission stay in Root.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use super::{Error, FrontDoor};
use crate::product_journey::{JourneyProjection, JourneyStatus};
use conduitos_mask_core::data::{Facts, Notice, PresentationStatus};

pub(super) fn facts(
    journey: &JourneyProjection,
    title: &str,
    graph: bool,
    refusal: Option<(&str, &str)>,
    details_open: bool,
) -> Facts {
    let status = match journey.status {
        JourneyStatus::World => PresentationStatus::World,
        JourneyStatus::PlotOpened => PresentationStatus::PlotOpened,
        JourneyStatus::BornLulled => PresentationStatus::BornLulled,
        JourneyStatus::Awake => PresentationStatus::Awake,
        JourneyStatus::Planned => PresentationStatus::Planned,
        JourneyStatus::QuiescentAwaitingInput => PresentationStatus::QuiescentAwaitingInput,
        JourneyStatus::SemanticCompleted => PresentationStatus::SemanticCompleted,
        JourneyStatus::Lulled => PresentationStatus::Lulled,
        JourneyStatus::Fulfilled => PresentationStatus::Fulfilled,
        JourneyStatus::InputUnavailable => PresentationStatus::InputUnavailable,
        JourneyStatus::Stopped => PresentationStatus::Stopped,
    };
    Facts {
        friendly_name: journey.friendly_name.clone(),
        foreground_title: title.into(),
        status,
        loss_recovery: journey.loss_kind.map(|loss| loss.recovery().into()),
        result: journey.result.clone(),
        result_omitted_bytes: journey.result_omitted_bytes,
        patchbay_graph_available: graph,
        refusal: refusal.map(|(heading, reason)| Notice {
            heading: heading.into(),
            reason: reason.into(),
        }),
        details_open,
    }
}

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
impl FrontDoor {
    // An uninstalled owner is healthy before the first Show. A failed or busy
    // owner cannot authorize cached workspace views or a new actionable Face.
    pub(super) fn workspace_mask_healthy(&self) -> bool {
        self.workspace_mask
            .try_borrow()
            .is_ok_and(|owner| !owner.is_failed())
    }
    fn mask_update(
        &self,
        width: u16,
        height: u16,
    ) -> Result<conduitos_mask_core::data::Update, Error> {
        use conduitos_mask_core::data::{Scope, Update, view_digest};
        let journey = self.journey.as_ref().ok_or(Error::Scene)?;
        let foreground = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.plots.iter().find(|plot| plot.foreground))
            .ok_or(Error::Scene)?;
        let view = self
            .application_view
            .as_ref()
            .map(|view| view.encode().map_err(|_| Error::Face))
            .transpose()?;
        Ok(Update {
            scope: Scope {
                session: 0,
                host: self.host_id.clone(),
                boot: self.boot_id.clone(),
                body: journey.body_id.clone(),
                journey_revision: journey.revision,
                outer_revision: self.revision,
                view_digest: view_digest(view.as_deref()),
                foreground: foreground.plot.clone(),
                width,
                height,
            },
            facts: facts(
                journey,
                foreground.title,
                self.patchbay_graph.is_some(),
                self.refusal.as_ref().map(|r| (r.heading(), r.reason())),
                self.exact_details_open,
            ),
            view,
        })
    }
    pub(super) fn workspace_mask_scene(
        &self,
        display: &impl crate::display::PixelTarget,
    ) -> Result<conduit_face::GraphicsScene, Error> {
        let format = display.format().validate().map_err(Error::Display)?;
        let width = u16::try_from(format.width).map_err(|_| Error::Scene)?;
        let height = u16::try_from(format.height).map_err(|_| Error::Scene)?;
        let update = self.mask_update(width, height)?;
        self.workspace_mask
            .try_borrow_mut()
            .map_err(|_| Error::Scene)?
            .frame(update, self.application_view.as_ref())
            .map_err(|_| Error::Scene)
    }
    fn mask_selection(&self) -> Option<conduitos_mask_core::data::Snapshot> {
        let (width, height) = self.workspace_mask.try_borrow().ok()?.extent()?;
        let update = self.mask_update(width, height).ok()?;
        self.workspace_mask
            .try_borrow_mut()
            .ok()?
            .selection(update, self.application_view.as_ref())
            .ok()
    }
    pub fn application_pages(&self) -> usize {
        self.mask_selection()
            .map_or(0, |snapshot| snapshot.pages as usize)
    }
    pub fn application_page(&self) -> usize {
        self.mask_selection()
            .map_or(0, |snapshot| snapshot.page as usize)
    }
    pub fn application_view(&self) -> Option<&conduit_face::ApplicationView> {
        self.workspace_mask_healthy().then_some(())?;
        self.application_view.as_ref()
    }
    pub fn reset_application_navigation(&mut self) {
        if let Ok(mut owner) = self.workspace_mask.try_borrow_mut() {
            owner.reset();
        }
    }
    pub fn selected_application_action(&self) -> Option<&conduit_face::ApplicationAction> {
        let snapshot = self.mask_selection()?;
        let view = self.application_view.as_ref()?;
        let node = view.nodes.get(usize::from(snapshot.selected_node?))?;
        conduitos_mask_core::layout::enabled_action(view, node)
    }
    pub fn navigate_application(&mut self, usage: u8, revision: u64) -> Result<bool, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        if !matches!(usage, 75 | 78 | 81 | 82) {
            return Ok(false);
        }
        let owner = self.workspace_mask.try_borrow().map_err(|_| Error::Scene)?;
        if owner.is_failed() {
            return Err(Error::Scene);
        }
        let extent = owner.extent();
        drop(owner);
        let Some((width, height)) = extent else {
            return Ok(false);
        };
        let update = self.mask_update(width, height)?;
        let changed = self
            .workspace_mask
            .try_borrow_mut()
            .map_err(|_| Error::Scene)?
            .navigate(update, self.application_view.as_ref(), usage)
            .map_err(|_| Error::Scene)?;
        if changed {
            self.advance()?;
        }
        Ok(changed)
    }
}
