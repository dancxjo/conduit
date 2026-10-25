//! Cross-architecture possession of one exact native face surface.

use conduit_core::{
    BaseEnforcementClass, BaseLifecycle, BaseProviderEntry, BaseRegistry, BaseRegistryLimits,
};

use crate::display::{DisplayError, PixelTarget};

/// Trusted cooperative Host-driver contract for an already initialized output.
///
/// Implementors must check the live output they own, including its revocation
/// state. A serialized descriptor or advertisement is insufficient. This is a
/// cooperative Rust integration boundary, not isolation from hostile Rust code.
/// The caller must retain the output and revoke its provider when the driver is
/// lost; construction does not admit a Plan, issue Play, or acknowledge a Show.
pub trait InitializedNativeOutput: PixelTarget {
    fn require_live(&self) -> Result<(), DisplayError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceProviderError {
    Display(DisplayError),
    InvalidProvider,
    InvalidIssuerKey,
}

/// Exact discovered surface provider plus issuer-private material. The key is
/// never descriptive Base truth and is deliberately not serializable or
/// exposed through inspection.
#[derive(Clone)]
pub struct NativeSurfaceProvider {
    pub entry: BaseProviderEntry,
    #[allow(dead_code)] // Reserved for the resource-bound renderer Host Call.
    pub(crate) issuer_key: [u8; 32],
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    pub(crate) display_owner:
        Option<crate::protected_compositor::display_owner::SharedDisplayOwner>,
}

impl NativeSurfaceProvider {
    /// Bridge an actual initialized cooperative driver into native Mask
    /// possession. The key is supplied by the trusted Host issuer and retained
    /// privately. Provider metadata remains descriptive, not independent proof
    /// of possession. No pixels are written or committed here.
    pub fn from_initialized_output(
        entry: BaseProviderEntry,
        issuer_key: [u8; 32],
        output: &mut impl InitializedNativeOutput,
    ) -> Result<Self, SurfaceProviderError> {
        output
            .require_live()
            .map_err(SurfaceProviderError::Display)?;
        output
            .format()
            .validate()
            .map_err(SurfaceProviderError::Display)?;
        if issuer_key == [0; 32] {
            return Err(SurfaceProviderError::InvalidIssuerKey);
        }
        if entry.lifecycle != BaseLifecycle::Ready
            || entry.enforcement_class != BaseEnforcementClass::Cooperative
            || !entry.capabilities.is_empty()
            || entry.resources.len() != 1
            || entry.resources[0].class_id.as_str() != conduit_face::SHOW_RESOURCE_CLASS
            || entry.resources[0].pool_id.as_str().is_empty()
            || entry.resources[0].capacity_units != 1
            || entry.resources[0].compute.is_some()
            || entry.resources[0].content.is_some()
        {
            return Err(SurfaceProviderError::InvalidProvider);
        }
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 0,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: 1,
            maximum_advertised_resources: 1,
        })
        .map_err(|_| SurfaceProviderError::InvalidProvider)?;
        registry
            .register(entry.clone())
            .map_err(|_| SurfaceProviderError::InvalidProvider)?;
        Ok(Self {
            entry,
            issuer_key,
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            display_owner: None,
        })
    }
}

#[cfg(test)]
mod tests;
