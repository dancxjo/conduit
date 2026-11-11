//! Private original IMAGE/Play custody for the existing finite shared queue.
//! Requested Rust backing only; allocator rounding and process RSS are separate.
use super::*;
use crate::{kernel_preparation::ImageMemoryLease, StdHost};
use conduit_body::BodyPlayIdentity;
use conduit_core::{BootId, HostId, OfferGeneration, PlanFragment};

#[derive(Debug)]
pub(crate) struct ImageLiveAdmission {
    pub(crate) host: HostId,
    pub(crate) boot: BootId,
    pub(crate) offer: OfferGeneration,
    pub(crate) play: BodyPlayIdentity,
    // Drop binding strings before releasing the original shared charge.
    pub(crate) memory: ImageMemoryLease,
}
impl ImageLiveAdmission {
    fn binding_bytes(&self) -> Option<usize> {
        self.host
            .0
            .capacity()
            .checked_add(self.boot.0.capacity())?
            .checked_add(self.play.active_play_id.0.capacity())?
            .checked_add(self.play.body_id.retained_backing_bytes())?
            .checked_add(self.play.wake_id.retained_backing_bytes())?
            .checked_add(self.play.plan_id.0.capacity())
    }
}
impl BodyLiveForeQueue {
    /// One unseeded closing Flow on an already sealed local fragment. This
    /// allocates finite reusable staging only; callers own construction/producer
    /// funding and execution admission. Queue acceptance is not kernel commit.
    pub fn for_local_closing_flow_fragment(
        fragment: &PlanFragment,
        flow: &str,
        control: RunControl,
        queue_slots: usize,
        maximum_items: u16,
    ) -> Result<Self, String> {
        if !conduit_core::verify_plan_fragment(fragment)
            || queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || maximum_items == 0
            || maximum_items > 64
            || queue_slots > usize::from(maximum_items)
        {
            return Err("live Fore requires a sealed fragment and explicit finite queue".into());
        }
        let mut inputs = fragment
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input);
        let port = inputs.next().ok_or("live closing Flow missing")?;
        if inputs.next().is_some()
            || port.front_port_id.as_str() != flow
            || port.track != ConnectionTrack::Payload
            || port.temporal != (PortTemporal::Flow { closes: true })
            || port.selected_line.is_some()
            || port.abnormal_kind.is_some()
            || port.byte_capacity == 0
            || port.byte_capacity > 131_072
        {
            return Err("live queue requires one exact local unseeded closing Flow".into());
        }
        Ok(Self {
            plan_id: fragment.plan_id.clone(),
            fragment_id: Some(fragment.fragment_id.clone()),
            port: port.clone(),
            maximum_items,
            control,
            initial: None,
            wait_timeout: None,
            state: Arc::new(Mutex::new(QueueState {
                slots: (0..queue_slots)
                    .map(|_| Vec::with_capacity(port.byte_capacity as usize))
                    .collect(),
                free: (0..queue_slots).collect(),
                queued: VecDeque::with_capacity(queue_slots),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
                play_started: false,
                started_at: None,
                idle_stop: None,
                image_admission: None,
            })),
        })
    }
    pub(crate) fn image_clone_metadata_bytes(&self) -> Option<usize> {
        if self.initial.is_some() {
            return None;
        }
        let mut bytes = self
            .plan_id
            .0
            .capacity()
            .checked_add(self.fragment_id.as_ref().map_or(0, |id| id.0.capacity()))?;
        for value in [
            &self.port.front_port_id.0,
            &self.port.placement_id.0,
            &self.port.gear_port_id.0,
            &self.port.value_kind.0,
        ] {
            bytes = bytes.checked_add(value.capacity())?;
        }
        bytes = bytes.checked_add(
            self.port
                .abnormal_kind
                .as_ref()
                .map_or(0, |id| id.0.capacity()),
        )?;
        if self.port.selected_line.is_some() {
            return None;
        }
        if let Some(contract) = &self.port.value_contract {
            bytes = bytes.checked_add(contract.retained_backing_bytes()?)?;
        }
        Some(bytes)
    }
    pub(crate) fn image_shared_backing_bytes(&self) -> Option<usize> {
        let state = self.state.lock().ok()?;
        if state.play_started
            || state.submitted != 0
            || state.close_requested
            || state.kernel_closed
            || state.play_terminal
            || !state.queued.is_empty()
        {
            return None;
        }
        let mut bytes = core::mem::size_of::<Mutex<QueueState>>()
            .checked_add(2 * core::mem::size_of::<usize>())?
            .checked_add(
                state
                    .slots
                    .capacity()
                    .checked_mul(core::mem::size_of::<Vec<u8>>())?,
            )?
            .checked_add(
                state
                    .free
                    .capacity()
                    .checked_mul(core::mem::size_of::<usize>())?,
            )?
            .checked_add(
                state
                    .queued
                    .capacity()
                    .checked_mul(core::mem::size_of::<usize>())?,
            )?;
        for slot in &state.slots {
            bytes = bytes.checked_add(slot.capacity())?;
        }
        if let Some(admission) = &state.image_admission {
            bytes = bytes.checked_add(admission.binding_bytes()?)?;
        }
        Some(bytes)
    }
    pub(crate) fn bind_image_admission(&self, admission: ImageLiveAdmission) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "live queue lock poisoned")?;
        if state.image_admission.is_some() || state.play_started || state.submitted != 0 {
            return Err("live IMAGE queue already bound or started".into());
        }
        state.image_admission = Some(admission);
        Ok(())
    }
    pub(crate) fn verify_image_admission(
        &self,
        host: &StdHost,
        play: &BodyPlayIdentity,
        control: &RunControl,
    ) -> Result<(), String> {
        let state = self.state.lock().map_err(|_| "live queue lock poisoned")?;
        let admission = state
            .image_admission
            .as_ref()
            .ok_or("live Fore lacks original IMAGE queue custody")?;
        host.kernel_resources
            .require_input_memory_owner(&admission.memory)?;
        if admission.host != host.advertisement.host_id
            || admission.boot != host.advertisement.boot_id
            || admission.offer != host.advertisement.offer_generation
            || &admission.play != play
            || !self.control.same_source(control)
            || state.play_started
            || state.play_terminal
        {
            return Err(
                "live queue differs from original IMAGE Host/Boot/offer/Body Play/control".into(),
            );
        }
        Ok(())
    }
    pub(crate) fn has_image_admission(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.image_admission.is_some())
    }
    pub(crate) fn shrink_image_retention(&self, bytes: usize) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "live queue lock poisoned")?;
        state
            .image_admission
            .as_mut()
            .ok_or("live queue lacks IMAGE admission")?
            .memory
            .retain_result(bytes);
        Ok(())
    }
    pub(crate) fn require_image_started(&self) -> Result<(), String> {
        let state = self.state.lock().map_err(|_| "live queue lock poisoned")?;
        if state.image_admission.is_none() || !state.play_started || state.play_terminal {
            return Err("IMAGE live producer requires its running admitted Body Play".into());
        }
        Ok(())
    }
    pub(crate) fn wait_until_image_started(&self) -> Result<(), String> {
        loop {
            let observed = self.control.activity_generation();
            let state = self.state.lock().map_err(|_| "live queue lock poisoned")?;
            if state.play_started && !state.play_terminal {
                return Ok(());
            }
            if state.play_terminal || self.control.stop_requested() {
                return Err("IMAGE live Play ended before start".into());
            }
            drop(state);
            self.control.wait_for_activity_or_stop(observed);
        }
    }
}
