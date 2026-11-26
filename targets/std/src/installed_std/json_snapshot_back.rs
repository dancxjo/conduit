//! One selected JSON snapshot Host Call; all file effects remain Host-owned.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};
use conduit_std_offers::*;

pub(super) static PUBLISH_FACTORY: BackFactory = BackFactory {
    implementation_id: JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION,
    budget,
    prepare: prepare_publish,
};
pub(super) static READ_FACTORY: BackFactory = BackFactory {
    implementation_id: JSON_SNAPSHOT_READ_IMPLEMENTATION,
    budget,
    prepare: prepare_read,
};
pub(super) struct JsonSnapshotBack {
    input_bytes: u32,
    output_bytes: u32,
    pending: bool,
    terminal: bool,
}
impl<const P: usize> StepBack<P> for JsonSnapshotBack {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return fail(FailureCode::InvalidLifecycle, 1);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if output.admitted_bytes > self.output_bytes
                        || output.value.byte_len > self.output_bytes
                    {
                        return fail(FailureCode::InvalidInput, 2);
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed JSON snapshot completion");
                    io.send(PortId(0), output.value)
                        .expect("ready JSON snapshot output");
                    self.pending = false;
                    self.terminal = true;
                    StepOutcome::Progress
                }
                (HostCallDisposition::Cancelled, None, None) => fail(FailureCode::Cancelled, 3),
                (
                    HostCallDisposition::Failed | HostCallDisposition::Denied,
                    None,
                    Some(failure),
                ) => StepOutcome::Fail(failure),
                _ => fail(FailureCode::InvalidLifecycle, 4),
            }
        } else if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return fail(FailureCode::InvalidLifecycle, 5);
            }
            let Ok(input) = BoundedValueRef::new(value, self.input_bytes) else {
                return fail(FailureCode::InvalidInput, 6);
            };
            io.consume(PortId(0)).expect("present JSON snapshot input");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("admitted JSON snapshot call");
            self.pending = true;
            StepOutcome::Progress
        } else if !self.pending && io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed JSON snapshot input close");
            self.terminal = true;
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
        self.terminal = true;
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

