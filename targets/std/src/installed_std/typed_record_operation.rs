use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{PlannedGear, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::TYPED_RECORD_FRAME_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TypedRecordFrameOperation {
    pending: bool,
    complete: bool,
}
impl TypedRecordFrameOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.complete => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                    ) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(162),
                    },
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return InstalledOperation::fail(163);
                };
                self.pending = false;
                self.complete = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if !self.pending => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(164),
        }
    }
    pub(super) fn cancel(&mut self) {
        self.pending = false;
        self.complete = true;
    }
}

pub(super) struct TypedRecordFrameHost {
    frame: [u8; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES],
    output: Vec<u8>,
}
impl TypedRecordFrameHost {
    fn new() -> Self {
        Self {
            frame: [0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES],
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }
    pub(super) fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_net::TypedRecordFrameRefusal> {
        let value = StructuredInfoValue::from_canonical_bytes(input)
            .map_err(|_| conduit_net::TypedRecordFrameRefusal::WrongTypedRecordValueType)?;
        let written = conduit_net::frame_typed_record_value_into(&value, &mut self.frame)?;
        let framed = conduit_net::framed_typed_record_value(&self.frame[..written])?;
        self.output = framed
            .canonical_bytes()
            .map_err(|_| conduit_net::TypedRecordFrameRefusal::FrameTooLarge)?;
        Ok(&self.output)
    }
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Vec<Option<TypedRecordFrameHost>> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            (placement.implementation_id.as_str()
                == conduit_std_offers::TYPED_RECORD_FRAME_STD_IMPLEMENTATION)
                .then(TypedRecordFrameHost::new)
        })
        .collect()
}
fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = conduit_std_offers::typed_record_frame_std_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || !placement.configuration.is_empty()
        || !placement.resources.is_empty()
    {
        return Err("planned typed-record frame differs from installed realization".into());
    }
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        host_requests: 1,
        sign_items: 24,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::TypedRecordFrame(
        TypedRecordFrameOperation {
            pending: false,
            complete: false,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};

    #[test]
    fn host_uses_the_shared_integrity_frame() {
        let original = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/text@1")).unwrap(),
            b"Calling".to_vec(),
        )
        .unwrap();
        let typed = conduit_net::typed_record_value(&original).unwrap();
        let mut expected_frame = [0; conduit_net::MAXIMUM_TYPED_RECORD_FRAME_BYTES];
        let written =
            conduit_net::frame_typed_record_value_into(&typed, &mut expected_frame).unwrap();
        let expected = conduit_net::framed_typed_record_value(&expected_frame[..written])
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let mut host = TypedRecordFrameHost::new();
        assert_eq!(
            host.execute(&typed.canonical_bytes().unwrap()).unwrap(),
            expected
        );
    }

    #[test]
    fn malformed_outer_value_never_becomes_a_frame() {
        let mut host = TypedRecordFrameHost::new();
        assert_eq!(
            host.execute(b"not typed"),
            Err(conduit_net::TypedRecordFrameRefusal::WrongTypedRecordValueType)
        );
        assert!(host.output.is_empty());
    }
}
