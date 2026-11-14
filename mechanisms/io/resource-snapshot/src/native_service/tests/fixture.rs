use super::*;
use alloc::{vec, vec::Vec};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone)]
pub struct Stats {
    pub reads: Rc<Cell<usize>>,
    pub programs: Rc<Cell<usize>>,
}
pub struct Storage {
    bytes: RefCell<Vec<u8>>,
    stats: Stats,
    fail: bool,
}
impl NorStorage for Storage {
    fn capacity(&self) -> u32 {
        4096
    }
    fn program_unit(&self) -> u32 {
        256
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), crate::nor::NorRefusal> {
        self.stats.reads.set(self.stats.reads.get() + 1);
        out.copy_from_slice(&self.bytes.borrow()[at as usize..at as usize + out.len()]);
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), crate::nor::NorRefusal> {
        self.stats.programs.set(self.stats.programs.get() + 1);
        if self.fail {
            return Err(crate::nor::NorRefusal::Media);
        }
        for (old, new) in self.bytes.borrow_mut()[at as usize..].iter_mut().zip(bytes) {
            *old &= *new;
        }
        Ok(())
    }
}
fn grant(read: bool) -> c::AuthorityGrant {
    c::AuthorityGrant {
        grant_id: if read { "grant/read" } else { "grant/append" }.into(),
        contract_id: if read {
            READ_AUTHORITY
        } else {
            crate::native_append::APPEND_AUTHORITY
        }
        .into(),
        host_call_contract_id: if read {
            READ_OPERATION
        } else {
            crate::native_append::APPEND_OPERATION
        }
        .into(),
        subject_kind: if read {
            crate::read_contract::READ_KIND
        } else {
            crate::append_contract::APPEND_KIND
        }
        .into(),
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        capability_id: SERVICE_CAPABILITY.into(),
    }
}
fn operation(read: bool) -> c::BoundBaseOperation {
    let grant = grant(read);
    let parameter = if read { 8 } else { 200 };
    let work = if read { 2 } else { 6 };
    let result = if read {
        crate::read_result::READ_RESULT_BYTES as u32
    } else {
        crate::append_result::APPEND_RESULT_BYTES
    };
    let scope = c::BaseCapabilityScope {
        host_id: grant.host_id.clone(),
        boot_id: grant.boot_id.clone(),
        base_instance_id: "base/instance".into(),
        base_provider_generation: 1,
        plan_id: "plan/test".into(),
        active_play_id: "play/test".into(),
        authority_grant_id: grant.grant_id.clone(),
        authority_contract_id: grant.contract_id.clone(),
        capability_id: grant.capability_id.clone(),
        implementation_id: "service/test".into(),
        operation_contract_id: grant.host_call_contract_id.clone(),
        subject_kind: grant.subject_kind.clone(),
        resource_pool_id: "pool/test".into(),
        resource_generation_id: c::ResourceGenerationId("generation/test".into()),
        envelope_id: "envelope/test".into(),
        maximum_parameter_bytes: parameter,
        maximum_result_bytes: result,
        maximum_work_units: work,
        maximum_in_flight: 1,
        maximum_operations: 16,
    };
    let authority = c::BaseCapabilityAuthority {
        grant,
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: parameter,
        maximum_result_bytes: result,
        maximum_work_units: work,
        maximum_in_flight: 1,
        maximum_operations: 16,
    };
    let claim = c::BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: 1,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: parameter,
        work_units: work,
    };
    let mut table = c::BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        1,
        [if read { 2 } else { 1 }; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(c::CapabilityIssueRequest { scope, authority })
        .unwrap();
    c::BoundBaseOperation::bind(table, handle, claim)
        .unwrap_or_else(|_| panic!("actual operation bind"))
}
pub fn fixture(fail: bool) -> (RecordServiceHostCall<Storage>, Stats) {
    let stats = Stats {
        reads: Rc::new(Cell::new(0)),
        programs: Rc::new(Cell::new(0)),
    };
    let storage = Storage {
        bytes: RefCell::new(vec![255; 4096]),
        stats: stats.clone(),
        fail,
    };
    let mut provider =
        PreparedNorRecordResidence::prepare(storage, RecordResidenceId([3; 32]), 0, 8)
            .unwrap_or_else(|_| panic!("geometry"));
    provider.recover().unwrap();
    let identity = AppendNativeIdentity {
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        base_id: "base/test".into(),
        provider_instance_id: "base/instance".into(),
        provider_generation: 1,
        resource_pool_id: "pool/test".into(),
        resource_generation_id: c::ResourceGenerationId("generation/test".into()),
        envelope_id: "envelope/test".into(),
        artifact_id: "artifact/test".into(),
        implementation_id: "service/test".into(),
        base_implementation_id: "base/implementation".into(),
    };
    // SAFETY: sole CPU NOR owner, bounded synchronous operations and test grants.
    let ready = unsafe {
        ReadyRecordService::new_retained(
            identity,
            provider,
            [grant(false), grant(true)],
            ServiceLimits {
                append_operations: 16,
                read_operations: 16,
            },
        )
    }
    .unwrap_or_else(|_| panic!("ready"));
    use conduit_plan_lowering::selected_operation_set::SelectedServiceRoute;
    // This fixture exercises execution only. Production selection is exclusively
    // through prepare_selected_retained and the separately tested checked binder.
    let owner = RecordServiceHostCall {
        ready,
        operations: [operation(false), operation(true)],
        routes: [
            SelectedServiceRoute {
                call: HostCallId(1),
                input: conduit_kernel::PortId(0),
                output: conduit_kernel::PortId(0),
                maximum_input_bytes: 200,
                maximum_output_bytes: crate::append_result::APPEND_RESULT_BYTES,
            },
            SelectedServiceRoute {
                call: HostCallId(0),
                input: conduit_kernel::PortId(1),
                output: conduit_kernel::PortId(1),
                maximum_input_bytes: 8,
                maximum_output_bytes: crate::read_result::READ_RESULT_BYTES as u32,
            },
        ],
        node: NodeId(3),
        next_request: 0,
        cached: None,
        input: [0; 200],
        custody: None,
        append_failed: false,
        terminal: false,
    };
    (owner, stats)
}

pub fn recovered_provider() -> PreparedNorRecordResidence<Storage> {
    let stats = Stats {
        reads: Rc::new(Cell::new(0)),
        programs: Rc::new(Cell::new(0)),
    };
    let storage = Storage {
        bytes: RefCell::new(vec![255; 4096]),
        stats,
        fail: false,
    };
    let mut provider =
        PreparedNorRecordResidence::prepare(storage, RecordResidenceId([3; 32]), 0, 8)
            .unwrap_or_else(|_| panic!("geometry"));
    provider.recover().unwrap();
    provider
}
