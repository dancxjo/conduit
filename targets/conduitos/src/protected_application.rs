//! Root owns effect admission and immutable snapshots, never retained app ports.
mod authority;
mod projection;
mod transport;
use alloc::boxed::Box;
use conduit_face::ApplicationView;
use conduitos_application_core::{EVENT_BYTES, SLOTS, VIEW_BYTES, data::*};
use core::cell::{Cell, RefCell};
use patchbay_application::PatchbayMaskTopology;
use patchbay_graph::PatchbayGraph;
pub(crate) use projection::project_tutorial;
use transport::Service;

pub(crate) struct Owner {
    service: RefCell<Option<Service>>,
    failed: Cell<bool>,
    needed: bool,
    kinds: [Option<Kind>; SLOTS],
    snapshots: [Option<Snapshot>; SLOTS],
    graphs: [[Option<Box<PatchbayGraph>>; SLOTS]; SLOTS],
    selected: [u8; SLOTS],
    topology: Option<Box<PatchbayMaskTopology>>,
}
impl Owner {
    pub fn new(initial: [Option<Initial>; SLOTS]) -> Result<Self, Refusal> {
        let mut owner = Self {
            service: RefCell::new(None),
            failed: Cell::new(false),
            needed: initial.iter().any(Option::is_some),
            kinds: core::array::from_fn(|i| initial[i].as_ref().map(Initial::kind)),
            snapshots: core::array::from_fn(|_| None),
            graphs: core::array::from_fn(|_| core::array::from_fn(|_| None)),
            selected: [0; SLOTS],
            topology: None,
        };
        for (slot, initial) in initial.iter().enumerate() {
            if let Some(Initial::Patchbay { targets, selected }) = initial {
                if usize::from(*selected) >= SLOTS || targets[usize::from(*selected)].is_none() {
                    return Err(Refusal::Bounds);
                }
                owner.selected[slot] = *selected;
                for (index, target) in targets.iter().enumerate() {
                    owner.graphs[slot][index] =
                        target.as_ref().map(|target| Box::new(target.graph.clone()));
                }
            }
        }
        if owner.needed {
            *owner.service.get_mut() = Some(Service::install()?);
            if !matches!(
                owner.command(&Command::Init {
                    applications: initial
                })?,
                Reply::Initialized
            ) {
                owner.abort();
                return Err(Refusal::Malformed);
            }
            for slot in 0..SLOTS {
                if owner.kinds[slot] == Some(Kind::Patchbay) {
                    owner.verify_graph(slot)?;
                }
            }
        }
        Ok(owner)
    }
    pub fn kind(&self, slot: usize) -> Option<Kind> {
        if !self.is_live() {
            return None;
        }
        self.kinds.get(slot).copied().flatten()
    }
    pub fn is_live(&self) -> bool {
        !self.failed.get()
            && (!self.needed
                || self
                    .service
                    .try_borrow()
                    .is_ok_and(|service| service.is_some()))
    }
    pub fn abort(&mut self) {
        self.failed.set(true);
        if let Some(mut service) = self.service.get_mut().take() {
            service.quarantine();
        }
        self.needed = true;
        self.snapshots.fill_with(|| None);
        self.graphs
            .iter_mut()
            .for_each(|targets| targets.fill_with(|| None));
        self.topology = None;
    }
    fn command(&mut self, command: &Command) -> Result<Reply, Refusal> {
        if self.failed.get() {
            return Err(Refusal::Order);
        }
        let result = self
            .service
            .get_mut()
            .as_mut()
            .ok_or(Refusal::Order)?
            .command(command);
        if result.is_err() {
            self.abort();
        }
        result
    }
    fn guard<T>(&mut self, result: Result<T, Refusal>) -> Result<T, Refusal> {
        if result.is_err() {
            self.abort();
        }
        result
    }
    fn snapshot(&self, slot: usize, snapshot: &Snapshot) -> Result<(), Refusal> {
        if usize::from(snapshot.slot) != slot || snapshot.view.len() > VIEW_BYTES {
            return Err(Refusal::Bounds);
        }
        let kind = self.kind(slot).ok_or(Refusal::Order)?;
        let view = ApplicationView::decode(&snapshot.view).map_err(|_| Refusal::Malformed)?;
        authority::validate_context(kind, &view, &snapshot.context).map_err(|_| Refusal::Malformed)
    }
    fn commit_updated(&mut self, reply: Reply) -> Result<(), Refusal> {
        let result = (|| {
            let Reply::Updated { snapshots } = reply else {
                return Err(Refusal::Malformed);
            };
            for (slot, snapshot) in snapshots.iter().enumerate() {
                match (self.kinds[slot], snapshot) {
                    (Some(_), Some(snapshot)) => self.snapshot(slot, snapshot)?,
                    (None, None) => {}
                    _ => return Err(Refusal::Malformed),
                }
            }
            self.snapshots = snapshots;
            Ok(())
        })();
        self.guard(result)
    }
    pub fn apply(
        &mut self,
        slot: usize,
        input: &[u8],
    ) -> Result<(Snapshot, Option<Proposal>), Refusal> {
        let result = self.apply_inner(slot, input);
        self.guard(result)
    }
    fn apply_inner(
        &mut self,
        slot: usize,
        input: &[u8],
    ) -> Result<(Snapshot, Option<Proposal>), Refusal> {
        if input.len() > EVENT_BYTES {
            return Err(Refusal::Bounds);
        }
        let kind = self.kind(slot).ok_or(Refusal::Order)?;
        let prior = self.snapshots.get(slot).and_then(Option::as_ref).cloned();
        let Reply::Applied { snapshot, proposal } = self.command(&Command::Apply {
            slot: slot as u8,
            input: input.into(),
        })?
        else {
            return Err(Refusal::Malformed);
        };
        self.snapshot(slot, &snapshot)?;
        if input.is_empty() {
            if proposal.is_some() {
                return Err(Refusal::Malformed);
            }
        } else {
            let prior = prior.ok_or(Refusal::Order)?;
            let view = ApplicationView::decode(&prior.view).map_err(|_| Refusal::Malformed)?;
            authority::admit(
                kind,
                input,
                &view,
                &prior.context,
                self.graph(slot),
                self.topology.as_deref(),
                proposal.as_ref(),
            )
            .map_err(|_| Refusal::Malformed)?;
        }
        self.snapshots[slot] = Some(snapshot.clone());
        Ok((snapshot, proposal))
    }
    pub fn graph(&self, slot: usize) -> Option<&PatchbayGraph> {
        if !self.is_live() {
            return None;
        }
        self.graphs
            .get(slot)?
            .get(usize::from(*self.selected.get(slot)?))?
            .as_deref()
    }
    fn verify_graph(&mut self, slot: usize) -> Result<(), Refusal> {
        let result = (|| {
            let Reply::Graph { graph } = self.command(&Command::Graph { slot: slot as u8 })? else {
                return Err(Refusal::Malformed);
            };
            if graph.as_ref() != self.graph(slot) {
                return Err(Refusal::Malformed);
            }
            Ok(())
        })();
        self.guard(result)
    }
    pub fn refresh(
        &mut self,
        evidence: &conduit_body::BodyBiographyEvidence,
    ) -> Result<(), Refusal> {
        if !self.kinds.contains(&Some(Kind::Tutorial)) {
            return Ok(());
        }
        let reply = self.command(&Command::Refresh {
            evidence: evidence.clone(),
        })?;
        self.commit_updated(reply)
    }
    pub fn select(&mut self, slot: usize, target: usize) -> Result<(), Refusal> {
        if slot >= SLOTS || target >= SLOTS || self.graphs[slot][target].is_none() {
            return Err(Refusal::Bounds);
        }
        let reply = self.command(&Command::Select {
            slot: slot as u8,
            target: target as u8,
        })?;
        self.commit_updated(reply)?;
        self.selected[slot] = target as u8;
        self.verify_graph(slot)
    }
    pub fn topology(&mut self, topology: &PatchbayMaskTopology) -> Result<(), Refusal> {
        if !self.kinds.contains(&Some(Kind::Patchbay)) {
            return Ok(());
        }
        let reply = self.command(&Command::Topology {
            topology: topology.clone(),
        })?;
        self.commit_updated(reply)?;
        self.topology = Some(Box::new(topology.clone()));
        Ok(())
    }
    pub fn complete(
        &mut self,
        slot: usize,
        proof: conduit_tour_model::TourRunProof,
    ) -> Result<(), Refusal> {
        if slot >= SLOTS {
            return Err(Refusal::Bounds);
        }
        let reply = self.command(&Command::CompleteTour {
            slot: slot as u8,
            proof,
        })?;
        self.commit_updated(reply)
    }
}
