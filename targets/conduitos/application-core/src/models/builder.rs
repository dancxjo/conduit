//! Transaction-private preparation: no model/read/effect interface exists here.
use super::{Application, Models, Patchbay};
use crate::{SLOTS, data::*, tutorial::Tutorial};
use alloc::boxed::Box;
use conduit_body::BodyBiographyEvidence;
use patchbay_application::{PatchbayApplicationPort, PatchbayCanvasDelivery};

pub(crate) struct Builder {
    manifest: InitManifest,
    remaining: [u8; SLOTS],
    applications: [Option<Application>; SLOTS],
}
impl Builder {
    pub(crate) fn new(manifest: InitManifest) -> Result<Self, Refusal> {
        let mut remaining = [0; SLOTS];
        // Validate the entire fixed declaration before constructing controllers.
        for (slot, declared) in manifest.slots.iter().enumerate() {
            remaining[slot] = match declared {
                None => 0,
                Some(InitSlot::Tutorial | InitSlot::Tour) => 1,
                Some(InitSlot::Patchbay { targets, selected }) => {
                    if *selected as usize >= SLOTS
                        || *targets == 0
                        || *targets & !((1u8 << SLOTS) - 1) != 0
                        || *targets & (1 << selected) == 0
                    {
                        return Err(Refusal::Bounds);
                    }
                    *targets
                }
            };
        }
        Ok(Self {
            manifest,
            remaining,
            applications: core::array::from_fn(|slot| match manifest.slots[slot] {
                Some(InitSlot::Patchbay { selected, .. }) => {
                    Some(Application::Patchbay(Patchbay {
                        ports: core::array::from_fn(|_| None),
                        selected: selected as usize,
                    }))
                }
                _ => None,
            }),
        })
    }
    fn expected(&self, slot: u8, target: u8) -> Result<usize, Refusal> {
        let slot = usize::from(slot);
        let Some(next) = self.remaining.iter().position(|bits| *bits != 0) else {
            return Err(Refusal::Order);
        };
        if slot != next || u32::from(target) != self.remaining[next].trailing_zeros() {
            return Err(Refusal::Order);
        }
        Ok(slot)
    }
    pub(crate) fn target(&mut self, slot: u8, target: u8, value: Target) -> Result<(), Refusal> {
        let slot = self.expected(slot, target)?;
        let Some(InitSlot::Patchbay { .. }) = self.manifest.slots[slot] else {
            return Err(Refusal::Order);
        };
        let Some(Application::Patchbay(patchbay)) = &mut self.applications[slot] else {
            return Err(Refusal::Order);
        };
        let controller = PatchbayApplicationPort::open_graph(value.graph, value.plan, value.body)
            .map_err(|_| Refusal::Application)?
            .with_canvas_delivery(PatchbayCanvasDelivery::TextInspection);
        patchbay.ports[usize::from(target)] = Some(Box::new(controller));
        self.remaining[slot] &= !(1 << target);
        Ok(())
    }
    pub(crate) fn tutorial(
        &mut self,
        slot: u8,
        evidence: BodyBiographyEvidence,
    ) -> Result<(), Refusal> {
        let slot = self.expected(slot, 0)?;
        if self.manifest.slots[slot] != Some(InitSlot::Tutorial) {
            return Err(Refusal::Order);
        }
        self.applications[slot] = Some(Application::Tutorial(Tutorial::prepare(&evidence)?));
        self.remaining[slot] = 0;
        Ok(())
    }
    pub(crate) fn tour(&mut self, slot: u8) -> Result<(), Refusal> {
        let slot = self.expected(slot, 0)?;
        if self.manifest.slots[slot] != Some(InitSlot::Tour) {
            return Err(Refusal::Order);
        }
        self.applications[slot] = Some(Application::Tour(Box::new(
            conduit_tour_model::TourApplicationPort::canonical(),
        )));
        self.remaining[slot] = 0;
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Models, Refusal> {
        if self.remaining.iter().any(|bits| *bits != 0) {
            return Err(Refusal::Order);
        }
        // All semantic preparation already happened in the admitted component
        // entries. Finish moves custody; it never clones or rebuilds a graph.
        Ok(Models {
            applications: self.applications,
        })
    }
}
