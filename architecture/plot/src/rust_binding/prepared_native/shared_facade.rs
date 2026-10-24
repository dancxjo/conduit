//! Native proof from a sealed registry, distinct from a single Source witness.
use super::*;
use facades::common_methods;

pub struct PreparedRegisteredNativeAdmission {
    pub(super) guard: registry::PreparedRegisteredValue,
    pub(super) native_index: usize,
}
impl PreparedRegisteredNativeAdmission {
    pub fn native_type(&self) -> &CheckedNativeType {
        &self.guard.registry.data.types[self.native_index]
    }
    common_methods!(guard.engine);
    pub fn admitted(&self) -> Result<AdmittedRegisteredNativeCanonical<'_>, PreparedNativeRefusal> {
        let value = self.guard.admitted()?;
        let native = self.native_type();
        if value.value_type != &native.value_type {
            return Err(PreparedNativeRefusal::ConflictingNativeMetadata);
        }
        Ok(AdmittedRegisteredNativeCanonical {
            native,
            bytes: value.bytes,
            registry: value.registry,
        })
    }
}
pub struct AdmittedRegisteredNativeCanonical<'a> {
    native: &'a CheckedNativeType,
    bytes: &'a [u8],
    registry: &'a NativeLawRegistry,
}
impl AdmittedRegisteredNativeCanonical<'_> {
    pub fn native_type(&self) -> &CheckedNativeType {
        self.native
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.bytes
    }
    pub fn source_registry(&self) -> &NativeLawRegistry {
        self.registry
    }
}
