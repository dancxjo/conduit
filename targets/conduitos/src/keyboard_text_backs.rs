//! The single canonical production Back implementation shared with the runtime guest.
pub(crate) use conduitos_runtime_core::backs::{
    FaceBack, KeyboardBack, PlannedBack, StreamTransformBack,
};
const _: () = assert!(
    conduitos_runtime_core::PORTS
        == conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE
);
