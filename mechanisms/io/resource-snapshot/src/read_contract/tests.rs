use super::*;
#[test]
fn read_source_contract_is_exact_finite_count_to_linked_media_result() {
    let kind = definition();
    kind.validate().unwrap();
    assert_eq!(kind.kind_id.as_str(), READ_KIND);
    assert_eq!(kind.inputs.len(), 1);
    assert_eq!(kind.outputs.len(), 1);
    assert_eq!(kind.inputs[0].value_kind.as_str(), COUNT_INFO_ID);
    assert_eq!(
        kind.outputs[0].value_kind.as_str(),
        crate::read_result::READ_RESULT_PROFILE
    );
    assert_eq!(kind.limits.max_active_instances, 1);
    assert_eq!(kind.limits.max_queue_items, 1);
    assert_eq!(
        kind.limits.max_queue_bytes as usize,
        READ_INPUT_BYTES as usize + crate::read_result::READ_RESULT_BYTES
    );
    assert_eq!(MAXIMUM_MEDIA_READS_PER_REQUEST, 2);
    let input = kind
        .value_contracts()
        .iter()
        .find(|c| c.location == FrontValueLocation::Input(port_id("index")))
        .unwrap();
    for count in [0, 1, u64::from(u32::MAX)] {
        input.contract.validate(&encode_count(count)).unwrap();
    }
    assert!(input
        .contract
        .validate(&encode_count(u64::from(u32::MAX) + 1))
        .is_err());
    assert!(input.contract.validate(&[0; 7]).is_err());
    let output = kind
        .value_contracts()
        .iter()
        .find(|c| c.location == FrontValueLocation::Output(port_id("result")))
        .unwrap();
    assert_eq!(
        output.contract.maximum_bytes as usize,
        crate::read_result::READ_RESULT_BYTES
    );
    output
        .contract
        .validate(crate::read_result::cancelled())
        .unwrap();
}
#[test]
fn read_contract_does_not_invent_authored_cancellation_or_extra_control_port() {
    let kind = definition();
    let profiles: alloc::vec::Vec<_> = kind
        .semantic_laws
        .iter()
        .filter_map(|law| match law {
            KindSemanticLaw::TerminalTransduction(profile) => Some(profile),
            _ => None,
        })
        .collect();
    assert_eq!(profiles.len(), 1);
    assert_eq!(
        profiles[0].cancellation,
        CancellationTransduction::NotCancellable
    );
    assert_eq!(
        profiles[0].normal_close,
        NormalCloseTransduction::PropagateAfterDrain
    );
    assert_eq!(
        profiles[0].abnormal,
        AbnormalTerminalTransduction::NotAccepted
    );
    assert!(kind.inputs.iter().all(|p| p.abnormal_kind.is_none()));
    assert!(kind.outputs.iter().all(|p| p.abnormal_kind.is_none()));
}
