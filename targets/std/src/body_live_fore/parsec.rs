//! Exact Parsec command staging; source transition acceptance remains in the scan.
use super::*;
use conduit_presentation::PARSEC_MAP_FRAME_BYTES;

impl BodyLiveForeQueue {
    pub fn for_parsec_plan(
        plan: &Plan,
        control: RunControl,
        queue_slots: usize,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 || plan.activations.len() != 1 {
            return Err("live Parsec Fore requires one sealed local scan Plan".into());
        }
        let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
            return Err("live Parsec Fore requires a scan activation".into());
        };
        let fragment = &plan.fragments[0];
        let owner = fragment
            .placements
            .iter()
            .find(|placement| placement.placement_id == scan.owner_placement_id)
            .ok_or("Parsec scan has no owner")?;
        crate::flow_activation::validate_planned_parsec_scan(owner, scan)?;
        crate::flow_activation::maximum_scan_child_steps(scan)?;
        let maximum_items = scan.limits.maximum_items;
        if maximum_items == 0
            || maximum_items > 32
            || queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || queue_slots > usize::from(maximum_items)
        {
            return Err("live Parsec Fore queue exceeds its admitted bounds".into());
        }
        let inputs = fragment
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        let Some(port) = inputs.first().filter(|_| inputs.len() == 1) else {
            return Err("live Parsec Fore requires one exact command input".into());
        };
        let contract = conduit_core::CheckedValueContract::new(
            crate::flow_activation::parsec_command_kind()?,
            PARSEC_MAP_FRAME_BYTES as u32,
            vec![],
        )
        .map_err(|error| format!("Parsec command contract: {error:?}"))?;
        if port.track != ConnectionTrack::Payload
            || !matches!(port.temporal, PortTemporal::Flow { closes: true })
            || port.selected_line.is_some()
            || port.abnormal_kind.is_some()
            || port.byte_capacity != PARSEC_MAP_FRAME_BYTES as u32
            || port.value_kind != scan.item_input.value_kind
            || port.value_contract.as_ref() != Some(&contract)
            || port.placement_id != owner.placement_id
            || port.gear_port_id.as_str() != "item"
        {
            return Err("live Parsec Fore differs from the exact local command Flow".into());
        }
        let mut slots = Vec::with_capacity(queue_slots);
        let mut free = Vec::with_capacity(queue_slots);
        for index in 0..queue_slots {
            slots.push(Vec::with_capacity(PARSEC_MAP_FRAME_BYTES));
            free.push(index);
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: (*port).clone(),
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
