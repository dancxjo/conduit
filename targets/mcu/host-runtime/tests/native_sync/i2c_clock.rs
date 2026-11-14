//! Real bound native owners and the actual kernel; CPU providers, no physical
//! attachment, whole Body admission or durable recording claim.
use super::*;
use conduit_core as c;
use conduit_i2c_transaction::{
    contract::{I2cContract, I2C_CALL, I2C_MAXIMUM_BYTES, I2C_RESULT_VALUE_BYTES},
    owner::{I2cAttachment, I2cCallSelection, I2cHostCall},
    I2cDisposition, I2cProvider, I2cTransaction,
};
use conduit_mcu_host_runtime::native_dispatch::deadline;
use conduit_monotonic_wait::clock_at::{
    codec::ClockDisposition,
    contract::{MonotonicClockContract, CLOCK_CALL, CLOCK_MAXIMUM_BYTES},
    owner::{
        ClockCallSelection, MonotonicClockHostCall, MonotonicDeadlineProvider, MAXIMUM_POLL_STEPS,
    },
};

fn scope(kind: &c::Kind, call: &str, maximum: u32, work: u64) -> c::BaseCapabilityScope {
    let mut s = fixture::scope();
    s.base_instance_id = format!("fixture/{}", kind.kind_id.as_str()).into();
    s.resource_pool_id = format!("pool/{}", kind.kind_id.as_str()).into();
    s.subject_kind = kind.kind_id.clone();
    s.operation_contract_id = call.into();
    s.maximum_parameter_bytes = maximum;
    s.maximum_result_bytes = maximum;
    s.maximum_work_units = work;
    s.maximum_operations = 2;
    s
}
struct Bus {
    effects: Rc<Cell<usize>>,
    revoked: Rc<Cell<bool>>,
}
impl I2cProvider for Bus {
    fn transact(
        &mut self,
        request: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert!(!self.revoked.get());
        self.effects.set(self.effects.get() + 1);
        let n = usize::from(request.read_length());
        input[..n].fill(0x60);
        Ok(n)
    }
    fn revoke(&mut self) {
        self.revoked.set(true);
    }
}
fn bus(effects: Rc<Cell<usize>>, revoked: Rc<Cell<bool>>) -> I2cHostCall<Bus> {
    let contract = I2cContract::prepare().unwrap();
    let s = scope(contract.kind(), I2C_CALL, I2C_MAXIMUM_BYTES, 1);
    let (fragment, lowered, active, placement) = fixture::selection_fixture::selected(
        &s,
        contract.kind(),
        I2C_CALL,
        I2C_MAXIMUM_BYTES,
        "i2c",
        "i2c",
    );
    bind_bus(
        s,
        Bus { effects, revoked },
        I2cCallSelection {
            contract: &contract,
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement: &placement,
        },
    )
}
fn bind_bus(
    s: c::BaseCapabilityScope,
    provider: Bus,
    selection: I2cCallSelection<'_>,
) -> I2cHostCall<Bus> {
    let (table, handle, claim) = fixture::authorization_with_bounds(
        s,
        selection.fragment,
        selection.active,
        I2C_MAXIMUM_BYTES,
        1,
    );
    // SAFETY: exclusively held finite synchronous CPU provider and exact fixture
    // attachment. The original selected Plan is checked by the native binder.
    unsafe {
        I2cHostCall::bind_selected(
            table,
            handle,
            claim,
            I2cAttachment {
                generation: 1,
                minimum_address: 0x70,
                maximum_address: 0x76,
                resource_bytes: 4096,
            },
            provider,
            selection,
        )
    }
    .unwrap()
}

