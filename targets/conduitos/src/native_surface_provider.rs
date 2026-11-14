//! Cross-architecture possession of one exact native face surface.

use conduit_core::{
    BaseEnforcementClass, BaseLifecycle, BaseProviderEntry, BaseRegistry, BaseRegistryLimits,
    ResourceOffer,
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
        let show = exact_show_resource(&entry).ok_or(SurfaceProviderError::InvalidProvider)?;
        if entry.lifecycle != BaseLifecycle::Ready
            || entry.enforcement_class != BaseEnforcementClass::Cooperative
            || !entry.capabilities.is_empty()
            || show.pool_id.as_str().is_empty()
            || show.capacity_units != 1
            || show.compute.is_some()
            || show.content.is_some()
        {
            return Err(SurfaceProviderError::InvalidProvider);
        }
        for (index, resource) in entry.resources.iter().enumerate() {
            if resource.pool_id.as_str().is_empty()
                || resource.class_id.as_str().is_empty()
                || resource.capacity_units == 0
                || entry.resources[..index]
                    .iter()
                    .any(|prior| prior.pool_id == resource.pool_id)
            {
                return Err(SurfaceProviderError::InvalidProvider);
            }
        }
        let resource_count = u16::try_from(entry.resources.len())
            .map_err(|_| SurfaceProviderError::InvalidProvider)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 0,
            maximum_resources_per_base: resource_count,
            maximum_advertised_capabilities: 1,
            maximum_advertised_resources: resource_count,
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

/// Require one exact semantic Show pool without depending on physical-resource
/// ordering. The complete Base inventory remains retained and registry-validated.
pub(crate) fn exact_show_resource(entry: &BaseProviderEntry) -> Option<&ResourceOffer> {
    let mut matching = entry
        .resources
        .iter()
        .filter(|resource| resource.class_id.as_str() == conduit_face::SHOW_RESOURCE_CLASS);
    let selected = matching.next()?;
    matching.next().is_none().then_some(selected)
}

#[cfg(test)]
mod tests;
