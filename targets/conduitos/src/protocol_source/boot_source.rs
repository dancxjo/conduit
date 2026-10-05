//! Native boot observation feeds the same bounded Source preparation entrance.
use super::{PreparedProtocolEntry, ProtocolSourceRefusal};

pub const PROTOCOL_MODULE_COMMAND: &[u8] = b"conduit.protocol/source@1";

#[derive(Debug)]
pub enum ProtocolBootRefusal {
    UnsupportedBoot,
    Module(crate::boot::BootModuleRefusal),
    Source(ProtocolSourceRefusal),
}

/// Retain an explicitly selected entry from the exact named boot module.
/// Neither module presence nor successful checking grants review, membership,
/// controller ownership or authority to execute. The native root must admit
/// those independently before proposing the complete body workload.
pub fn prepare_boot_source(
    entry: &str,
) -> Result<Option<PreparedProtocolEntry>, ProtocolBootRefusal> {
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    ))]
    {
        let bytes =
            crate::boot::named_module(PROTOCOL_MODULE_COMMAND, super::MAXIMUM_PACKAGE_BYTES)
                .map_err(ProtocolBootRefusal::Module)?;
        bytes
            .map(|bytes| PreparedProtocolEntry::prepare(bytes, entry))
            .transpose()
            .map_err(ProtocolBootRefusal::Source)
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    )))]
    {
        let _ = entry;
        Err(ProtocolBootRefusal::UnsupportedBoot)
    }
}
