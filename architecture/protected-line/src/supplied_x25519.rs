//! Noise trait adapter for caller-supplied X25519 keys.

use noise_protocol::{U8Array, DH};
use noise_rust_crypto::sensitive::Sensitive;
use x25519_dalek::{PublicKey, StaticSecret};

/// Delegates every cryptographic operation to `x25519-dalek`. This profile
/// deliberately refuses the trait's internal key-generation entrance because
/// each target must supply fresh CSPRNG material before constructing Noise.
pub(crate) enum SuppliedX25519 {}

impl DH for SuppliedX25519 {
    type Key = Sensitive<[u8; 32]>;
    type Pubkey = [u8; 32];
    type Output = Sensitive<[u8; 32]>;

    fn name() -> &'static str {
        "25519"
    }

    fn genkey() -> Self::Key {
        panic!("protected Line requires caller-supplied ephemeral entropy")
    }

    fn pubkey(key: &Self::Key) -> Self::Pubkey {
        *PublicKey::from(&StaticSecret::from(**key)).as_bytes()
    }

    fn dh(key: &Self::Key, peer: &Self::Pubkey) -> Result<Self::Output, ()> {
        let secret = StaticSecret::from(**key);
        let public = PublicKey::from(*peer);
        Ok(Sensitive::from_slice(
            secret.diffie_hellman(&public).as_bytes(),
        ))
    }
}
