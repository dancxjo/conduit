//! Actual possession and native-owner failure paths under the std allocator probe.
use super::*;
use conduit_kernel::{
    scheduler::HostCallRequest, BoundedValueRef, HostCallId, RequestId, ValueRef,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
fn scope() -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: HostId::from("host/one"),
        boot_id: BootId::from("boot/one"),
        base_instance_id: BaseInstanceId::from("base/files/instance"),
        base_provider_generation: 4,
        plan_id: PlanId::from("plan/one"),
        active_play_id: ActivePlayId::from("play/one"),
        authority_grant_id: AuthorityGrantId::from("grant/visible"),
        authority_contract_id: AuthorityContractId::from("authority/file-read@1"),
        capability_id: CapabilityId::from("file/copy"),
        implementation_id: ImplementationId::from("implementation/file-copy@1"),
        operation_contract_id: HostCallContractId::from("conduit.host/file-read@1"),
        subject_kind: KindId::from("file/source"),
        resource_pool_id: ResourcePoolId::from("files/project/source"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        envelope_id: CapabilityEnvelopeId::from("file/read-only/project-source/max-64"),
        maximum_parameter_bytes: 64,
        maximum_result_bytes: 128,
        maximum_work_units: 8,
        maximum_in_flight: 2,
        maximum_operations: 1,
    }
}

fn authority() -> BaseCapabilityAuthority {
    BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: AuthorityGrantId::from("grant/visible"),
            contract_id: AuthorityContractId::from("authority/file-read@1"),
            host_call_contract_id: HostCallContractId::from("conduit.host/file-read@1"),
            subject_kind: KindId::from("file/source"),
            host_id: HostId::from("host/one"),
            boot_id: BootId::from("boot/one"),
            capability_id: CapabilityId::from("file/copy"),
        },
        base_instance_id: BaseInstanceId::from("base/files/instance"),
        base_provider_generation: 4,
        resource_pool_id: ResourcePoolId::from("files/project/source"),
        resource_generation_id: ResourceGenerationId("resource-generation/7".into()),
        operation_contract_id: HostCallContractId::from("conduit.host/file-read@1"),
        envelope_id: CapabilityEnvelopeId::from("file/read-only/project-source/max-64"),
        maximum_parameter_bytes: 256,
        maximum_result_bytes: 512,
        maximum_work_units: 32,
        maximum_in_flight: 4,
        maximum_operations: 10,
    }
}

fn request() -> CapabilityIssueRequest {
    CapabilityIssueRequest {
        scope: scope(),
        authority: authority(),
    }
}

fn table(key: u8) -> BaseCapabilityTable {
    BaseCapabilityTable::new(
        HostId::from("host/one"),
        BootId::from("boot/one"),
        BaseInstanceId::from("base/files/instance"),
        4,
        [key; 32],
        4,
    )
    .unwrap()
}

fn claim() -> BaseOperationClaim {
    let scope = scope();
    BaseOperationClaim {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_instance_id: scope.base_instance_id,
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id,
        active_play_id: scope.active_play_id,
        implementation_id: scope.implementation_id,
        operation_contract_id: scope.operation_contract_id,
        subject_kind: scope.subject_kind,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        parameter_bytes: 32,
        work_units: 2,
    }
}

fn prepared(provider: &mut NativeProvider) -> PreparedNativeSource<'_> {
    let mut table = table(7);
    let handle = table.issue(request()).unwrap();
    let operation = BoundBaseOperation::bind(table, handle, claim())
        .unwrap_or_else(|_| panic!("fixture binding"));
    PreparedNativeSource {
        node: NodeId(1),
        operation,
        provider,
        output: vec![0; 128].into_boxed_slice(),
        next_request: 0,
        pending: None,
        cancelled: false,
        cancellation_started: None,
    }
}
fn call(node: u16, sequence: u32) -> HostCallRequest {
    HostCallRequest {
        node: NodeId(node),
        request: RequestId(sequence),
        call: HostCallId(0),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 0,
            },
            64,
        )
        .unwrap(),
    }
}
struct Provider {
    calls: Arc<AtomicUsize>,
    bytes: usize,
    error: Option<String>,
}
// SAFETY: Copies one fixed byte or moves a prepared error; no IO, allocation,
// pending work, borrowed output or background activity escapes the call.
unsafe impl ImmediateNativeSource for Provider {
    fn read(&mut self, _: &[u8], output: &mut [u8]) -> Result<usize, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        output[0] = 42;
        Ok(self.bytes)
    }
}
fn provider(bytes: usize, error: Option<String>) -> (NativeProvider, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    (
        NativeProvider::Immediate(Box::new(Provider {
            calls: calls.clone(),
            bytes,
            error,
        })),
        calls,
    )
}
#[test]
fn immediate_success_and_exhaustion_use_no_boundary_allocations() {
    let (mut provider, calls) = provider(1, None);
    let mut source = prepared(&mut provider);
    let probe = crate::allocation_probe::begin();
    let result = source.read_immediate(call(1, 0), &[]).map(|b| b == [42]);
    let allocations = probe.finish();
    assert!(result.unwrap());
    assert_eq!(allocations, 0);
    let probe = crate::allocation_probe::begin();
    let result = source.read_immediate(call(1, 1), &[]);
    let allocations = probe.finish();
    assert!(matches!(
        result,
        Err(ImmediateReadFailure::Authorization(
            BaseCapabilityRefusal::Exhausted
        ))
    ));
    assert_eq!(allocations, 0);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
#[test]
fn bad_request_stop_and_sequence_exhaustion_refuse_before_provider_or_lease() {
    for mode in 0..3 {
        let (mut provider, calls) = provider(1, None);
        let mut source = prepared(&mut provider);
        if mode == 1 {
            source.cancel();
        }
        if mode == 2 {
            source.next_request = u32::MAX;
        }
        let request = call(
            if mode == 0 { 2 } else { 1 },
            if mode == 2 { u32::MAX } else { 0 },
        );
        let probe = crate::allocation_probe::begin();
        let result = source.read_immediate(request, &[]);
        let allocations = probe.finish();
        assert!(matches!(
            result,
            Err(ImmediateReadFailure::Request | ImmediateReadFailure::Sequence)
        ));
        assert_eq!(allocations, 0);
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(source.operation.inspection().in_flight_operations, 0);
    }
}
#[test]
fn oversized_result_revokes_and_prepared_provider_error_keeps_owned_failure_without_growth() {
    for oversized in [false, true] {
        let message = (!oversized).then(|| "prepared provider failure".to_owned());
        let (mut provider, calls) = provider(if oversized { 129 } else { 0 }, message);
        let mut source = prepared(&mut provider);
        let probe = crate::allocation_probe::begin();
        let result = source.read_immediate(call(1, 0), &[]);
        let allocations = probe.finish();
        if oversized {
            assert!(matches!(result, Err(ImmediateReadFailure::ResultEnvelope)));
        } else {
            assert!(
                matches!(result,Err(ImmediateReadFailure::Provider(ref s)) if s=="prepared provider failure")
            );
        }
        assert_eq!(allocations, 0);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(source.operation.inspection().in_flight_operations, 0);
        if oversized {
            assert_eq!(
                source.operation.inspection().lifecycle,
                CapabilityLifecycle::Revoked
            );
        }
    }
}
