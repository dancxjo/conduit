// Independently compiled finite kernel/storage geometry.
use super::*;
pub(super) const FORE_PORTS: usize = {
    let a = source::sampling_layout::FORE_PORTS;
    let b = source::mask_layout::FORE_PORTS;
    let c = source::output_layout::FORE_PORTS;
    let ab = if a > b { a } else { b };
    if ab > c {
        ab
    } else {
        c
    }
};
macro_rules! shape {
    ($name:ident,$module:ident,$tag:ident) => {
        pub(super) const $name: usize = source::$module::COUNTS[(c::$tag - 1) as usize] as usize;
    };
}
shape!(SN, sampling_layout, ASSIGNED_NODE);
shape!(SC, sampling_layout, ASSIGNED_CORD);
pub(super) const SR: usize = SN * SP;
shape!(ST, sampling_layout, ASSIGNED_ROUTE_TARGET);
// Fixed tables are indexed by node/port, not packed record counts.
pub(super) const SH: usize = source::sampling_layout::HOST_CALL_SLOTS;
pub(super) const SK: usize = source::sampling_layout::PENDING_HOST_CALL_SLOTS;
shape!(ON, output_layout, ASSIGNED_NODE);
shape!(OC, output_layout, ASSIGNED_CORD);
pub(super) const OR: usize = ON * OP;
shape!(OT, output_layout, ASSIGNED_ROUTE_TARGET);
pub(super) const OH: usize = source::output_layout::HOST_CALL_SLOTS;
pub(super) const OK: usize = source::output_layout::PENDING_HOST_CALL_SLOTS;
pub(super) const SP: usize = source::sampling_layout::PORTS_PER_NODE;
pub(super) const OP: usize = source::output_layout::PORTS_PER_NODE;
pub(super) const SS: usize = source::sampling_layout::SIGN_ITEMS;
pub(super) const OS: usize = source::output_layout::SIGN_ITEMS;
#[cfg(not(record_service))]
type RecordingDriver = conduit_mcu_host_runtime::execution_factory::NoExtraBack;
#[cfg(record_service)]
type RecordingDriver = conduit_mcu_host_runtime::execution_factory::BoxedRecordServiceBack<SP,
    { conduit_resource_snapshot::read_result::READ_RESULT_BYTES }>;
pub type SamplingKernel<'a, 'f> = ExecutionKernel<
    Driver<'f, resident::BorrowedBack<'a>, RecordingDriver>,
    SN,
    SC,
    SP,
    SR,
    ST,
    SH,
    SS,
    HostedSignLog,
    SK,
>;
pub type OutputKernel<'a, 'f> = ExecutionKernel<
    Driver<'f, resident::BorrowedBack<'a>>,
    ON,
    OC,
    OP,
    OR,
    OT,
    OH,
    OS,
    HostedSignLog,
    OK,
>;
#[used]
#[no_mangle]
pub static CONDUIT_EXECUTION_KERNEL_BYTES: [usize; 3] = [
    core::mem::size_of::<SamplingKernel<'static, 'static>>() + sign_bytes(SS, 0),
    core::mem::size_of::<conduit_seven_segment_mask::native_kernel::Kernel>(),
    core::mem::size_of::<OutputKernel<'static, 'static>>() + sign_bytes(OS, OS),
];
#[cfg(not(record_service))]
pub type SamplingCalls = conduit_mcu_host_runtime::execution_factory::CallsWithOptionalAppender<
    conduit_mcu_host_runtime::execution_factory::Calls<Pulse, Wire, MonotonicTimer, WAIT_SLOTS>, RecordingStorage>;
#[cfg(record_service)]
pub type SamplingCalls = conduit_mcu_host_runtime::execution_factory::CallsWithRecordService<
    conduit_mcu_host_runtime::execution_factory::Calls<Pulse, Wire, MonotonicTimer, WAIT_SLOTS>, RecordingStorage>;
pub(super) type SamplingBound<'a, 'f> = (Box<SamplingKernel<'a, 'f>>, SamplingCalls);
pub(super) type OutputBound<'a, 'f> = (
    Box<OutputKernel<'a, 'f>>,
    conduit_mcu_host_runtime::execution_factory::Calls<Pulse, Wire, MonotonicTimer, 0>,
);

const fn sign_bytes(items: usize, remote: usize) -> usize {
    match HostedSignLog::canonical_storage_bytes(items as u16, remote as u16) {
        Some(bytes) => bytes,
        None => panic!("checked Sign storage geometry"),
    }
}
