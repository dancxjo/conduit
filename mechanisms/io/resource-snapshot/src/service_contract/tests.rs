use super::*;
#[test]
fn combined_source_keeps_original_append_abi_and_independent_read_pair() {
    let service = definition();
    service.validate().unwrap();
    let append = crate::append_contract::definition();
    let read = crate::read_contract::definition();
    assert_eq!(service.kind_id.as_str(), SERVICE_KIND);
    assert_ne!(
        crate::append_contract::APPEND_KIND,
        crate::read_contract::READ_KIND
    );
    assert_eq!(service.inputs.len(), 2);
    assert_eq!(service.outputs.len(), 2);
    assert_eq!(service.inputs[0], append.inputs[0]);
    assert_eq!(service.outputs[0], append.outputs[0]);
    assert_eq!(service.inputs[1].port_id, port_id(READ_INPUT));
    assert_eq!(service.outputs[1].port_id, port_id(READ_OUTPUT));
    assert_eq!(service.inputs[1].value_kind, read.inputs[0].value_kind);
    assert_eq!(service.outputs[1].value_kind, read.outputs[0].value_kind);
    for original in append.value_contracts() {
        assert!(service.value_contracts().contains(original));
    }
    assert_eq!(service.value_contracts().len(), 4);
    assert_eq!(service.limits.max_active_instances, 1);
    assert_eq!(service.limits.max_queue_items, 1);
    assert_eq!(
        service.limits.max_queue_bytes,
        append.limits.max_queue_bytes + read.limits.max_queue_bytes
    );
}
#[test]
fn pairwise_closure_is_canonical_and_no_authored_cancellation_is_promised() {
    let service = definition();
    let profiles: alloc::vec::Vec<_> = service
        .semantic_laws
        .iter()
        .filter_map(|law| match law {
            KindSemanticLaw::TerminalTransduction(profile) => Some(profile),
            _ => None,
        })
        .collect();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].input_port_id, port_id(READ_INPUT));
    assert_eq!(profiles[0].output_port_id, port_id(READ_OUTPUT));
    assert_eq!(profiles[1].input_port_id, port_id(APPEND_INPUT));
    assert_eq!(profiles[1].output_port_id, port_id(APPEND_OUTPUT));
    for profile in profiles {
        assert_eq!(
            profile.normal_close,
            NormalCloseTransduction::PropagateAfterDrain
        );
        assert_eq!(
            profile.cancellation,
            CancellationTransduction::NotCancellable
        );
    }
    assert!(service.configuration.is_empty());
    assert!(service.startup_parameters.is_empty());
}
