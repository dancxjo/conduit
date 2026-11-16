// Independently compiled finite kernel/storage geometry.
use super::*;
#[cfg(all(dictionary_sign_archive, not(all(fore_carrier, record_service))))]
compile_error!("archived duplex execution requires the checked single-result record service and Fore carrier");
pub(crate) const FORE_PORTS: usize = {
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
// Stock is derived from the complete independently emitted entry, not from
// thermometer behavior. Linked terminal matrices are checked against actual Backs.
use conduit_mcu_host_runtime::execution_profile as storage;
pub(super) const SQ: usize = source::sampling_layout::CORD_VALUE_SLOT_CAPACITIES.len();
pub(super) const OQ: usize = source::output_layout::CORD_VALUE_SLOT_CAPACITIES.len();
pub(super) const SF: usize =
    storage::phase_bytes(&source::sampling_layout::GENERATED_TERMINAL_TRANSDUCTIONS);
pub(super) const OF: usize =
    storage::phase_bytes(&source::output_layout::GENERATED_TERMINAL_TRANSDUCTIONS);
pub(super) const SU: usize =
    storage::unresolved_nodes(SN, source::sampling_layout::ABNORMAL_CORD_COUNT);
pub(super) const OU: usize =
    storage::unresolved_nodes(ON, source::output_layout::ABNORMAL_CORD_COUNT);
pub(super) const SZ: usize = storage::cord_terminals(
    SC,
    source::sampling_layout::ABNORMAL_CORD_COUNT,
    source::sampling_layout::EXTERNAL_INGRESS_CORD_COUNT,
);
pub(super) const OZ: usize = storage::cord_terminals(
    OC,
    source::output_layout::ABNORMAL_CORD_COUNT,
    source::output_layout::EXTERNAL_INGRESS_CORD_COUNT,
);
#[cfg(fore_carrier)]
pub(crate) const SAMPLING_LINE_SIGNS: conduit_mcu_host_runtime::execution_sign_budget::DuplexSignBudget =
    match conduit_mcu_host_runtime::execution_sign_budget::DuplexSignBudget::select(
        SS as u16, source::sampling_layout::PHYSICAL_SIGN_ITEM_CEILING,
        source::sampling_layout::PHYSICAL_SIGN_BYTE_CEILING as usize,
    ) {
        Some(budget) => budget,
        None => panic!("checked Source lacks finite duplex Sign capacity"),
    };
#[cfg(not(record_service))]
type RecordingDriver = conduit_mcu_host_runtime::execution_factory::NoExtraBack;
#[cfg(record_service)]
type RecordingDriver = conduit_mcu_host_runtime::execution_factory::BoxedRecordServiceBack<
    SP,
    { conduit_resource_snapshot::read_result::READ_RESULT_BYTES },
>;
#[cfg(not(dictionary_sign_archive))]
pub(crate) type SamplingSignLog = HostedSignLog;
#[cfg(dictionary_sign_archive)]
type SamplingArchive = conduit_kernel::dictionary_live_sign_archive::LivePrefixArchive<
    { crate::config::DICTIONARY_ARCHIVE_EVENTS },
    { crate::config::DICTIONARY_ARCHIVE_TEMPLATES },
    { crate::config::DICTIONARY_ARCHIVE_SEGMENTS },
    { crate::config::DICTIONARY_ARCHIVE_REQUEST_LANES },
    { crate::config::DICTIONARY_ARCHIVE_REMOTE_STREAMS },
>;
#[cfg(dictionary_sign_archive)]
pub(crate) type SamplingSignLog = conduit_kernel::dictionary_live_sign_archive::DictionaryLiveSignLog<
    { crate::config::DICTIONARY_ARCHIVE_EVENTS },
    { crate::config::DICTIONARY_ARCHIVE_TEMPLATES },
    { crate::config::DICTIONARY_ARCHIVE_SEGMENTS },
    { crate::config::DICTIONARY_ARCHIVE_REQUEST_LANES },
    { crate::config::DICTIONARY_ARCHIVE_REMOTE_STREAMS },
>;
#[cfg(dictionary_sign_archive)]
const _: () = assert!(SamplingSignLog::required_archive_backing_bytes()
    <= crate::config::DICTIONARY_ARCHIVE_LIMITS.archive_backing_bytes);
pub type SamplingKernel<'a, 'f> = ExecutionKernel<
    Driver<'f, resident::BorrowedBack<'a>, RecordingDriver>,
    SN,
    SC,
    SP,
    SR,
    ST,
    SH,
    SS,
    SamplingSignLog,
    SK,
    HostedValueStore,
    SQ,
    0,
    SF,
    SU,
    SZ,
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
    HostedValueStore,
    OQ,
    0,
    OF,
    OU,
    OZ,
>;
#[used]
#[no_mangle]
pub static CONDUIT_EXECUTION_KERNEL_BYTES: [usize; 3] = [
    core::mem::size_of::<SamplingKernel<'static, 'static>>() + sampling_sign_external_bytes(),
    core::mem::size_of::<conduit_seven_segment_mask::native_kernel::Kernel>(),
    core::mem::size_of::<OutputKernel<'static, 'static>>() + sign_bytes(OS, OS),
];
const fn sampling_sign_external_bytes() -> usize {
    let active = {
        #[cfg(fore_carrier)] { match SAMPLING_LINE_SIGNS.backing_bytes() { Some(n) => n, None => panic!("finite Sign geometry") } }
        #[cfg(not(fore_carrier))] { sign_bytes(SS, 0) }
    };
    // The wrapper/Box pointer is already part of SamplingKernel. Count only
    // the separately allocated codec, in addition to the active Sign backing.
    #[cfg(dictionary_sign_archive)]
    { match active.checked_add(core::mem::size_of::<SamplingArchive>()) {
        Some(bytes) => bytes, None => panic!("archive Sign geometry"),
    } }
    #[cfg(not(dictionary_sign_archive))]
    { active }
}
#[cfg(not(record_service))]
pub type SamplingCalls = conduit_mcu_host_runtime::execution_factory::CallsWithOptionalAppender<
    conduit_mcu_host_runtime::execution_factory::Calls<Pulse, Wire, MonotonicTimer, WAIT_SLOTS>,
    RecordingStorage,
>;
#[cfg(record_service)]
pub type SamplingCalls = conduit_mcu_host_runtime::execution_factory::CallsWithRecordService<
    conduit_mcu_host_runtime::execution_factory::Calls<Pulse, Wire, MonotonicTimer, WAIT_SLOTS>,
    RecordingStorage,
>;
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
