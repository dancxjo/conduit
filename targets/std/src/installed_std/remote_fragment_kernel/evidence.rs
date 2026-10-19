//! Borrow the authoritative identity and retained Signs without changing the Play.
use super::InstalledRemoteFragment;
use conduit_kernel::HostedSignLog;
use conduit_plan_lowering::lowering::KernelIdentityMap;

impl InstalledRemoteFragment {
    /// Exact Plan/fragment mapping used by this kernel. This is not a new identity.
    pub fn kernel_identity(&self) -> &KernelIdentityMap {
        &self.lowered.identity
    }

    /// Read-only retained evidence, including its explicit retention gap and
    /// remote lifecycle identities. Reading does not acknowledge a value or
    /// release any kernel ownership. Callers own any descriptive reconstruction.
    pub fn kernel_signs(&self) -> &HostedSignLog {
        self.scheduler.signs()
    }
}
