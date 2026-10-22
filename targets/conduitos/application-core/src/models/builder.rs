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
                        port: None,
                        targets: [None; SLOTS],
                        selected: selected as usize,
                        publication: None,
                        publication_sequence: 0,
                        canonical: false,
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
    pub(crate) fn validate_target(&self, slot: u8, target: u8) -> Result<(), Refusal> {
        let index = self.expected(slot, target)?;
        if !matches!(self.manifest.slots[index], Some(InitSlot::Patchbay { .. })) {
            return Err(Refusal::Order);
        }
        Ok(())
    }
    pub(crate) fn target(&mut self, slot: u8, target: u8, value: Target) -> Result<(), Refusal> {
        self.validate_target(slot, target)?;
        let slot = usize::from(slot);
        let Some(Application::Patchbay(patchbay)) = &mut self.applications[slot] else {
            return Err(Refusal::Order);
        };
        let index = if let Some(port) = patchbay.port.as_mut() {
            port.append_prepared_graph(value.graph, value.plan, &value.body)
                .map_err(|_| Refusal::Application)?
        } else {
            let port = PatchbayApplicationPort::open_graph(value.graph, value.plan, value.body)
                .and_then(|port| port.with_prepared_graph_capacity(SLOTS))
                .map_err(|_| Refusal::Application)?
                .with_canvas_delivery(PatchbayCanvasDelivery::TextInspection);
            patchbay.port = Some(Box::new(port));
            0
        };
        patchbay.targets[index] = Some(target);
        if usize::from(target) == patchbay.selected {
            patchbay
                .port
                .as_mut()
                .ok_or(Refusal::Order)?
                .select_prepared_plot(index)
                .map_err(|_| Refusal::Application)?;
        }
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
