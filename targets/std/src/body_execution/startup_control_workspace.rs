//! Fixed joined-control frames owned before the original Body starts.
//! TLS and checked Source retain their own admission.
use super::{
    DelegatedBodyPreparationReceipt, DelegatedBodyTerminalReceipt, StartupChimeCallReceipt,
    StartupOwnerChallenge,
};
use crate::{kernel_preparation::ImageMemoryLease, StdHost};
use std::sync::{Arc, Mutex};

// Same finite joined frame limit as the existing product rendezvous protocol.
pub const STARTUP_MEMBER_CONTROL_FRAME_BYTES: usize = 512 * 1024;
pub const STARTUP_MEMBER_MAXIMUM_CHIME_RECEIPTS: usize =
    crate::installed_std::MAXIMUM_INSTALLED_NODES;

pub struct StartupMemberControlWorkspace {
    write: Vec<u8>,
    read: Vec<u8>,
    // Physical frame owners drop before the original counter lease.
    _memory: Option<ImageMemoryLease>,
}

impl StdHost {
    /// The existing owner retains this workspace through terminal or unknown
    /// outcomes. No arbitrary byte grant, counter replacement or growth API.
    pub fn prepare_startup_member_control_workspace(
        &self,
    ) -> Result<StartupMemberControlWorkspace, String> {
        self.kernel_resources.require_owner_identity_preparation()?;
        let bytes = backing_bytes()?;
        let memory = if self.kernel_resources.has_image_bounds() {
            Some(self.kernel_resources.fund_tick_preparation(bytes)?)
        } else {
            None
        };
        let prepare = || -> Result<Vec<u8>, String> {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(STARTUP_MEMBER_CONTROL_FRAME_BYTES)
                .map_err(|_| "startup control frame allocation failed")?;
            bytes.resize(STARTUP_MEMBER_CONTROL_FRAME_BYTES, 0);
            Ok(bytes)
        };
        Ok(StartupMemberControlWorkspace {
            write: prepare()?,
            read: prepare()?,
            _memory: memory,
        })
    }
}

fn backing_bytes() -> Result<usize, String> {
    use core::mem::size_of;
    let (shared, _) = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<
            Mutex<StartupMemberControlWorkspace>,
        >())
        .map_err(|_| "startup control shared owner backing overflow")?;
    let mut bytes = shared.pad_to_align().size();
    // These are distinct owners: two actual frame buffers, decoded reply
    // strings, and the JSON decoder's reusable escaped-string scratch. The
    // power-of-two frame limit also bounds its Vec growth capacity.
    for owned in [
        STARTUP_MEMBER_CONTROL_FRAME_BYTES,           // transmitted frame
        STARTUP_MEMBER_CONTROL_FRAME_BYTES,           // received frame
        STARTUP_MEMBER_CONTROL_FRAME_BYTES,           // bounded decoded field ownership/diagnostics
        STARTUP_MEMBER_CONTROL_FRAME_BYTES,           // decoder scratch
        12 * 256, // retained exact preparation/session identity strings
        size_of::<DelegatedBodyPreparationReceipt>(), // retained receipt
        size_of::<DelegatedBodyPreparationReceipt>(), // current decoded progress
        size_of::<DelegatedBodyTerminalReceipt>(),
        size_of::<StartupOwnerChallenge>(),
        size_of::<(
            conduit_body::MembershipCredential,
            conduit_body::FederationPeer,
            [u8; 32],
        )>(),
        STARTUP_MEMBER_MAXIMUM_CHIME_RECEIPTS * size_of::<StartupChimeCallReceipt>(),
        ImageMemoryLease::owned_backing_bytes(),
    ] {
        bytes = bytes
            .checked_add(owned)
            .ok_or("startup control workspace backing overflow")?;
    }
    Ok(bytes)
}

impl StartupMemberControlWorkspace {
    /// Both coordinator and sole carrier worker retain the same original
    /// charge. Its Arc/Mutex storage was included before either frame existed.
    pub fn into_shared(self) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(self))
    }
    pub fn write_buffer(&mut self) -> &mut [u8] {
        &mut self.write
    }
    pub fn read_buffer(&mut self) -> &mut [u8] {
        &mut self.read
    }
}
