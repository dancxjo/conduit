//! Authored TM1637 Frame and packet realization, without environmental policy.
//! Construction grants no provider custody, caller authority or execution.
#![no_std]

mod frame;
pub use frame::{validate_display_frame, PreparedDisplayFrame, DISPLAY_FRAME_BYTES};

#[cfg(feature = "packets")]
mod packets;
#[cfg(feature = "packets")]
pub use packets::{DisplayPacketRefusal, PreparedDisplayPackets, SourceDisplayPackets};
