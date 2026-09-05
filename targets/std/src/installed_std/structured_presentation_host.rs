//! Pre-admitted capture of exact structured presentation effects.

use conduit_core::{
    bind_presentation, bind_sign, ActivePlayIdentity, ConnectionId, HostAdvertisement, KindId,
    Observation, ObservationKind, PlacementId, PlanFragment, PresentationId, ValuePayload,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{scheduler::HostOperationRequest, NodeId};
use conduit_plan_lowering::lowering::{KernelExecutionIdentityMap, KernelIdentityMap};

struct CaptureSlot {
    node: NodeId,
    placement_id: PlacementId,
    connection_id: ConnectionId,
    value_kind: KindId,
    request: Option<HostOperationRequest>,
    encoded: Vec<u8>,
}

pub(super) struct CapturedStructuredPresentation {
    pub(super) request: HostOperationRequest,
    pub(super) placement_id: PlacementId,
    pub(super) connection_id: ConnectionId,
    pub(super) value_kind: KindId,
    pub(super) encoded: Vec<u8>,
    pub(super) sequence: u64,
}

pub(super) struct StructuredPresentationHost {
    slots: Vec<CaptureSlot>,
}

impl StructuredPresentationHost {
    pub(super) fn prepare(
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
    ) -> Result<Self, String> {
        let capacity = fragment
            .placements
            .iter()
            .filter(|placement| {
                placement.implementation_id.as_str()
                    == conduit_std_offers::STRUCTURED_PRESENTATION_STD_IMPLEMENTATION
            })
            .count();
        let mut slots = Vec::with_capacity(capacity);
        for placement in &fragment.placements {
            if placement.implementation_id.as_str()
                != conduit_std_offers::STRUCTURED_PRESENTATION_STD_IMPLEMENTATION
            {
                continue;
            }
            let node = identity
                .node_for_placement(&placement.placement_id)
                .ok_or_else(|| "structured presentation placement was not lowered".to_string())?;
            let input = placement
                .inputs
                .first()
                .ok_or_else(|| "structured presentation input is missing".to_string())?;
            let connection = fragment
                .connections
                .iter()
                .find(|connection| {
                    connection.sink_placement_id == placement.placement_id
                        && connection.sink_port_id == input.port_id
                })
                .ok_or_else(|| "structured presentation has no exact input Cord".to_string())?;
            slots.push(CaptureSlot {
                node,
                placement_id: placement.placement_id.clone(),
                connection_id: connection.connection_id.clone(),
                value_kind: input.value_kind.clone(),
                request: None,
                encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            });
        }
        Ok(Self { slots })
    }

    pub(super) fn capture(
        &mut self,
        request: HostOperationRequest,
        input: &[u8],
    ) -> Result<(), String> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.node == request.node)
            .ok_or_else(|| "structured presentation request has no admitted capture".to_string())?;
        if slot.request.is_some() || input.len() > slot.encoded.capacity() {
            return Err("structured presentation exceeded its admitted capture".into());
        }
        // The checked Plan and exact Cord already bind this request to `value_kind`;
        // semantic decoding remains above Play in the typed Presentation projection.
        slot.encoded.extend_from_slice(input);
        slot.request = Some(request);
        Ok(())
    }

    /// A failed or cancelled Play need not have reached every planned presenter.
    /// Keep effects that happened, but never fabricate an effect for an idle sink.
    pub(super) fn retain_realized_effects(&mut self) {
        self.slots.retain(|slot| slot.request.is_some());
    }

    fn into_captured(self) -> Result<Vec<CapturedStructuredPresentation>, String> {
        self.slots
            .into_iter()
            .enumerate()
            .map(|(sequence, slot)| {
                Ok(CapturedStructuredPresentation {
                    request: slot.request.ok_or_else(|| {
                        "structured presentation completed without a value".to_string()
                    })?,
                    placement_id: slot.placement_id,
                    connection_id: slot.connection_id,
                    value_kind: slot.value_kind,
                    encoded: slot.encoded,
                    sequence: sequence as u64,
                })
            })
            .collect()
    }

    pub(super) fn project(
        self,
        host: &HostAdvertisement,
        fragment: &PlanFragment,
        active_play: &ActivePlayIdentity,
        lowered: &KernelIdentityMap,
        execution: &mut KernelExecutionIdentityMap,
        next_sign_sequence: &mut u64,
    ) -> Result<(Vec<Observation>, Vec<PresentationId>), String> {
        let mut observations = Vec::with_capacity(self.slots.len());
        let mut presentation_ids = Vec::with_capacity(self.slots.len());
        for captured in self.into_captured()? {
            let presentation = bind_presentation(
                &active_play.active_play_id,
                &captured.placement_id,
                captured.sequence,
            );
            let sign = bind_sign(
                &host.host_id,
                &host.boot_id,
                Some(&active_play.active_play_id),
                *next_sign_sequence,
            );
            *next_sign_sequence = next_sign_sequence
                .checked_add(1)
                .ok_or_else(|| "std sign sequence exhausted".to_string())?;
            execution
                .bind_presentation(
                    lowered,
                    captured.request.node,
                    captured.request.request,
                    &presentation,
                )
                .map_err(|error| format!("bind std structured presentation: {error:?}"))?;
            execution
                .bind_sign(
                    &sign,
                    Some(captured.request.node),
                    Some(captured.request.request),
                    Some(&presentation.presentation_id),
                )
                .map_err(|error| format!("bind std structured presentation sign: {error:?}"))?;
            observations.push(Observation {
                sign_id: sign.sign_id,
                active_play_id: Some(active_play.active_play_id.clone()),
                presentation_id: Some(presentation.presentation_id.clone()),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                plan_id: Some(fragment.plan_id.clone()),
                placement_id: Some(captured.placement_id),
                connection_id: Some(captured.connection_id),
                kind: ObservationKind::ValuePresented {
                    value: ValuePayload {
                        value_kind: captured.value_kind,
                        encoded: captured.encoded,
                    },
                },
            });
            presentation_ids.push(presentation.presentation_id);
        }
        Ok((observations, presentation_ids))
    }
}
