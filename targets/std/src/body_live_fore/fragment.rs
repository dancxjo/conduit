//! Bounded ingress for a local fragment with a seed and one closing Flow.
use super::*;
use conduit_core::{verify_plan_fragment, PlanFragment};

impl BodyLiveForeQueue {
    pub fn for_local_flow_fragment(
        fragment: &PlanFragment,
        flow: &str,
        seed: crate::ExternalForeInput,
        control: RunControl,
        queue_slots: usize,
        maximum_items: u16,
    ) -> Result<Self, String> {
        if !verify_plan_fragment(fragment)
            || queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || queue_slots > usize::from(maximum_items)
            || maximum_items > 64
        {
            return Err("live Fore requires a sealed fragment and bounded queue".into());
        }
        let inputs: Vec<_> = fragment
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
            .collect();
        let port = inputs
            .iter()
            .find(|port| port.front_port_id.as_str() == flow)
            .ok_or("live Flow input is missing")?;
        let seed_port = inputs
            .iter()
            .find(|port| port.front_port_id == seed.front_port_id)
            .ok_or("seed Fore input is missing")?;
        if inputs.len() != 2
            || port.front_port_id == seed_port.front_port_id
            || port.temporal != (PortTemporal::Flow { closes: true })
            || !matches!(
                seed_port.temporal,
                PortTemporal::Value | PortTemporal::Flow { closes: true }
            )
            || seed.track != ConnectionTrack::Payload
            || inputs.iter().any(|port| {
                port.track != ConnectionTrack::Payload
                    || port.selected_line.is_some()
                    || port.abnormal_kind.is_some()
                    || port.byte_capacity == 0
                    || port.byte_capacity > 131_072
            })
        {
            return Err("live fragment requires one local closing Flow and one seed input".into());
        }
        if seed.bytes.len() > seed_port.byte_capacity as usize
            || seed_port.value_contract.as_ref().map_or_else(
                || {
                    conduit_core::validate_primitive_info(
                        seed_port.value_kind.as_str(),
                        &seed.bytes,
                    )
                    .is_err()
                },
                |contract| contract.validate(&seed.bytes).is_err(),
            )
        {
            return Err("live fragment seed violates its sealed contract".into());
        }
        Ok(Self {
            plan_id: fragment.plan_id.clone(),
            fragment_id: Some(fragment.fragment_id.clone()),
            port: (**port).clone(),
            maximum_items,
            control,
            initial: Some(((**seed_port).clone(), seed.bytes)),
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
            })),
        })
    }

    pub(crate) fn matches_local_flow_fragment(
        &self,
        fragment: &PlanFragment,
        control: &RunControl,
    ) -> bool {
        self.plan_id == fragment.plan_id
            && self.fragment_id.as_ref() == Some(&fragment.fragment_id)
            && self.control.same_source(control)
            && self.port.temporal == (PortTemporal::Flow { closes: true })
            && self
                .initial()
                .is_some_and(|(seed, _)| fragment.fore_ports.contains(seed))
            && fragment.fore_ports.contains(&self.port)
    }
}
