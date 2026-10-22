//! Retained reading policy; Root supplies owner truth and admits every resulting effect.
use crate::{
    data::*,
    layout::{ApplicationViewport, PAGE_ROWS, rows},
};
use conduit_face::ApplicationView;
use conduit_human::{KeyEvent, KeyTransition};
#[derive(Default)]
pub struct Reader {
    current: Option<Current>,
    viewport: ApplicationViewport,
    revision: u64,
}
struct Current {
    scope: Scope,
    facts: Facts,
    view: Option<ApplicationView>,
}
impl Reader {
    fn expected(&self, expected: u64) -> Result<(), Refusal> {
        if self.revision != expected {
            Err(Refusal::Order)
        } else {
            Ok(())
        }
    }
    fn next(&self) -> Result<u64, Refusal> {
        self.revision.checked_add(1).ok_or(Refusal::Capacity)
    }
    pub fn update(&mut self, expected: u64, update: Update) -> Result<Snapshot, Refusal> {
        self.expected(expected)?;
        update.validate()?;
        let view = update
            .view
            .as_ref()
            .map(|bytes| ApplicationView::decode(bytes).map_err(|_| Refusal::Layout))
            .transpose()?;
        if self.current.as_ref().is_some_and(|old| {
            old.scope.session != update.scope.session
                || old.scope.host != update.scope.host
                || old.scope.boot != update.scope.boot
                || old.scope.outer_revision > update.scope.outer_revision
        }) {
            return Err(Refusal::Order);
        }
        let next = self.next()?;
        let changed = self
            .current
            .as_ref()
            .is_none_or(|old| old.view != view || old.scope.foreground != update.scope.foreground);
        if changed {
            self.viewport = ApplicationViewport::default();
        }
        self.current = Some(Current {
            scope: update.scope,
            facts: update.facts,
            view,
        });
        self.revision = next;
        self.snapshot()
    }
    pub fn snapshot(&self) -> Result<Snapshot, Refusal> {
        let current = self.current.as_ref().ok_or(Refusal::Order)?;
        let pages = current
            .view
            .as_ref()
            .map_or(0, |view| rows(view, |_, _| {}).div_ceil(PAGE_ROWS));
        Ok(Snapshot {
            scope: current.scope.clone(),
            reader_revision: self.revision,
            page: u32::try_from(self.viewport.page).map_err(|_| Refusal::Bounds)?,
            pages: u32::try_from(pages).map_err(|_| Refusal::Bounds)?,
            selected_node: self
                .viewport
                .selected_node
                .map(u16::try_from)
                .transpose()
                .map_err(|_| Refusal::Bounds)?,
        })
    }
    pub fn selection(&self, expected: u64) -> Result<Snapshot, Refusal> {
        self.expected(expected)?;
        self.snapshot()
    }
    pub fn reset(&mut self, expected: u64) -> Result<Snapshot, Refusal> {
        self.expected(expected)?;
        if self.current.is_none() {
            return Err(Refusal::Order);
        }
        self.revision = self.next()?;
        self.viewport = ApplicationViewport::default();
        self.snapshot()
    }
    pub fn navigate(&mut self, expected: u64, key: [u8; 3]) -> Result<(bool, Snapshot), Refusal> {
        self.expected(expected)?;
        let event = KeyEvent::decode(&key).map_err(|_| Refusal::Malformed)?;
        let current = self.current.as_ref().ok_or(Refusal::Order)?;
        if event.transition() != KeyTransition::Pressed
            || current.facts.details_open
            || current.view.is_none()
        {
            return Ok((false, self.snapshot()?));
        }
        // Reserve the fence before a fallible control operation can mutate state.
        let next = self.next()?;
        let changed = self
            .viewport
            .navigate(current.view.as_ref().ok_or(Refusal::Order)?, event.usage())?;
        if changed {
            self.revision = next;
        }
        Ok((changed, self.snapshot()?))
    }
    pub fn frame(
        &self,
        expected: u64,
    ) -> Result<(FrameReceipt, conduit_face::GraphicsScene), Refusal> {
        self.expected(expected)?;
        let current = self.current.as_ref().ok_or(Refusal::Order)?;
        let refusal = current
            .facts
            .refusal
            .as_ref()
            .map(|notice| (notice.heading.as_str(), notice.reason.as_str()));
        let scene = crate::scene::scene(
            &current.facts,
            &current.facts.foreground_title,
            current.view.as_ref(),
            self.viewport,
            current.facts.patchbay_graph_available,
            refusal,
            current.scope.width,
            current.scope.height,
        )?;
        Ok((
            FrameReceipt {
                snapshot: self.snapshot()?,
            },
            scene,
        ))
    }
}
