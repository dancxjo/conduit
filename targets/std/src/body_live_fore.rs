//! One finite typed Fore queue shared by external action producers and a Body Play.
//! Producers do not own a scheduler or reconstruct Plot state.

use crate::RunControl;
use conduit_core::{
    verify_plan, ConnectionTrack, Plan, PlanId, PlannedActivationEntry, PlannedForePort,
    PortDirection, PortTemporal,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub const MAX_LIVE_FORE_SLOTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyLiveForeAdmission {
    /// Accepted into finite Host staging, before kernel ingress or Todo state.
    Accepted {
        sequence: u64,
    },
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyLiveForeStatus {
    pub queue_accepted: u16,
    pub kernel_admitted: u16,
    pub queued: u16,
    pub close_requested: bool,
    pub kernel_closed: bool,
    pub play_terminal: bool,
}

#[derive(Debug, Clone)]
pub struct BodyLiveForeQueue {
    plan_id: PlanId,
    port: PlannedForePort,
    maximum_items: u16,
    control: RunControl,
    state: Arc<Mutex<QueueState>>,
}

#[derive(Debug)]
struct QueueState {
    slots: Vec<Vec<u8>>,
    free: Vec<usize>,
    queued: VecDeque<usize>,
    submitted: u16,
    kernel_admitted: u16,
    close_requested: bool,
    kernel_closed: bool,
    play_terminal: bool,
}

impl BodyLiveForeQueue {
    /// Admit a queue for the exact sealed local Todo Flow before Play. Its
    /// finite slots are allocated here, never while advancing the scheduler.
    pub fn for_todo_plan(
        plan: &Plan,
        control: RunControl,
        queue_slots: usize,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 || plan.activations.len() != 1 {
            return Err("live Todo Fore requires one complete sealed local scan Plan".into());
        }
        let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
            return Err("live Todo Fore requires a scan activation".into());
        };
        let maximum_items = scan.limits.maximum_items;
        if maximum_items == 0
            || maximum_items > 64
            || queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || queue_slots > usize::from(maximum_items)
        {
            return Err("live Todo Fore queue exceeds its admitted bounds".into());
        }
        let inputs = plan.fragments[0]
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        let Some(port) = inputs.first().filter(|_| inputs.len() == 1) else {
            return Err("live Todo Fore requires one exact input port".into());
        };
        if port.track != ConnectionTrack::Payload
            || !matches!(port.temporal, PortTemporal::Flow { closes: true })
            || port.selected_line.is_some()
            || port.abnormal_kind.is_some()
            || port.byte_capacity == 0
            || port.byte_capacity > 75
        {
            return Err("live Todo Fore requires a local bounded Value Flow".into());
        }
        let mut slots = Vec::with_capacity(queue_slots);
        let mut free = Vec::with_capacity(queue_slots);
        for index in 0..queue_slots {
            slots.push(Vec::with_capacity(port.byte_capacity as usize));
            free.push(index);
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: (*port).clone(),
            maximum_items,
            control,
            state: Arc::new(Mutex::new(QueueState {
                slots,
                free,
                queued: VecDeque::with_capacity(queue_slots),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
            })),
        })
    }

    pub fn submit(&self, canonical: &[u8]) -> Result<BodyLiveForeAdmission, String> {
        if self.control.stop_requested() {
            return Err("live Todo Fore Play is cancelling".into());
        }
        if canonical.len() > self.port.byte_capacity as usize {
            return Err("live Todo Fore value exceeds its sealed byte bound".into());
        }
        if let Some(contract) = &self.port.value_contract {
            contract
                .validate(canonical)
                .map_err(|error| format!("live Todo Fore value contract: {error:?}"))?;
        } else {
            conduit_core::validate_primitive_info(self.port.value_kind.as_str(), canonical)
                .map_err(|error| format!("live Todo Fore value kind: {error:?}"))?;
        }
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if state.close_requested || state.kernel_closed || state.play_terminal {
            return Err("live Todo Fore input is closed".into());
        }
        if state.submitted == self.maximum_items {
            return Err("live Todo Fore selected item bound is exhausted".into());
        }
        let Some(index) = state.free.pop() else {
            return Ok(BodyLiveForeAdmission::Full);
        };
        let sequence = u64::from(state.submitted);
        state.slots[index].clear();
        state.slots[index].extend_from_slice(canonical);
        state.queued.push_back(index);
        state.submitted += 1;
        drop(state);
        self.control.signal_activity();
        Ok(BodyLiveForeAdmission::Accepted { sequence })
    }

    pub fn close(&self) -> Result<(), String> {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if state.close_requested || state.play_terminal {
            return Err("live Todo Fore was already closed".into());
        }
        state.close_requested = true;
        drop(state);
        self.control.signal_activity();
        Ok(())
    }

    /// Distinguishes Host staging from kernel ingress at cancellation or
    /// failure. Kernel admission is still not a committed Todo state output.
    pub fn status(&self) -> BodyLiveForeStatus {
        let state = self.state.lock().expect("live Fore queue lock poisoned");
        BodyLiveForeStatus {
            queue_accepted: state.submitted,
            kernel_admitted: state.kernel_admitted,
            queued: state.queued.len() as u16,
            close_requested: state.close_requested,
            kernel_closed: state.kernel_closed,
            play_terminal: state.play_terminal,
        }
    }

    pub(crate) fn mark_play_terminal(&self) {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        state.play_terminal = true;
        drop(state);
        self.control.signal_activity();
    }

    pub(crate) fn matches_plan(&self, plan: &Plan, control: &RunControl) -> bool {
        self.plan_id == plan.plan_id
            && self.control.same_source(control)
            && plan.fragments.len() == 1
            && plan.fragments[0]
                .fore_ports
                .iter()
                .any(|port| port == &self.port)
    }

    pub(crate) fn maximum_items(&self) -> u16 {
        self.maximum_items
    }

    pub(crate) fn port(&self) -> &PlannedForePort {
        &self.port
    }

    pub(crate) fn observed_generation(&self) -> u64 {
        self.control.activity_generation()
    }

    pub(crate) fn wait_if_empty(&self, observed: u64) {
        let state = self.state.lock().expect("live Fore queue lock poisoned");
        if !state.queued.is_empty() || state.close_requested {
            return;
        }
        drop(state);
        self.control.wait_for_activity_or_stop(observed);
    }

    pub(crate) fn with_front<R>(&self, f: impl FnOnce(Option<(u64, &[u8])>, bool) -> R) -> R {
        let state = self.state.lock().expect("live Fore queue lock poisoned");
        let front = state.queued.front().map(|index| {
            (
                u64::from(state.kernel_admitted),
                state.slots[*index].as_slice(),
            )
        });
        f(
            front,
            state.close_requested && state.queued.is_empty() && !state.kernel_closed,
        )
    }

    pub(crate) fn acknowledge(&self, sequence: u64) -> Result<(), String> {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if sequence != u64::from(state.kernel_admitted) {
            return Err("live Todo Fore acceptance sequence changed".into());
        }
        let index = state
            .queued
            .pop_front()
            .ok_or("live Todo Fore accepted an absent value")?;
        state.free.push(index);
        state.kernel_admitted += 1;
        Ok(())
    }

    pub(crate) fn mark_kernel_closed(&self) -> Result<(), String> {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if !state.close_requested || !state.queued.is_empty() || state.kernel_closed {
            return Err("live Todo Fore close changed during delivery".into());
        }
        state.kernel_closed = true;
        Ok(())
    }
}
