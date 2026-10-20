//! A real numerical producer queue scoped to one sealed ordinary Each Plan.
use super::*;
impl BodyLiveForeQueue {
    pub fn for_near_each_plan(
        plan: &Plan,
        control: RunControl,
        queue_slots: usize,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 || plan.activations.len() != 1 {
            return Err("numerical Fore requires one exact local Each Plan".into());
        }
        let PlannedActivationEntry::Unary(each) = &plan.activations[0] else {
            return Err("numerical Fore requires Each activation".into());
        };
        crate::flow_activation::validate_near_each(plan, each)?;
        let maximum_items = each.limits.maximum_items;
        if queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || queue_slots > usize::from(maximum_items)
        {
            return Err("numerical Fore queue exceeds its admitted slots".into());
        }
        let mut inputs = plan.fragments[0]
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input);
        let port = inputs.next().ok_or("numerical Fore input is absent")?;
        if inputs.next().is_some()
            || port.track != ConnectionTrack::Payload
            || port.temporal != (PortTemporal::Flow { closes: true })
            || port.abnormal_kind.is_some()
            || port.selected_line.is_some()
            || port.item_capacity != 1
            || port.value_kind.as_str() != conduit_core::QUANTITY_INFO_ID
            || port.byte_capacity == 0
            || port.byte_capacity as usize > conduit_core::QUANTITY_VALUE_MAX_ENCODED_BYTES
        {
            return Err("numerical Fore requires one finite local Quantity Flow".into());
        }
        let mut slots = Vec::with_capacity(queue_slots);
        let mut free = Vec::with_capacity(queue_slots);
        for index in 0..queue_slots {
            slots.push(Vec::with_capacity(port.byte_capacity as usize));
            free.push(index);
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: port.clone(),
            maximum_items,
            control,
            initial: None,
            wait_timeout: None,
            state: Arc::new(Mutex::new(QueueState {
                slots,
                free,
                queued: VecDeque::with_capacity(queue_slots),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
                play_started: false,
                started_at: None,
            })),
        })
    }
}
