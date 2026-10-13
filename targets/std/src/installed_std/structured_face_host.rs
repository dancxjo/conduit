//! Pre-admitted capture of exact structured face effects.

use conduit_core::{
    bind_face, bind_sign, ActivePlayIdentity, ConnectionId, FaceId, HostAdvertisement, KindId,
    Observation, ObservationKind, PlacementId, PlanFragment, ValuePayload,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{scheduler::HostCallRequest, NodeId};
use conduit_plan_lowering::lowering::{KernelExecutionIdentityMap, KernelIdentityMap};

struct CaptureSlot {
    node: NodeId,
    placement_id: PlacementId,
    connection_id: ConnectionId,
    value_kind: KindId,
    request: Option<HostCallRequest>,
    encoded: Vec<u8>,
}

pub(super) struct CapturedStructuredFace {
    pub(super) request: HostCallRequest,
    pub(super) placement_id: PlacementId,
    pub(super) connection_id: ConnectionId,
    pub(super) value_kind: KindId,
    pub(super) encoded: Vec<u8>,
    pub(super) sequence: u64,
}

pub(super) struct StructuredFaceHost {
    slots: Vec<CaptureSlot>,
}

impl StructuredFaceHost {
    pub(super) fn prepare(
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
    ) -> Result<Self, String> {
        let capacity = fragment
            .placements
            .iter()
            .filter(|placement| {
                placement.implementation_id.as_str()
                    == conduit_std_offers::STRUCTURED_FACE_STD_IMPLEMENTATION
            })
            .count();
        let mut slots = Vec::with_capacity(capacity);
        for placement in &fragment.placements {
            if placement.implementation_id.as_str()
                != conduit_std_offers::STRUCTURED_FACE_STD_IMPLEMENTATION
            {
                continue;
            }
            let node = identity
                .node_for_placement(&placement.placement_id)
                .ok_or_else(|| "structured face placement was not lowered".to_string())?;
            let input = placement
                .inputs
                .first()
                .ok_or_else(|| "structured face input is missing".to_string())?;
            let connection = fragment
                .connections
                .iter()
                .find(|connection| {
                    connection.sink_placement_id == placement.placement_id
                        && connection.sink_port_id == input.port_id
                })
                .ok_or_else(|| "structured face has no exact input Cord".to_string())?;
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

    pub(super) fn capture(&mut self, request: HostCallRequest, input: &[u8]) -> Result<(), String> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.node == request.node)
            .ok_or_else(|| "structured face request has no admitted capture".to_string())?;
        if slot.request.is_some() || input.len() > slot.encoded.capacity() {
            return Err("structured face exceeded its admitted capture".into());
        }
        // The checked plan and exact Cord already bind this request to `value_kind`;
        // semantic decoding remains above Play in the typed Face projection.
        slot.encoded.extend_from_slice(input);
        slot.request = Some(request);
        Ok(())
    }

    /// A failed or cancelled Play need not have reached every planned presenter.
    /// Keep effects that happened, but never make an effect for an idle sink.
    pub(super) fn retain_realized_effects(&mut self) {
        self.slots.retain(|slot| slot.request.is_some());
    }

    fn into_captured(self) -> Result<Vec<CapturedStructuredFace>, String> {
        self.slots
            .into_iter()
            .enumerate()
            .map(|(sequence, slot)| {
                Ok(CapturedStructuredFace {
                    request: slot
                        .request
                        .ok_or_else(|| "structured face completed without a value".to_string())?,
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
    ) -> Result<(Vec<Observation>, Vec<FaceId>), String> {
        let mut observations = Vec::with_capacity(self.slots.len());
        let mut face_ids = Vec::with_capacity(self.slots.len());
        for captured in self.into_captured()? {
            let face = bind_face(
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
                .bind_face(
                    lowered,
                    captured.request.node,
                    captured.request.request,
                    &face,
                )
                .map_err(|error| format!("bind std structured face: {error:?}"))?;
            execution
                .bind_sign(
                    &sign,
                    Some(captured.request.node),
                    Some(captured.request.request),
                    Some(&face.face_id),
                )
                .map_err(|error| format!("bind std structured face sign: {error:?}"))?;
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == captured.placement_id)
                .ok_or("structured Face effect has no planned Gear")?;
            let label = conduit_semantic_catalog::face_data_label(&placement.configuration)?.into();
            observations.push(Observation {
                sign_id: sign.sign_id,
                active_play_id: Some(active_play.active_play_id.clone()),
                face_id: Some(face.face_id.clone()),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                plan_id: Some(fragment.plan_id.clone()),
                placement_id: Some(captured.placement_id),
                connection_id: Some(captured.connection_id),
                kind: ObservationKind::ValuePresented {
                    label,
                    value: ValuePayload {
                        value_kind: captured.value_kind,
                        encoded: captured.encoded,
                    },
                },
            });
            face_ids.push(face.face_id);
        }
        Ok((observations, face_ids))
    }
}
