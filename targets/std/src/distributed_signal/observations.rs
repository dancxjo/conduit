//! Neutral observations grounded by browser delivery acknowledgements.

use conduit_core::{
    bind_face, bind_sign, Observation, ObservationKind, PlanFragment, TerminalDisposition,
    ValuePayload,
};
use conduit_wire::SessionBinding;

pub(super) fn presented(
    sink: &PlanFragment,
    binding: &SessionBinding,
    sequence: u64,
    payload: &[u8],
) -> Result<Observation, String> {
    let placement = &sink.placements[0];
    let face = bind_face(
        &binding.sink_active_play_id,
        &placement.placement_id,
        sequence,
    );
    let sign = bind_sign(
        &sink.host_id,
        &sink.boot_id,
        Some(&binding.sink_active_play_id),
        sequence,
    );
    Ok(Observation {
        sign_id: sign.sign_id,
        active_play_id: Some(binding.sink_active_play_id.clone()),
        face_id: Some(face.face_id),
        host_id: sink.host_id.clone(),
        boot_id: sink.boot_id.clone(),
        plan_id: Some(sink.plan_id.clone()),
        placement_id: Some(placement.placement_id.clone()),
        connection_id: Some(binding.connection_id.clone()),
        kind: ObservationKind::ValuePresented {
            label: conduit_semantic_catalog::face_data_label(&placement.configuration)?.into(),
            value: ValuePayload {
                value_kind: binding.value_kind.clone(),
                encoded: payload.to_vec(),
            },
        },
    })
}

pub(super) fn terminal(
    sink: &PlanFragment,
    binding: &SessionBinding,
    sequence: u64,
) -> Observation {
    Observation {
        sign_id: bind_sign(
            &sink.host_id,
            &sink.boot_id,
            Some(&binding.sink_active_play_id),
            sequence,
        )
        .sign_id,
        active_play_id: Some(binding.sink_active_play_id.clone()),
        face_id: None,
        host_id: sink.host_id.clone(),
        boot_id: sink.boot_id.clone(),
        plan_id: Some(sink.plan_id.clone()),
        placement_id: None,
        connection_id: Some(binding.connection_id.clone()),
        kind: ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed,
        },
    }
}
