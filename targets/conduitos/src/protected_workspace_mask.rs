//! Root transport custody and admission of advisory Mask results.
//! This owner holds no selected-node/page model and grants no Show or input effect.
mod control;
mod transport;
use conduit_face::{ApplicationView, GraphicsScene, LayoutRect};
use conduitos_mask_core::data::{self, Command, Refusal, Reply, Scope, Snapshot, Update};
use transport::Service;
#[derive(Default)]
pub(crate) struct Owner {
    service: Option<Service>,
    failed: bool,
    revision: u64,
    control_revision: u64,
    extent: Option<(u16, u16)>,
    scope: Option<Scope>,
}
impl Owner {
    pub(crate) fn is_failed(&self) -> bool {
        self.failed
    }
    pub(crate) fn extent(&self) -> Option<(u16, u16)> {
        if self.failed { None } else { self.extent }
    }
    fn terminal(&mut self) {
        self.failed = true;
        self.extent = None;
        self.scope = None;
        if let Some(mut service) = self.service.take() {
            service.quarantine();
        }
    }
    fn service(&mut self) -> Result<&mut Service, Refusal> {
        if self.failed {
            return Err(Refusal::Order);
        }
        if self.service.is_none() {
            self.service = Some(Service::install()?);
        }
        self.service.as_mut().ok_or(Refusal::Order)
    }
    fn checked(
        &self,
        snapshot: &Snapshot,
        scope: &Scope,
        revision: u64,
        control_revision: u64,
        view: Option<&ApplicationView>,
    ) -> Result<(), Refusal> {
        if &snapshot.scope != scope
            || snapshot.reader_revision != revision
            || snapshot.pages as usize > conduitos_mask_core::MAX_VIEW_BYTES
            || snapshot.page >= snapshot.pages.max(1)
            || (view.is_none() && (snapshot.pages != 0 || snapshot.selected_node.is_some()))
            || snapshot.selected_node.is_some_and(|index| {
                view.and_then(|v| {
                    v.nodes
                        .get(usize::from(index))
                        .and_then(|node| conduitos_mask_core::layout::enabled_action(v, node))
                })
                .is_none()
            })
        {
            return Err(Refusal::Malformed);
        }
        Ok(())
    }
    fn update(
        &mut self,
        update: &mut Update,
        view: Option<&ApplicationView>,
    ) -> Result<(), Refusal> {
        update.scope.session = self.service()?.instance();
        update.validate()?;
        let next = self.revision.checked_add(1).ok_or(Refusal::Capacity)?;
        // Send owned finite facts; no state is accepted until its exact scope returns.
        let command = Command::Update {
            expected: self.revision,
            update: Update {
                scope: update.scope.clone(),
                facts: update.facts.clone(),
                view: update.view.clone(),
            },
        };
        let bytes = self.service()?.command(&command)?;
        let Reply::Updated { snapshot } = data::decode(&bytes)? else {
            return Err(Refusal::Malformed);
        };
        self.checked(&snapshot, &update.scope, next, view)?;
        self.revision = next;
        self.extent = Some((update.scope.width, update.scope.height));
        self.scope = Some(update.scope.clone());
        Ok(())
    }
    pub(crate) fn frame(
        &mut self,
        mut update: Update,
        view: Option<&ApplicationView>,
    ) -> Result<GraphicsScene, Refusal> {
        let result = (|| {
            self.update(&mut update, view)?;
            let command = Command::Frame {
                expected: self.revision,
            };
            let bytes = self.service()?.command(&command)?;
            let (receipt, scene) = data::decode_frame(&bytes)?;
            self.checked(&receipt.snapshot, &update.scope, self.revision, view)?;
            let clip = LayoutRect {
                x: 0,
                y: 0,
                width: update.scope.width,
                height: update.scope.height,
            };
            if scene.commands().iter().any(|command| command.clip != clip) {
                return Err(Refusal::Malformed);
            }
            Ok(scene)
        })();
        if result.is_err() {
            self.terminal();
        }
        result
    }
    pub(crate) fn selection(
        &mut self,
        mut update: Update,
        view: Option<&ApplicationView>,
    ) -> Result<Snapshot, Refusal> {
        let result = (|| {
            self.update(&mut update, view)?;
            let command = Command::Selection {
                expected: self.revision,
            };
            let bytes = self.service()?.command(&command)?;
            let Reply::Selected { snapshot } = data::decode(&bytes)? else {
                return Err(Refusal::Malformed);
            };
            self.checked(&snapshot, &update.scope, self.revision, view)?;
            Ok(snapshot)
        })();
        if result.is_err() {
            self.terminal();
        }
        result
    }
    pub(crate) fn navigate(
        &mut self,
        mut update: Update,
        view: Option<&ApplicationView>,
        usage: u8,
    ) -> Result<bool, Refusal> {
        let result = (|| {
            self.update(&mut update, view)?;
            let command = Command::Navigate {
                expected: self.revision,
                key: conduit_human::KeyEvent::new(
                    usage,
                    conduit_human::KeyTransition::Pressed,
                    conduit_human::KeyModifiers::NONE,
                )
                .map_err(|_| Refusal::Malformed)?
                .encode(),
            };
            let bytes = self.service()?.command(&command)?;
            let Reply::Navigated { changed, snapshot } = data::decode(&bytes)? else {
                return Err(Refusal::Malformed);
            };
            let next = if changed {
                self.revision.checked_add(1).ok_or(Refusal::Capacity)?
            } else {
                self.revision
            };
            self.checked(&snapshot, &update.scope, next, view)?;
            self.revision = next;
            Ok(changed)
        })();
        if result.is_err() {
            self.terminal();
        }
        result
    }
    pub(crate) fn reset(&mut self) {
        if self.extent().is_none() {
            return;
        }
        let result = (|| {
            let next = self.revision.checked_add(1).ok_or(Refusal::Capacity)?;
            let command = Command::Reset {
                expected: self.revision,
            };
            let bytes = self.service()?.command(&command)?;
            let Reply::Updated { snapshot } = data::decode(&bytes)? else {
                return Err(Refusal::Malformed);
            };
            if self.scope.as_ref() != Some(&snapshot.scope)
                || snapshot.reader_revision != next
                || snapshot.page != 0
                || snapshot.selected_node.is_some()
            {
                return Err(Refusal::Malformed);
            }
            self.revision = next;
            Ok::<(), Refusal>(())
        })();
        if result.is_err() {
            self.terminal();
        }
    }
}
