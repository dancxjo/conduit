//! ConduitOS admission for the shared protected Line session.
//!
//! This adapter supplies only fresh ephemeral key material and finite session
//! storage. The caller separately owns the carrier, peer/session binding,
//! rendezvous PSK, admission policy, and any membership or effect authority.

use conduit_protected_line::{
    IMPLEMENTATION_ID, ProtectedCarrier, ProtectedFrameCarrier, ProtectedLineError,
    ProtectedSessionPolicy, Role, SessionBinding, establish_protected_session,
};

use crate::cryptographic_entropy::{
    CryptographicEntropyBase, CryptographicEntropySource, EntropyRefusal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConduitOsProtectedLineRefusal {
    Entropy(EntropyRefusal),
    Session(ProtectedLineError),
}

impl ConduitOsProtectedLineRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Entropy(error) => error.as_str(),
            Self::Session(_) => "protected-line-session-refused",
        }
    }
}

pub const fn protected_line_implementation_id() -> &'static str {
    IMPLEMENTATION_ID
}

/// Establish one admitted protected session over an already-admitted carrier.
///
/// Exactly one entropy request is consumed. Both the ephemeral key and this
/// function's PSK copy are volatile-erased before return on success or refusal.
pub fn establish_conduitos_protected_line<C, S, const REQUESTS: u32>(
    mut carrier: C,
    role: Role,
    binding: &SessionBinding,
    policy: ProtectedSessionPolicy,
    mut preshared_key: [u8; 32],
    entropy: &mut CryptographicEntropyBase<S, REQUESTS>,
) -> Result<ProtectedCarrier<C>, ConduitOsProtectedLineRefusal>
where
    C: ProtectedFrameCarrier,
    S: CryptographicEntropySource,
{
    let mut ephemeral_private_key = [0_u8; 32];
    if let Err(error) = entropy.fill(&mut ephemeral_private_key) {
        erase(&mut preshared_key);
        erase(&mut ephemeral_private_key);
        return Err(ConduitOsProtectedLineRefusal::Entropy(error));
    }
    let session = establish_protected_session(
        &mut carrier,
        role,
        binding,
        policy,
        preshared_key,
        ephemeral_private_key,
    );
    erase(&mut preshared_key);
    erase(&mut ephemeral_private_key);
    let session = session.map_err(ConduitOsProtectedLineRefusal::Session)?;
    ProtectedCarrier::new(carrier, session, policy).map_err(ConduitOsProtectedLineRefusal::Session)
}

fn erase(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: every byte is exclusively borrowed and valid for a volatile write.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

#[cfg(test)]
mod tests;
