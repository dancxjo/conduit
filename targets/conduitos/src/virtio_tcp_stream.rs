//! One TCP stream interface; native state belongs exclusively to the TCP guest.
use crate::{
    arch::{CandidateDeadline, VirtioNetError},
    virtio_tcp::{VirtioTcpEndpoint, VirtioTcpError},
};
use embedded_io::{Error, ErrorKind};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod local;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub(crate) use crate::protected_tcp::Stream as VirtioTcpStream;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub(crate) use local::VirtioTcpStream;
/// Proof sessions without an admitted clock advance smoltcp's internal timer
/// conservatively. Product admission uses a measured candidate deadline.
pub(super) const PROOF_POLLS_PER_PROTOCOL_TICK: u32 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VirtioTcpIoError(pub VirtioTcpError);

impl core::fmt::Display for VirtioTcpIoError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl core::error::Error for VirtioTcpIoError {}

impl Error for VirtioTcpIoError {
    fn kind(&self) -> ErrorKind {
        match self.0 {
            VirtioTcpError::InvalidEndpoint | VirtioTcpError::ClockRegressed => {
                ErrorKind::InvalidInput
            }
            VirtioTcpError::ConnectRefused => ErrorKind::ConnectionRefused,
            VirtioTcpError::RemoteClosed | VirtioTcpError::Cancelled => {
                ErrorKind::ConnectionAborted
            }
            VirtioTcpError::Timeout => ErrorKind::TimedOut,
            VirtioTcpError::SendRefused => ErrorKind::WriteZero,
            VirtioTcpError::ReceiveRefused
            | VirtioTcpError::RequestTooLarge
            | VirtioTcpError::ResponseTooLarge
            | VirtioTcpError::Device(_) => ErrorKind::Other,
        }
    }
}

/// The admitted route and bounded polling basis for one TCP connection.
#[derive(Clone, Copy)]
pub(crate) struct VirtioTcpConnectOptions {
    pub random_seed: u64,
    pub endpoint: VirtioTcpEndpoint,
    pub maximum_polls: u32,
    pub deadline: Option<CandidateDeadline>,
}

pub(super) fn report_read_refusal(code: &str) {
    crate::arch::early_write(b"CONDUIT_NATIVE_TCP_READ {\"code\":\"");
    crate::arch::early_write(code.as_bytes());
    crate::arch::early_write(b"\"}\n");
}

impl From<VirtioNetError> for VirtioTcpIoError {
    fn from(error: VirtioNetError) -> Self {
        Self(VirtioTcpError::Device(error))
    }
}
