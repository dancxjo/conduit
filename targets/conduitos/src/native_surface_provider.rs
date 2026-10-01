//! Cross-architecture possession of one exact native presentation surface.

use conduit_core::BaseProviderEntry;

/// Exact discovered surface provider plus issuer-private material. The key is
/// never descriptive Base truth and is deliberately not serializable or
/// exposed through inspection.
#[derive(Clone)]
pub struct NativeSurfaceProvider {
    pub entry: BaseProviderEntry,
    pub(crate) issuer_key: [u8; 32],
}
