//! Exact generated identity of the Pico distributed-Lenia fragment.

mod generated {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/pico_signal_image.rs"));
}

pub const PLAN_ID: &str = generated::PLAN_ID;
pub const HOST_ID: &str = generated::HOST_ID;
pub const FIRMWARE_BUILD_ID: &str = generated::LENIA_FIRMWARE_BUILD_ID;

pub const WORK_PLAY_ID: &str = generated::LENIA_WORK_PLAY_ID;
pub const WORK_LINE_ID: &str = generated::LENIA_WORK_LINE_ID;
pub const WORK_SOURCE_HOST_ID: &str = generated::LENIA_WORK_SOURCE_HOST_ID;
pub const WORK_SOURCE_BOOT_ID: &str = generated::LENIA_WORK_SOURCE_BOOT_ID;
pub const WORK_SINK_HOST_ID: &str = generated::LENIA_WORK_SINK_HOST_ID;

pub const RESULT_PLAY_ID: &str = generated::LENIA_RESULT_PLAY_ID;
pub const RESULT_LINE_ID: &str = generated::LENIA_RESULT_LINE_ID;
pub const RESULT_SINK_HOST_ID: &str = generated::LENIA_RESULT_SINK_HOST_ID;
pub const RESULT_SINK_BOOT_ID: &str = generated::LENIA_RESULT_SINK_BOOT_ID;