fn bus_request(address: u8) -> Vec<u8> {
    let ty = I2cContract::prepare()
        .unwrap()
        .decode_request_type()
        .unwrap();
    let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let field = |name| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    let octet = c::StructuredInfoType::leaf(c::kind_id("value/u8")).unwrap();
    c::StructuredInfoValue::record(
        ty.clone(),
        vec![
            c::StructuredFieldValue::new(
                "address",
                c::StructuredInfoValue::leaf(field("address"), vec![address]).unwrap(),
            )
            .unwrap(),
            c::StructuredFieldValue::new(
                "read",
                c::StructuredInfoValue::leaf(field("read"), vec![1]).unwrap(),
            )
            .unwrap(),
            c::StructuredFieldValue::new(
                "write",
                c::StructuredInfoValue::sequence(
                    field("write"),
                    vec![c::StructuredInfoValue::leaf(octet, vec![0xd0]).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
struct Clock {
    polls: Rc<Cell<usize>>,
    next: Rc<Cell<Option<u64>>>,
    revoked: Rc<Cell<bool>>,
}
impl MonotonicDeadlineProvider for Clock {
    fn poll_until(&mut self, _: u64) -> Result<Option<u64>, ClockDisposition> {
        assert!(!self.revoked.get());
        self.polls.set(self.polls.get() + 1);
        Ok(self.next.take())
    }
    fn revoke(&mut self) {
        self.revoked.set(true);
    }
}
fn clock(
    polls: Rc<Cell<usize>>,
    next: Rc<Cell<Option<u64>>>,
    revoked: Rc<Cell<bool>>,
) -> MonotonicClockHostCall<Clock> {
    let contract = MonotonicClockContract::prepare().unwrap();
    let s = scope(
        contract.kind(),
        CLOCK_CALL,
        CLOCK_MAXIMUM_BYTES,
        MAXIMUM_POLL_STEPS,
    );
    let (fragment, lowered, active, placement) = fixture::selection_fixture::selected(
        &s,
        contract.kind(),
        CLOCK_CALL,
        CLOCK_MAXIMUM_BYTES,
        "clock",
        "clock",
    );
    bind_clock(
        s,
        Clock {
            polls,
            next,
            revoked,
        },
        ClockCallSelection {
            contract: &contract,
            fragment: &fragment,
            lowered: &lowered,
            active: &active,
            placement: &placement,
        },
    )
}
fn bind_clock(
    s: c::BaseCapabilityScope,
    provider: Clock,
    selection: ClockCallSelection<'_>,
) -> MonotonicClockHostCall<Clock> {
    let (table, handle, claim) = fixture::authorization_with_bounds(
        s,
        selection.fragment,
        selection.active,
        CLOCK_MAXIMUM_BYTES,
        MAXIMUM_POLL_STEPS,
    );
    // SAFETY: exclusive bounded CPU clock, exact independently issued scope;
    // polls never block or allocate and revocation quiesces future observations.
    unsafe { MonotonicClockHostCall::bind_selected(table, handle, claim, provider, selection) }
        .unwrap()
}

fn clock_request() -> Vec<u8> {
    let ty = MonotonicClockContract::prepare()
        .unwrap()
        .request_type()
        .clone();
    let c::StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let instant =
        c::StructuredInfoValue::leaf(fields[0].value_type().clone(), 20u64.to_le_bytes().to_vec())
            .unwrap();
    c::StructuredInfoValue::record(
        ty,
        vec![c::StructuredFieldValue::new("deadline", instant).unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
struct Request {
    input: ValueRef,
    maximum: u32,
    tag: &'static str,
    started: bool,
}
impl StepBack<1> for Request {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        if !self.started {
            io.request_host_call(
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(self.input, self.maximum).unwrap(),
            )
            .unwrap();
            self.started = true;
            return StepOutcome::Progress;
        }
        if let Some((_, result)) = io.host_completion() {
            assert_eq!(result.disposition, HostCallDisposition::Completed);
            let value =
                c::validate_canonical_structured_value(bytes.host_output().unwrap()).unwrap();
            assert_eq!(value.variant_tag().unwrap(), self.tag);
            io.consume_host_completion().unwrap();
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}
fn request_kernel(
    input: &[u8],
    maximum: u32,
    result: u32,
    value_bytes: u32,
    tag: &'static str,
    fail: Rc<Cell<bool>>,
) -> StoredExecutionKernel<Request, Signs, 1, 1, 1, 0, 0, 1> {
    let mut values = HostedValueStore::new(4, I2C_MAXIMUM_BYTES, value_bytes).unwrap();
    let input = values.store(input).unwrap();
    kernel_with_back(
        values,
        Request {
            input,
            maximum,
            tag,
            started: false,
        },
        maximum,
        result,
        fail,
    )
}
#[path = "io/deadline_tests.rs"]
mod deadline_tests;
#[path = "io/i2c_tests.rs"]
mod i2c_tests;
#[path = "io/runner_tests.rs"]
mod runner_tests;
