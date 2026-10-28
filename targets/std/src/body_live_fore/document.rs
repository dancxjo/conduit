//! Bounded ingress for the existing revisioned document owner.
use super::*;

impl BodyLiveForeQueue {
    pub(crate) fn initial_document_outputs(&self) -> usize {
        usize::from(
            self.initial.is_none()
                && self.port.value_kind.as_str() == conduit_semantic_catalog::TEXT_EDIT_VALUE_KIND,
        )
    }

    /// Stage one checked edit after a genuine start, with one exact current
    /// document supplied by the Body owner. This queue never adopts a result.
    pub fn for_document_step_plan(
        plan: &Plan,
        control: RunControl,
        current: Vec<u8>,
        wait_timeout: Duration,
    ) -> Result<Self, String> {
        if !verify_plan(plan)
            || plan.fragments.len() != 1
            || !plan.activations.is_empty()
            || wait_timeout.is_zero()
            || wait_timeout > Duration::from_secs(300)
        {
            return Err("document step requires a finite ordinary sealed Plan".into());
        }
        let fragment = &plan.fragments[0];
        let [step] = fragment.placements.as_slice() else {
            return Err("document step requires its sole transition".into());
        };
        if step.kind_id.as_str() != conduit_semantic_catalog::TEXT_APPLY_EDIT_KIND
            || step.implementation_id.as_str() != conduit_std_offers::TEXT_APPLY_EDIT_IMPLEMENTATION
        {
            return Err("document step requires the installed pure editing law".into());
        }
        if fragment.fore_ports.len() != 3 {
            return Err("document step requires two inputs and one output".into());
        }
        let find = |name: &str, direction| {
            fragment
                .fore_ports
                .iter()
                .find(|port| port.front_port_id.as_str() == name && port.direction == direction)
        };
        let document = find("document", PortDirection::Input).ok_or("document input is absent")?;
        let edit = find("edit", PortDirection::Input).ok_or("edit input is absent")?;
        let output = find("current", PortDirection::Output).ok_or("current output is absent")?;
        for (port, kind, maximum) in [
            (
                document,
                conduit_semantic_catalog::TEXT_DOCUMENT_VALUE_KIND,
                conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES,
            ),
            (
                edit,
                conduit_semantic_catalog::TEXT_EDIT_VALUE_KIND,
                conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES,
            ),
            (
                output,
                conduit_semantic_catalog::TEXT_DOCUMENT_VALUE_KIND,
                conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES,
            ),
        ] {
            if port.value_kind.as_str() != kind
                || port.temporal != PortTemporal::Value
                || port.track != ConnectionTrack::Payload
                || port.selected_line.is_some()
                || port.abnormal_kind.is_some()
                || port.item_capacity != 1
                || port.byte_capacity != maximum
            {
                return Err("document step differs from its exact finite Fore".into());
            }
        }
        conduit_semantic_catalog::TextDocument::decode(&current)
            .map_err(|error| format!("document step current: {error:?}"))?;
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: edit.clone(),
            maximum_items: 1,
            control,
            initial: Some((document.clone(), current)),
            wait_timeout: Some(wait_timeout),
            state: Arc::new(Mutex::new(QueueState {
                slots: vec![Vec::with_capacity(edit.byte_capacity as usize)],
                free: vec![0],
                queued: VecDeque::with_capacity(1),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
                play_started: false,
                started_at: None,
                idle_stop: Some(crate::RunControlRequestId::new(
                    "body/document-idle-timeout",
                )?),
            })),
        })
    }

    /// Admit one ordinary local editor Plan. The selected Host's finite session
    /// allows at most 64 proposals, including refused edits; this queue grants
    /// neither Mask action authority nor permission to publish a file.
    pub fn for_text_document_plan(
        plan: &Plan,
        control: RunControl,
        queue_slots: usize,
        maximum_edits: u16,
    ) -> Result<Self, String> {
        if !verify_plan(plan)
            || plan.fragments.len() != 1
            || !plan.activations.is_empty()
            || queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || maximum_edits == 0
            || maximum_edits > 64
            || queue_slots > usize::from(maximum_edits)
        {
            return Err(
                "document Fore requires one sealed local Plan and finite edit slots".into(),
            );
        }
        let fragment = &plan.fragments[0];
        let [owner] = fragment.placements.as_slice() else {
            return Err("document Fore requires its sole ordinary document owner".into());
        };
        if owner.kind_id.as_str() != conduit_semantic_catalog::TEXT_EDIT_DOCUMENT_KIND
            || owner.implementation_id.as_str()
                != conduit_std_offers::TEXT_EDIT_DOCUMENT_STD_IMPLEMENTATION
        {
            return Err("document Fore requires the installed revisioned editor".into());
        }
        let mut inputs = fragment
            .fore_ports
            .iter()
            .filter(|p| p.direction == PortDirection::Input);
        let port = inputs.next().ok_or("document edit Fore is absent")?;
        let mut outputs = fragment
            .fore_ports
            .iter()
            .filter(|p| p.direction == PortDirection::Output);
        let current = outputs.next().ok_or("document Current Fore is absent")?;
        if inputs.next().is_some()
            || outputs.next().is_some()
            || port.value_kind.as_str() != conduit_semantic_catalog::TEXT_EDIT_VALUE_KIND
            || current.value_kind.as_str() != conduit_semantic_catalog::TEXT_DOCUMENT_VALUE_KIND
            || port.byte_capacity == 0
            || port.byte_capacity > conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES
            || current.byte_capacity == 0
            || current.byte_capacity > conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES
            || [port, current].iter().any(|p| {
                p.track != ConnectionTrack::Payload
                    || p.selected_line.is_some()
                    || p.abnormal_kind.is_some()
                    || p.temporal != (PortTemporal::Flow { closes: false })
            })
        {
            return Err("document Fore differs from the bounded edit/Current contract".into());
        }
        let slots = (0..queue_slots)
            .map(|_| Vec::with_capacity(port.byte_capacity as usize))
            .collect();
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: port.clone(),
            maximum_items: maximum_edits,
            control,
            initial: None,
            wait_timeout: None,
            state: Arc::new(Mutex::new(QueueState {
                slots,
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
}
