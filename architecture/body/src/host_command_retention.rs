//! Bounded RAM custody of previously authenticated input, never execution rights.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCommandRetentionRefusal {
    Bounds,
    Occupied,
    NotRetained,
    Payload,
    Context,
    Clock,
}

/// Consumes one attributed command. No strings/catalog or signature are copied;
/// the transcript binds all original context facts and the exact payload.
/// This is not Clone/Deserialize, a new challenge, or permission to start work.
pub struct RetainedHostCommandStamp {
    transcript: [u8; 32],
    payload_digest: [u8; 32],
    payload_bytes: u32,
    maximum_bytes: u32,
    nonce: [u8; 32],
    issued_at: u64,
    expires_at: u64,
}

impl RetainedHostCommandStamp {
    /// Read-only identity of the original authenticated custody attempt.
    /// This digest grants no preparation, admission, or execution rights.
    pub fn custody_identity(&self) -> [u8; 32] {
        self.transcript
    }

    /// Match the original attributed attempt, not merely an identical payload.
    /// A fresh challenge for the same bytes cannot replace admitted custody.
    /// The command must first be reborrowed under current independent facts.
    pub fn check_command(
        &self,
        command: &AttributedHostCommand<'_, '_>,
    ) -> Result<(), HostCommandRetentionRefusal> {
        if host_command_attribution_transcript(command.challenge(), command.payload())
            .map_err(|_| HostCommandRetentionRefusal::Context)?
            != self.transcript
        {
            return Err(HostCommandRetentionRefusal::Context);
        }
        Ok(())
    }
}

/// One fixed, caller-funded input buffer. Retained bytes are immutable. Root must
/// still perform policy/admission/retirement checks before interpreting it, and
/// must own its consumption separately from transport delivery acknowledgments.
pub struct HostCommandInput<const N: usize> {
    stamp: Option<RetainedHostCommandStamp>,
    bytes: [u8; N],
}

impl AttributedHostCommand<'_, '_> {
    pub fn into_retained_stamp(
        self,
    ) -> Result<RetainedHostCommandStamp, HostCommandRetentionRefusal> {
        let c = self.challenge;
        Ok(RetainedHostCommandStamp {
            transcript: host_command_attribution_transcript(&c, self.payload)
                .map_err(|_| HostCommandRetentionRefusal::Bounds)?,
            payload_digest: Sha256::digest(self.payload).into(),
            payload_bytes: u32::try_from(self.payload.len())
                .map_err(|_| HostCommandRetentionRefusal::Bounds)?,
            maximum_bytes: c.maximum_payload_bytes,
            nonce: c.nonce,
            issued_at: c.issued_at_millis,
            expires_at: c.expires_at_millis,
        })
    }
}

impl<const N: usize> Default for HostCommandInput<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> HostCommandInput<N> {
    /// Initialize the caller's original storage without a second N-byte value.
    pub fn initialize_in_place(storage: &mut core::mem::MaybeUninit<Self>) -> &mut Self {
        let target = storage.as_mut_ptr();
        // SAFETY: both fields are fully initialized before the reference exists.
        unsafe {
            core::ptr::addr_of_mut!((*target).stamp).write(None);
            core::ptr::addr_of_mut!((*target).bytes)
                .cast::<u8>()
                .write_bytes(0, N);
            storage.assume_init_mut()
        }
    }
    pub const fn new() -> Self {
        Self {
            stamp: None,
            bytes: [0; N],
        }
    }
    pub fn is_retained(&self) -> bool {
        self.stamp.is_some()
    }
    pub fn writable(&mut self) -> Result<&mut [u8; N], HostCommandRetentionRefusal> {
        if self.stamp.is_some() {
            return Err(HostCommandRetentionRefusal::Occupied);
        }
        Ok(&mut self.bytes)
    }
    /// Original attribution deadline, never renewed by RAM custody. This is
    /// inspection only: expiry does not prove worker or resource retirement.
    pub fn expires_at_millis(&self) -> Option<u64> {
        self.stamp.as_ref().map(|stamp| stamp.expires_at)
    }
    pub fn payload(&self) -> Option<&[u8]> {
        self.stamp
            .as_ref()
            .map(|stamp| &self.bytes[..stamp.payload_bytes as usize])
    }
    pub fn payload_digest(&self) -> Option<&[u8; 32]> {
        self.stamp.as_ref().map(|stamp| &stamp.payload_digest)
    }
    /// Seal the same receive buffer in place. No second document is allocated or
    /// copied. Refusal leaves the existing bytes and occupied state unchanged.
    pub fn retain(
        &mut self,
        stamp: RetainedHostCommandStamp,
    ) -> Result<(), HostCommandRetentionRefusal> {
        use HostCommandRetentionRefusal as E;
        if self.stamp.is_some() {
            return Err(E::Occupied);
        }
        let length = usize::try_from(stamp.payload_bytes).map_err(|_| E::Bounds)?;
        let payload = self.bytes.get(..length).ok_or(E::Bounds)?;
        if Sha256::digest(payload).as_slice() != stamp.payload_digest {
            return Err(E::Payload);
        }
        self.bytes[length..].fill(0);
        self.stamp = Some(stamp);
        Ok(())
    }
    /// Storage release only. Root must separately establish any required native
    /// retirement; this does not stop workers or revoke their capabilities.
    pub fn retire(&mut self) {
        self.bytes.fill(0);
        self.stamp = None;
    }

    /// Reborrow the same attribution under independently retained current facts.
    /// No nonce or expiry is renewed, no signature/remote ACK is substituted, and
    /// no copied identity or this token itself grants local capability possession.
    pub fn reborrow<'c>(
        &self,
        member: &'c MembershipCredential,
        actual: &'c HostAdvertisement,
        issuer: &'c FederationPeer,
        contract: &'c str,
        now_millis: u64,
    ) -> Result<AttributedHostCommand<'c, '_>, HostCommandRetentionRefusal> {
        use HostCommandRetentionRefusal as E;
        let stamp = self.stamp.as_ref().ok_or(E::NotRetained)?;
        if now_millis < stamp.issued_at || now_millis >= stamp.expires_at {
            return Err(E::Clock);
        }
        if member.host_id != actual.host_id
            || member.boot_id != actual.boot_id
            || actual.protocol_version != PROTOCOL_VERSION
        {
            return Err(E::Context);
        }
        let challenge = HostCommandChallenge {
            body_id: member.body_id.as_str(),
            part_id: member.part_id.as_str(),
            credential_id: member.credential_id.as_str(),
            receiver_host: actual.host_id.as_str(),
            receiver_boot: actual.boot_id.as_str(),
            receiver_generation: actual.offer_generation.0,
            receiver_profile: actual.profile.as_str(),
            issuer_host: issuer.host_id.as_str(),
            issuer_boot: issuer.boot_id.as_str(),
            issuer_generation: issuer.offer_generation.0,
            issuer_key: issuer.verifying_key,
            payload_contract: contract,
            maximum_payload_bytes: stamp.maximum_bytes,
            nonce: stamp.nonce,
            issued_at_millis: stamp.issued_at,
            expires_at_millis: stamp.expires_at,
        };
        let payload = self.payload().ok_or(E::NotRetained)?;
        if host_command_attribution_transcript(&challenge, payload).map_err(|_| E::Context)?
            != stamp.transcript
        {
            return Err(E::Context);
        }
        Ok(AttributedHostCommand { challenge, payload })
    }
}

impl<const N: usize> Drop for HostCommandInput<N> {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}
