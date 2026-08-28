//! Fail-closed errors shared by admitted remote ingress transports.

use conduit_wire::{stream_framing::StreamFrameError, WireError};

use crate::receipts::UsbSignError;

pub type RemoteResult<T> = Result<T, RemoteError>;

#[allow(
    dead_code,
    reason = "variants are shared across exclusive remote firmware modes"
)]
#[derive(Debug)]
pub enum RemoteError {
    UsbDisconnected,
    Framing(StreamFrameError),
    Codec(WireError),
    Sign(UsbSignError),
    BufferOverflow,
    InvalidGeneratedEndpoint,
    InvalidSignal,
    InvalidNetworkJoin,
    NetworkJoinFailed,
    NetworkJoinTimeout,
    NetworkConfigurationTimeout,
    Storage(conduit_kernel::StorageError),
    Kernel(conduit_kernel::scheduler::SchedulerError),
    SignStorage(conduit_kernel::SignError),
    KernelIdle,
    KernelCompletedEarly,
    KernelCancelled,
    KernelTerminalInvariant,
}

impl RemoteError {
    #[allow(dead_code)]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UsbDisconnected => "usb-disconnected",
            Self::Framing(_) => "malformed-stream-frame",
            Self::Codec(_) => "invalid-session-frame",
            Self::Sign(_) => "sign-channel-failure",
            Self::BufferOverflow => "bounded-buffer-overflow",
            Self::InvalidGeneratedEndpoint => "invalid-generated-endpoint",
            Self::InvalidSignal => "invalid-signal",
            Self::InvalidNetworkJoin => "invalid-network-join",
            Self::NetworkJoinFailed => "network-join-failed",
            Self::NetworkJoinTimeout => "network-join-timeout",
            Self::NetworkConfigurationTimeout => "network-configuration-timeout",
            Self::Storage(_) => "kernel-storage-failure",
            Self::Kernel(_) => "kernel-scheduler-failure",
            Self::SignStorage(_) => "kernel-sign-failure",
            Self::KernelIdle => "kernel-idle-before-effect",
            Self::KernelCompletedEarly => "kernel-completed-before-effect",
            Self::KernelCancelled => "kernel-cancelled",
            Self::KernelTerminalInvariant => "kernel-terminal-invariant",
        }
    }
}

impl From<StreamFrameError> for RemoteError {
    fn from(error: StreamFrameError) -> Self {
        Self::Framing(error)
    }
}

impl From<WireError> for RemoteError {
    fn from(error: WireError) -> Self {
        Self::Codec(error)
    }
}

impl From<UsbSignError> for RemoteError {
    fn from(error: UsbSignError) -> Self {
        Self::Sign(error)
    }
}
