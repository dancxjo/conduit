//! CPU implementation descriptions only; no initialized renderer or admission.
use conduit_core as c;
use conduit_environment_make::DisplayMaskSource;
pub fn offers(source: &DisplayMaskSource) -> Vec<c::CapabilityOffer> {
    source
        .plot()
        .expanded
        .gears
        .iter()
        .map(|gear| {
            c::BackOfferBuilder::new(
                fixture_kind(&gear.kind_id),
                c::Back {
                    capability_id: format!("fixture/{}", gear.gear_id.as_str()).into(),
                    execution_profile_id: "fixture/mask".into(),
                    implementation_id: format!("fixture/{}", gear.gear_id.as_str()).into(),
                    artifact_id: "fixture/mask".into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build()
        })
        .collect()
}
fn fixture_kind(id: &c::KindId) -> c::Kind {
    let projection = [
        conduit_face::renderer_kind_projection(),
        conduit_face::face_interaction_kind_projection(),
        conduit_face::face_tee_kind_projection(),
    ]
    .into_iter()
    .find(|projection| &projection.kind_id == id)
    .unwrap();
    c::Kind {
        startup_parameters: c::data_label_startup_front(),
        shorthand: None,
        kind_id: projection.kind_id,
        kind_contract_revision: projection.kind_contract_revision,
        inputs: projection.inputs,
        outputs: projection.outputs,
        configuration: projection.configuration,
        semantic_laws: Default::default(),
        limits: c::CapabilityLimits {
            max_active_instances: conduit_face::MAX_FACE_ACTIVE_INSTANCES,
            max_queue_items: conduit_face::MAX_FACE_QUEUE_ITEMS,
            max_queue_bytes: conduit_face::MAX_RENDERER_VALUE_BYTES
                * u32::from(conduit_face::MAX_FACE_QUEUE_ITEMS),
        },
    }
}
