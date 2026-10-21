//! One owner of the unchanged canonical Tutorial, Tour and Patchbay controllers.
use crate::{EVENT_BYTES, SLOTS, VIEW_BYTES, data::*, tutorial::Tutorial};
use alloc::boxed::Box;
use patchbay_application::{PatchbayApplicationPort, PatchbayMaskTopology};
mod builder;
pub(crate) use builder::Builder;
use patchbay_graph::PatchbayGraph;
pub struct Patchbay {
    ports: [Option<Box<PatchbayApplicationPort>>; SLOTS],
    selected: usize,
}
enum Application {
    Tutorial(Tutorial),
    Tour(Box<conduit_tour_model::TourApplicationPort>),
    Patchbay(Patchbay),
}
pub struct Models {
    applications: [Option<Application>; SLOTS],
}
impl Models {
    pub fn new(initial: [Option<Initial>; SLOTS]) -> Result<Self, Refusal> {
        // Direct host-fixture construction uses the same staged constructors;
        // it is not an admitted aggregate command in the service protocol.
        let mut builder = Builder::new(InitManifest::from_initial(&initial))?;
        for (slot, initial) in initial.into_iter().enumerate() {
            match initial {
                None => {}
                Some(Initial::Tutorial { evidence }) => builder.tutorial(slot as u8, evidence)?,
                Some(Initial::Tour) => builder.tour(slot as u8)?,
                Some(Initial::Patchbay { targets, .. }) => {
                    for (target, value) in targets.into_iter().enumerate() {
                        if let Some(value) = value {
                            builder.target(slot as u8, target as u8, value)?;
                        }
                    }
                }
            }
        }
        builder.finish()
    }
    pub fn kind(&self, slot: usize) -> Option<Kind> {
        match self.applications.get(slot)?.as_ref()? {
            Application::Tutorial(_) => Some(Kind::Tutorial),
            Application::Tour(_) => Some(Kind::Tour),
            Application::Patchbay(_) => Some(Kind::Patchbay),
        }
    }
    pub fn graph(&self, slot: usize) -> Option<&PatchbayGraph> {
        let Application::Patchbay(p) = self.applications.get(slot)?.as_ref()? else {
            return None;
        };
        Some(p.ports[p.selected].as_ref()?.graph())
    }
    pub fn apply(
        &mut self,
        slot: usize,
        input: &[u8],
    ) -> Result<(Snapshot, Option<Proposal>), Refusal> {
        if input.len() > EVENT_BYTES {
            return Err(Refusal::Bounds);
        }
        let application = self
            .applications
            .get_mut(slot)
            .and_then(Option::as_mut)
            .ok_or(Refusal::Order)?;
        let (view, proposal, context) = match application {
            Application::Tutorial(p) => {
                let (v, r) = p.apply(input)?;
                (v, r, Context::None)
            }
            Application::Tour(p) => {
                let out = p.apply(input).map_err(|_| Refusal::Application)?;
                let context = Context::Tour {
                    chapter: p.controller().state().progress.chapter,
                    stage: p.controller().state().progress.stage,
                };
                let request = out.request.map(|r| match r {
                    conduit_tour_model::TourWorkspaceRequest::Run { chapter, stage } => {
                        Proposal::RunTour { chapter, stage }
                    }
                    conduit_tour_model::TourWorkspaceRequest::OpenPatchbay => {
                        Proposal::OpenPatchbay
                    }
                });
                (out.view, request, context)
            }
            Application::Patchbay(p) => {
                let out = p.ports[p.selected]
                    .as_mut()
                    .ok_or(Refusal::Order)?
                    .apply(input)
                    .map_err(|_| Refusal::Application)?;
                (
                    out.view,
                    out.request.map(Proposal::EditCurrent),
                    Context::None,
                )
            }
        };
        if view.len() > VIEW_BYTES {
            return Err(Refusal::Bounds);
        }
        Ok((
            Snapshot {
                slot: slot as u8,
                view,
                context,
            },
            proposal,
        ))
    }
    pub fn refresh(
        &mut self,
        evidence: &conduit_body::BodyBiographyEvidence,
    ) -> Result<(), Refusal> {
        for app in self.applications.iter_mut().flatten() {
            if let Application::Tutorial(p) = app {
                *p = Tutorial::prepare(evidence)?;
            }
        }
        Ok(())
    }
    pub fn select(&mut self, slot: usize, target: usize) -> Result<(), Refusal> {
        let Some(Application::Patchbay(p)) =
            self.applications.get_mut(slot).and_then(Option::as_mut)
        else {
            return Err(Refusal::Order);
        };
        if p.ports.get(target).is_none_or(Option::is_none) {
            return Err(Refusal::Bounds);
        }
        p.selected = target;
        Ok(())
    }
    pub fn topology(&mut self, topology: &PatchbayMaskTopology) -> Result<(), Refusal> {
        for app in self.applications.iter_mut().flatten() {
            if let Application::Patchbay(p) = app {
                for port in p.ports.iter_mut().flatten() {
                    port.set_mask_topology(topology.clone());
                }
            }
        }
        Ok(())
    }
    pub fn complete(
        &mut self,
        slot: usize,
        proof: conduit_tour_model::TourRunProof,
    ) -> Result<(), Refusal> {
        let Some(Application::Tour(p)) = self.applications.get_mut(slot).and_then(Option::as_mut)
        else {
            return Err(Refusal::Order);
        };
        p.complete_run(proof).map_err(|_| Refusal::Application)
    }
    pub fn snapshots(&mut self) -> Result<[Option<Snapshot>; SLOTS], Refusal> {
        let mut out = core::array::from_fn(|_| None);
        for (slot, target) in out.iter_mut().enumerate() {
            if self.kind(slot).is_some() {
                let (snapshot, proposal) = self.apply(slot, &[])?;
                if proposal.is_some() {
                    return Err(Refusal::Application);
                }
                *target = Some(snapshot);
            }
        }
        Ok(out)
    }
    pub fn execute(&mut self, command: Command) -> Result<Reply, Refusal> {
        match command {
            Command::PrepareInit { .. }
            | Command::PrepareTarget { .. }
            | Command::FinishTarget { .. }
            | Command::PrepareTutorial { .. }
            | Command::PrepareTour { .. }
            | Command::FinishInit
            | Command::ProjectTutorial { .. }
            | Command::PrepareGraphProjection { .. }
            | Command::FinishGraphProjection => Err(Refusal::Order),
            Command::Apply { slot, input } => {
                let (snapshot, proposal) = self.apply(usize::from(slot), &input)?;
                Ok(Reply::Applied { snapshot, proposal })
            }
            Command::Refresh { evidence } => {
                self.refresh(&evidence)?;
                Ok(Reply::Updated {
                    snapshots: self.snapshots()?,
                })
            }
            Command::Select { slot, target } => {
                self.select(usize::from(slot), usize::from(target))?;
                Ok(Reply::Updated {
                    snapshots: self.snapshots()?,
                })
            }
            Command::Topology { topology } => {
                self.topology(&topology)?;
                Ok(Reply::Updated {
                    snapshots: self.snapshots()?,
                })
            }
            Command::CompleteTour { slot, proof } => {
                self.complete(usize::from(slot), proof)?;
                Ok(Reply::Updated {
                    snapshots: self.snapshots()?,
                })
            }
            Command::Graph { slot } => Ok(Reply::Graph {
                graph: self.graph(usize::from(slot)).cloned(),
            }),
        }
    }
}
