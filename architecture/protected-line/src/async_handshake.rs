//! Async carrier IO for the same protected-session handshake, never admission.
use super::CarrierFailure;
use core::future::Future;

/// Bounded frame IO with no runtime or Send requirement. Implementations enforce
/// each supplied receive deadline and their local send deadline. Cancellation
/// must stop using borrowed buffers when the future returns or is dropped.
/// Original carrier possession and session-slot admission remain caller-owned.
pub trait AsyncProtectedFrameCarrier {
    fn send_frame(&mut self, frame: &[u8]) -> impl Future<Output = Result<(), CarrierFailure>>;
    fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout_millis: u32,
    ) -> impl Future<Output = Result<usize, CarrierFailure>>;
}

/// Establish the existing Noise profile with async IO and fixed 64-byte scratch.
/// The caller supplies the original admitted carrier, PSK and fresh ephemeral
/// private key; it owns the whole-attempt deadline and must retire its original
/// admission slot on failure, cancellation/drop, and eventual session retirement.
/// No slot is acquired or retired here. Success authenticates the bound transport
/// endpoints, not Body membership, Fore possession or execution authority.
#[cfg(feature = "noise-session")]
pub async fn establish_protected_session_async<C: AsyncProtectedFrameCarrier>(
    carrier: &mut C,
    role: crate::Role,
    binding: &crate::SessionBinding,
    policy: super::ProtectedSessionPolicy,
    preshared_key: [u8; 32],
    ephemeral_private_key: [u8; 32],
) -> Result<crate::ProtectedSession, crate::ProtectedLineError> {
    use crate::{ProtectedHandshake, ProtectedLineError};
    let policy = policy.validate()?;
    let mut handshake = ProtectedHandshake::new(
        role,
        binding,
        policy.traffic,
        preshared_key,
        ephemeral_private_key,
    )?;
    let mut frame = [0_u8; 64];
    for _ in 0..policy.handshake_work_units {
        if handshake.next_message_bytes()? > frame.len() {
            return Err(ProtectedLineError::HandshakeFrameLength);
        }
        if handshake.is_write_turn() {
            let length = handshake.next_message_bytes()?;
            handshake.write_message(&mut frame[..length])?;
            carrier
                .send_frame(&frame[..length])
                .await
                .map_err(|failure| failure.protected(true))?;
        } else {
            let length = carrier
                .receive_frame(&mut frame, policy.handshake_timeout_millis)
                .await
                .map_err(|failure| failure.protected(true))?;
            if length > frame.len() {
                return Err(ProtectedLineError::Pressure);
            }
            handshake.read_message(&frame[..length])?;
        }
    }
    handshake.finish()
}
