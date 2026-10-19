use super::*;
fn base() -> HostedSignLog {
    HostedSignLog::new(4, (4 * core::mem::size_of::<KernelEvent>()) as u32).unwrap()
}
#[test]
fn declared_policy_prepares_sha256_storage_and_undeclared_profiles_stay_plain() {
    let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
    let mut fragment = exact.plan.fragments[0].clone();
    let plain = StdSignLog::prepare(base(), &[&fragment], 32).unwrap();
    assert!(plain.admission().is_none());
    assert_eq!(plain.bindings().count(), 0);
    fragment.placements[0]
        .realization_properties
        .push(conduit_std_offers::routed_content_sign_requirement().unwrap());
    let mut log = StdSignLog::prepare(base(), &[&fragment], 32).unwrap();
    let admission = *log.admission().unwrap();
    assert_eq!(
        admission.binding_storage_bytes,
        content_sign_storage_bytes(4).unwrap()
    );
    assert_eq!(admission.maximum_digest_input_bytes, 32);
    let guard = crate::allocation_probe::begin();
    for i in 0..16u32 {
        let bytes = i.to_le_bytes();
        let event = log
            .record_value(NodeId(0), PortId(0), CordId(0), &bytes)
            .unwrap();
        let binding = log
            .bindings()
            .find(|b| b.event_sequence == event.sequence)
            .unwrap();
        assert_eq!(
            binding.digest,
            conduit_core::semantic_digest(admission.digest_domain, &bytes)
        );
        assert_eq!(binding.byte_length, 4);
    }
    assert_eq!(guard.finish(), 0);
}
#[test]
fn changed_digest_or_storage_policy_refuses_during_preparation() {
    let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
    let mut fragment = exact.plan.fragments[0].clone();
    let requirement = conduit_std_offers::routed_content_sign_requirement().unwrap();
    let mut bytes = requirement.canonical_value().to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    let changed =
        conduit_core::StructuredConfigurationValue::new(requirement.profile().clone(), bytes)
            .unwrap();
    fragment.placements[0].realization_properties.push(changed);
    assert!(StdSignLog::prepare(base(), &[&fragment], 32).is_err());
}

fn fragment_with_values(hash: bool) -> PlanFragment {
    let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
    let mut fragment = exact.plan.fragments[0].clone();
    if hash {
        fragment.placements[0]
            .realization_properties
            .push(conduit_std_offers::routed_content_sign_requirement().unwrap());
    }
    fragment.placements[0]
        .realization_properties
        .push(conduit_std_offers::routed_content_value_requirement().unwrap());
    fragment
}
#[test]
fn exact_value_policy_prepares_fixed_storage_and_exports_actual_bytes() {
    let fragment = fragment_with_values(true);
    let mut log = StdSignLog::prepare(base(), &[&fragment], 32).unwrap();
    let admission = *log.admission().unwrap();
    assert_eq!(admission.value_storage_bytes, Some(128));
    assert_eq!(admission.maximum_copy_passes, 1);
    let guard = crate::allocation_probe::begin();
    assert_eq!(log.ensure_value_extent(33), Err(SignError::InvalidBudget));
    assert_eq!(
        log.record_value(NodeId(0), PortId(0), CordId(0), &[0; 33]),
        Err(SignError::InvalidBudget)
    );
    assert_eq!(log.len(), 0);
    for i in 0..16u32 {
        let mut bytes = i.to_le_bytes();
        let event = log
            .record_value(NodeId(0), PortId(0), CordId(0), &bytes)
            .unwrap();
        bytes.fill(255);
        assert_eq!(
            log.retained_value(event.sequence),
            Some(i.to_le_bytes().as_slice())
        );
    }
    assert_eq!(guard.finish(), 0);
    let report = capture_report(&log).unwrap();
    assert_eq!(report.bindings.len(), 4);
    assert_eq!(report.retained_values.len(), 4);
    for value in report.retained_values {
        assert!(report.bindings.contains(&value.binding));
        assert_eq!(value.bytes.len(), value.binding.byte_length as usize);
        assert_eq!(
            conduit_core::semantic_digest(admission.digest_domain, &value.bytes),
            value.binding.digest
        );
        assert!(u32::from_le_bytes(value.bytes.try_into().unwrap()) >= 12);
    }
}
#[test]
fn value_capture_requires_exact_digest_dependency_and_refuses_policy_or_budget_drift() {
    assert!(StdSignLog::prepare(base(), &[&fragment_with_values(false)], 32).is_err());
    let mut fragment = fragment_with_values(true);
    assert!(StdSignLog::prepare(base(), &[&fragment], u32::MAX).is_err());
    let property = conduit_std_offers::routed_content_value_requirement().unwrap();
    fragment.placements[0]
        .realization_properties
        .retain(|p| p.profile() != property.profile());
    let mut bytes = property.canonical_value().to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    let changed =
        conduit_core::StructuredConfigurationValue::new(property.profile().clone(), bytes).unwrap();
    fragment.placements[0].realization_properties.push(changed);
    assert!(StdSignLog::prepare(base(), &[&fragment], 32).is_err());
}
