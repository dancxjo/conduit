//! Wide declaration round trip; this is not a physical memory-admission proof.
use super::*;
use conduit_core as c;
#[test]
fn generated_state_above_sixty_four_kib_remains_exact_and_independently_bounded() {
    let mut fragment = crate::tests::sealed_current_fragment();
    for placement in &mut fragment.placements {
        placement.limits.max_queue_bytes = 70_000;
    }
    fragment.connections[0].byte_capacity = 70_000;
    let identity = c::PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let plan = c::seal_plan(identity, vec![fragment]);
    let fragment = &plan.fragments[0];
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let generated =
        crate::generate_embedded_plan(fragment, &lowered, crate::EmbeddedImageBounds::HOST_TOOLING)
            .unwrap();
    let mut maxima = c::AssignedPlanMaxima {
        encoded_bytes: u16::MAX,
        runtime_state_bytes: u32::MAX,
        counts: [u8::MAX; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let encoded = crate::encode_assigned_plan(&generated, maxima).unwrap();
    let state = u32::from_le_bytes(encoded[12..16].try_into().unwrap());
    assert!(state > u32::from(u16::MAX));
    assert_eq!(c::ASSIGNED_PLAN_HEADER_BYTES, 125);
    let calls = generated
        .host_calls
        .iter()
        .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
        .collect::<Vec<_>>();
    let resources = generated
        .resources
        .iter()
        .map(|resource| resource.resource)
        .collect::<Vec<_>>();
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    maxima.runtime_state_bytes = state;
    assert_eq!(
        c::decode_assigned_plan(&encoded, maxima, required)
            .unwrap()
            .runtime_state_bytes,
        state
    );
    maxima.runtime_state_bytes = state - 1;
    assert_eq!(
        c::decode_assigned_plan(&encoded, maxima, required),
        Err(c::AssignedPlanRefusal::RuntimeCapacityExceeded)
    );
    assert!(
        matches!(crate::encode_assigned_plan(&generated,maxima),Err(GenerationError::BoundExceeded{table:"assigned runtime state bytes",actual,maximum}) if actual==u64::from(state)&&maximum==u64::from(state-1))
    );
}