/// Decode the exact lowercase canonical reference selected before Play.
pub(crate) fn selected_reference(placement: &PlannedGear) -> Result<BoundedResourceRef, String> {
    let [configuration] = placement.configuration.as_slice() else {
        return Err("JSON snapshot requires one exact reference".into());
    };
    let ConfigurationValue::Text(hex) = &configuration.value else {
        return Err("JSON snapshot reference must be text".into());
    };
    if configuration.key != "reference"
        || hex.is_empty()
        || hex.len() > 2 * MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES
        || hex
            .bytes()
            .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        return Err("JSON snapshot reference exceeds its canonical hex profile".into());
    }
    let encoded = conduit_data::bytes_literal::decode_hex(hex)
        .map_err(|why| format!("JSON snapshot hex: {why:?}"))?;
    let reference = BoundedResourceRef::decode(&encoded)
        .map_err(|why| format!("JSON snapshot reference: {why:?}"))?;
    if reference
        .encode()
        .map_err(|why| format!("JSON snapshot canonical reference: {why:?}"))?
        != encoded
        || reference.content_profile.as_str() != "text/json-utf8@1"
        || reference.access_class.as_str() != JSON_SNAPSHOT_ACCESS_CLASS
        || reference.extent.bytes == 0
        || reference.extent.bytes > u64::from(JSON_SNAPSHOT_MAXIMUM_BYTES)
        || reference.extent.items != Some(1)
        || reference.lifetime.expires_at.is_some()
    {
        return Err("JSON snapshot reference differs from its exact immutable profile".into());
    }
    Ok(reference)
}
fn selected_bindings(
    resources: &[ResourceBinding],
) -> Result<(&ResourceBinding, &ResourceBinding), String> {
    let [first, second] = resources else {
        return Err("JSON snapshot requires exact content and preparation-memory bindings".into());
    };
    match (first.class_id.as_str(), second.class_id.as_str()) {
        (JSON_SNAPSHOT_ACCESS_CLASS, RUNTIME_MEMORY_RESOURCE_CLASS) => Ok((first, second)),
        (RUNTIME_MEMORY_RESOURCE_CLASS, JSON_SNAPSHOT_ACCESS_CLASS) => Ok((second, first)),
        _ => Err("JSON snapshot bindings have incorrect or duplicate resource classes".into()),
    }
}
fn validate(placement: &PlannedGear) -> Result<bool, String> {
    if !cfg!(target_os = "linux") {
        return Err("selected JSON snapshot file effects require Linux".into());
    }
    let write = match placement.implementation_id.as_str() {
        JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION => true,
        JSON_SNAPSHOT_READ_IMPLEMENTATION => false,
        _ => return Err("unsupported JSON snapshot implementation".into()),
    };
    let (resource, memory) = selected_bindings(&placement.resources)?;
    let content = resource
        .content
        .as_ref()
        .ok_or("JSON snapshot lacks selected content")?;
    let [authority] = placement.authority.as_slice() else {
        return Err("JSON snapshot requires independent current authority".into());
    };
    // Fence every selected authority/residence identifier before this factory
    // clones content into the temporary offer or decodes reference metadata.
    if [
        placement.host_id.as_str(),
        placement.boot_id.as_str(),
        placement.kind_id.as_str(),
        placement.capability_id.as_str(),
        resource.pool_id.as_str(),
        resource.class_id.as_str(),
        memory.pool_id.as_str(),
        memory.class_id.as_str(),
        content.owner_host.as_str(),
        content.owner_boot.as_str(),
        content.base_id.as_str(),
        content.residence_profile.as_str(),
        content.contract.content_profile.as_str(),
        authority.grant_id.as_str(),
        authority.contract_id.as_str(),
        authority.host_call_contract_id.as_str(),
        authority.subject_kind.as_str(),
        authority.host_id.as_str(),
        authority.boot_id.as_str(),
        authority.capability_id.as_str(),
    ]
    .iter()
    .any(|text| text.is_empty() || text.len() > JSON_SNAPSHOT_MAXIMUM_ID_BYTES)
    {
        return Err("JSON snapshot selected metadata exceeds preparation profile".into());
    }
    let reference = selected_reference(placement)?;
    let offer = json_snapshot_offer(content.contract.clone())?;
    content
        .validate()
        .map_err(|why| format!("JSON snapshot content: {why:?}"))?;
    content
        .contract
        .accepts_reference(&reference)
        .map_err(|why| format!("JSON snapshot extent: {why:?}"))?;
    if placement.semantic_contract != offer.semantic_contract
        || placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.capability_id != offer.capability_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || resource.class_id.as_str() != JSON_SNAPSHOT_ACCESS_CLASS
        || memory.pool_id == resource.pool_id
        || memory.class_id.as_str() != RUNTIME_MEMORY_RESOURCE_CLASS
        || memory.units != JSON_SNAPSHOT_PREPARATION_BYTES
        || memory.protected.is_some()
        || memory.compute.is_some()
        || memory.content.is_some()
        || resource.units != 1
        || resource.protected.is_some()
        || resource.compute.is_some()
        || content.base_id.as_str() != JSON_SNAPSHOT_BASE
        || content.residence_profile.as_str() != JSON_SNAPSHOT_RESIDENCE
        || content.owner_host != placement.host_id
        || content.owner_boot != placement.boot_id
        || authority.host_id != placement.host_id
        || authority.boot_id != placement.boot_id
        || authority.grant_id.as_str().is_empty()
        || authority.contract_id.as_str() != JSON_SNAPSHOT_AUTHORITY
        || authority.host_call_contract_id != offer.host_calls[0].contract_id
        || authority.subject_kind != placement.kind_id
        || authority.capability_id != placement.capability_id
        || write != (content.contract.access == ResourceAccessMode::WriteCandidatePublish)
    {
        return Err(
            "JSON snapshot differs from exact selected Back, residence or authority".into(),
        );
    }
    Ok(write)
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    let call = &placement.host_calls[0];
    Ok(BackBudget {
        value_items: 3,
        value_bytes: call
            .maximum_input_bytes
            .checked_add(call.maximum_output_bytes)
            .ok_or("JSON snapshot preparation backing overflow")?,
        host_requests: 1,
        sign_items: 8,
        maximum_value_bytes: call.maximum_input_bytes.max(call.maximum_output_bytes),
    })
}
fn prepare_operation(placement: &PlannedGear, write: bool) -> Result<InstalledBack, String> {
    if validate(placement)? != write {
        return Err("JSON snapshot factory operation mismatch".into());
    }
    let call = &placement.host_calls[0];
    Ok(InstalledBack::JsonSnapshot(JsonSnapshotBack {
        input_bytes: call.maximum_input_bytes,
        output_bytes: call.maximum_output_bytes,
        pending: false,
        terminal: false,
    }))
}
fn prepare_publish(
    p: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    prepare_operation(p, true)
}
fn prepare_read(
    p: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    prepare_operation(p, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{HostCallOutcome, ValueRef};
    fn back() -> JsonSnapshotBack {
        JsonSnapshotBack {
            input_bytes: 4096,
            output_bytes: 512,
            pending: false,
            terminal: false,
        }
    }
    fn request(back: &mut JsonSnapshotBack, length: u32) -> StepOutcome {
        let mut io = StepIo::test_frame(
            [Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: length,
            })],
            [false],
            [Some(512)],
            None,
            4096,
        );
        back.step(&mut io, &StepInputBytes::test_frame([None], None))
    }
    fn completion(
        back: &mut JsonSnapshotBack,
        ready: bool,
        request: u32,
        length: u32,
        disposition: HostCallDisposition,
    ) -> StepOutcome {
        let mut io = StepIo::test_frame(
            [None],
            [false],
            [ready.then_some(512)],
            Some((
                RequestId(request),
                HostCallOutcome {
                    disposition,
                    output: (disposition == HostCallDisposition::Completed).then(|| {
                        BoundedValueRef::new(
                            ValueRef {
                                slot: 1,
                                generation: 1,
                                byte_len: length,
                            },
                            length.max(512),
                        )
                        .unwrap()
                    }),
                    failure: matches!(
                        disposition,
                        HostCallDisposition::Failed | HostCallDisposition::Denied
                    )
                    .then_some(Failure {
                        code: FailureCode::HostCallDenied,
                        detail: 77,
                    }),
                },
            )),
            4096,
        );
        back.step(&mut io, &StepInputBytes::test_frame([None], None))
    }
    #[test]
    fn one_shot_waits_under_output_pressure_and_requires_original_completion() {
        let mut back = back();
        assert_eq!(request(&mut back, 4), StepOutcome::Progress);
        assert_eq!(
            completion(&mut back, false, 0, 16, HostCallDisposition::Completed),
            StepOutcome::Await
        );
        assert!(back.pending);
        assert!(!back.terminal);
        assert!(matches!(
            completion(&mut back, true, 1, 16, HostCallDisposition::Completed),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                ..
            })
        ));
        assert_eq!(
            completion(&mut back, true, 0, 16, HostCallDisposition::Completed),
            StepOutcome::Progress
        );
        assert!(back.terminal);
        assert!(!back.pending);
        assert_eq!(request(&mut back, 4), StepOutcome::Complete);
    }
    #[test]
    fn bounds_failure_and_cancellation_never_emit_a_success_reference() {
        assert!(matches!(
            request(&mut back(), 4097),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                ..
            })
        ));
        for disposition in [
            HostCallDisposition::Denied,
            HostCallDisposition::Failed,
            HostCallDisposition::Cancelled,
        ] {
            let mut back = back();
            assert_eq!(request(&mut back, 4), StepOutcome::Progress);
            assert!(matches!(
                completion(&mut back, true, 0, 16, disposition),
                StepOutcome::Fail(_)
            ));
        }
        let mut oversized = back();
        request(&mut oversized, 4);
        assert!(matches!(
            completion(&mut oversized, true, 0, 513, HostCallDisposition::Completed),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                ..
            })
        ));
        <JsonSnapshotBack as StepBack<1>>::cancel(&mut oversized);
        assert_eq!(request(&mut oversized, 4), StepOutcome::Complete);
    }
    #[test]
    fn binding_roles_are_independent_of_pool_sort_order() {
        let content = ResourceBinding {
            pool_id: "z/content".into(),
            class_id: JSON_SNAPSHOT_ACCESS_CLASS.into(),
            units: 1,
            compute: None,
            protected: None,
            content: None,
        };
        let memory = ResourceBinding {
            pool_id: "a/memory".into(),
            class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
            units: JSON_SNAPSHOT_PREPARATION_BYTES,
            compute: None,
            protected: None,
            content: None,
        };
        for resources in [
            [content.clone(), memory.clone()],
            [memory.clone(), content.clone()],
        ] {
            let (selected, preparation) = selected_bindings(&resources).unwrap();
            assert_eq!(selected, &content);
            assert_eq!(preparation, &memory);
        }
        assert!(selected_bindings(&[memory.clone(), memory]).is_err());
        assert!(selected_bindings(&[content]).is_err());
    }
    #[test]
    fn exact_factory_lookup_is_explicit_without_ambient_inventory_changes() {
        for id in [
            JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION,
            JSON_SNAPSHOT_READ_IMPLEMENTATION,
        ] {
            assert_eq!(
                super::super::catalog::factory(&id.into())
                    .unwrap()
                    .implementation_id,
                id
            );
        }
        assert!(super::super::catalog::factory(&"std/unknown-json-snapshot@1".into()).is_none());
    }
}
