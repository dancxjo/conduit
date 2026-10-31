//! Retention for the pinned four-node output graph and its bounded local Fore.
//! This is a storage bound, never caller, GPIO or Body authority.
use conduit_kernel as k;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplaySignBudget {
    frames: u32,
    ordinary_items: u16,
    ordinary_bytes: u32,
    remote_items: u16,
    remote_bytes: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignBudgetRefusal {
    ZeroWork,
    InvalidMinimum,
    Overflow,
}

impl DisplaySignBudget {
    /// The exclusive Fore owner admits at most this many inputs, including
    /// brightness refusals, and stops on the first kernel/service failure.
    /// The graph has four nodes, five payload cords, one port per node and
    /// exactly one ingress/refusal egress. No recovery/terminal transduction
    /// is selected. Changing those laws requires renewing this bound.
    pub fn derive(
        frames: u32,
        minimum_items: u16,
        minimum_bytes: u32,
    ) -> Result<Self, SignBudgetRefusal> {
        use SignBudgetRefusal as E;
        if frames == 0 {
            return Err(E::ZeroWork);
        }
        if minimum_items == 0 {
            return Err(E::InvalidMinimum);
        }
        // One admitted ingress + offer/accept/deliver of a refusal per input.
        // Either Fore can close once. Retries do not publish another identity.
        let remote = frames
            .checked_mul(4)
            .and_then(|n| n.checked_add(2))
            .ok_or(E::Overflow)?;
        // All four completions, two conservative fault slots and RunCancelled.
        // The caller stops after the first fault; repeated failing Steps are
        // not admitted work. Cancellation is idempotent in the ordinary kernel.
        const NODES: u32 = 4;
        const CORDS: u32 = 5;
        const PORTS: u32 = 1;
        let retained = remote.checked_add(NODES + 2 + 1).ok_or(E::Overflow)?;
        // Conservative simultaneous admission burst: input consumption/close,
        // all routing + possible terminal publications, remote output close,
        // host request/cancel, Back terminal/recovery and the fuel grant.
        // Some terms cannot coexist; keeping them covers preflight, pending
        // calls and cleanup without making optional telemetry permanent.
        const BURST: u32 = PORTS + PORTS + CORDS + CORDS + 1 + 5;
        let charge =
            u32::try_from(core::mem::size_of::<k::KernelEvent>()).map_err(|_| E::Overflow)?;
        let ordinary = retained
            .checked_add(BURST)
            .ok_or(E::Overflow)?
            .max(u32::from(minimum_items))
            .max(minimum_bytes.div_ceil(charge));
        Ok(Self {
            frames,
            ordinary_items: ordinary.try_into().map_err(|_| E::Overflow)?,
            ordinary_bytes: ordinary.checked_mul(charge).ok_or(E::Overflow)?,
            remote_items: remote.try_into().map_err(|_| E::Overflow)?,
            remote_bytes: k::remote_sign_storage_bytes(remote.try_into().map_err(|_| E::Overflow)?)
                .ok_or(E::Overflow)?,
        })
    }
    pub fn maximum_frames(self) -> u32 {
        self.frames
    }
    pub fn ordinary_items(self) -> u16 {
        self.ordinary_items
    }
    pub fn ordinary_bytes(self) -> u32 {
        self.ordinary_bytes
    }
    pub fn remote_items(self) -> u16 {
        self.remote_items
    }
    pub fn remote_bytes(self) -> u32 {
        self.remote_bytes
    }
    pub fn storage_bytes(self) -> Option<usize> {
        k::HostedSignLog::canonical_storage_bytes(self.ordinary_items, self.remote_items)
    }
}
