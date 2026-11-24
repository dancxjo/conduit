//! One staging slot for the exact local resident interaction Fore.
use super::*;

impl BodyLiveForeQueue {
    /// Prepare the sealed Fore for an acknowledged Show producer. The caller
    /// binds the owner placement and funds this slot before starting the Play.
    pub fn for_face_interaction_plan(
        plan: &Plan,
        owner: &conduit_core::PlacementId,
        control: RunControl,
        maximum_items: u16,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || !plan.activations.is_empty() || maximum_items == 0 {
            return Err("FaceInteraction ingress requires a sealed finite ordinary Plan".into());
        }
        let [fragment] = plan.fragments.as_slice() else {
            return Err("FaceInteraction ingress requires one local fragment".into());
        };
        let [port] = fragment.fore_ports.as_slice() else {
            return Err("FaceInteraction ingress requires one exact input Fore".into());
        };
        if port.direction != PortDirection::Input
            || port.track != ConnectionTrack::Payload
            || port.front_port_id.as_str() != "interaction"
            || port.gear_port_id.as_str() != "interaction"
            || &port.placement_id != owner
            || port.temporal != (PortTemporal::Flow { closes: true })
            || port.abnormal_kind.is_some()
            || port.selected_line.is_some()
            || port.item_capacity != 1
            || port.value_kind.as_str() != conduit_face::FACE_INTERACTION_VALUE_KIND
            || port.byte_capacity as usize != conduit_face::MAX_FACE_INTERACTION_BYTES
        {
            return Err("FaceInteraction ingress differs from the sealed local owner Fore".into());
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            fragment_id: None,
            port: port.clone(),
            maximum_items,
            control,
            initial: None,
            wait_timeout: None,
            state: Arc::new(Mutex::new(QueueState {
                slots: vec![Vec::with_capacity(port.byte_capacity as usize)],
                free: vec![0],
                queued: VecDeque::with_capacity(1),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
                play_started: false,
                started_at: None,
                idle_stop: None,
            })),
        })
    }

    /// Heap retained exclusively by the one-slot interaction queue. The
    /// caller accounts for its inline queue handle and shared RunControl.
    pub(crate) fn face_interaction_backing_bytes(&self) -> Option<usize> {
        use core::mem::size_of;
        let port = &self.port;
        if port.value_kind.as_str() != conduit_face::FACE_INTERACTION_VALUE_KIND
            || self.initial.is_some()
            || port.abnormal_kind.is_some()
            || port.selected_line.is_some()
        {
            return None;
        }
        let state = self.state.lock().ok()?;
        let [slot] = state.slots.as_slice() else {
            return None;
        };
        let contract_bytes = match &port.value_contract {
            Some(contract) if contract.constraints.capacity() == 0 => {
                contract.value_kind.0.capacity()
            }
            Some(_) => return None,
            None => 0,
        };
        [
            size_of::<Mutex<QueueState>>(),
            2 * size_of::<usize>(),
            state.slots.capacity().checked_mul(size_of::<Vec<u8>>())?,
            state.free.capacity().checked_mul(size_of::<usize>())?,
            state.queued.capacity().checked_mul(size_of::<usize>())?,
            slot.capacity(),
            self.plan_id.0.capacity(),
            port.front_port_id.0.capacity(),
            port.gear_port_id.0.capacity(),
            port.placement_id.0.capacity(),
            port.value_kind.0.capacity(),
            contract_bytes,
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
    }
}
