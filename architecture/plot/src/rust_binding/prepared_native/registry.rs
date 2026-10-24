//! One sealed immutable Native table shared by independently prepared guards.
use super::*;
use crate::NativeLawWitness;
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Shared;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as Shared;

pub(super) type SharedRegistry = Shared<RegistryData>;
pub(super) struct RegistryData {
    pub(super) types: Vec<CheckedNativeType>,
    sources: Vec<NativeLawWitness>,
}
/// The only constructor replays opaque compiler-verified Source closures.
/// Public inspection types and catalogues cannot create this authority.
/// Registered guards check invariant arithmetic without changing predicate bytes
/// or their proven hints. This registry is not expression-envelope authority.
#[derive(Clone)]
pub struct NativeLawRegistry {
    pub(super) data: SharedRegistry,
}
impl NativeLawRegistry {
    pub fn new(witnesses: &[NativeLawWitness]) -> Result<Self, PreparedNativeRefusal> {
        let types = NativeLawWitness::admission_metadata_for(witnesses).map_err(refusal)?;
        // The factory admitted the aggregate expanded Source closure and all
        // metadata before this retained provenance clone. Source custody and
        // the shared table each have their separate4MiB preparation profile.
        Ok(Self {
            data: Shared::new(RegistryData {
                types,
                sources: witnesses.to_vec(),
            }),
        })
    }
    pub fn source_witnesses(&self) -> &[NativeLawWitness] {
        &self.data.sources
    }
    pub fn prepare_native(
        &self,
        identity: &KindId,
        bounds: PreparedNativeBounds,
    ) -> Result<PreparedRegisteredNativeAdmission, PreparedNativeRefusal> {
        self.prepare_native_with_occurrence_limit(
            identity,
            bounds,
            MAXIMUM_PREPARED_NATIVE_OCCURRENCES,
        )
    }
    pub fn prepare_native_with_occurrence_limit(
        &self,
        identity: &KindId,
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<PreparedRegisteredNativeAdmission, PreparedNativeRefusal> {
        AdmissionEngine::validate_profile(bounds, occurrence_limit)?;
        let metadata = metadata::Metadata::Shared(self.data.clone());
        let index = metadata.lookup(identity)?;
        let guard =
            self.prepare_type(&self.data.types[index].value_type, bounds, occurrence_limit)?;
        Ok(PreparedRegisteredNativeAdmission {
            guard,
            native_index: index,
        })
    }
    // Private future-envelope seam: structural roots have constructor-derived
    // identities and complete nested owner coverage, never a fabricated Native.
    pub(crate) fn prepare_type(
        &self,
        value_type: &StructuredInfoType,
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<PreparedRegisteredValue, PreparedNativeRefusal> {
        AdmissionEngine::validate_profile(bounds, occurrence_limit)?;
        let metadata = metadata::Metadata::Shared(self.data.clone());
        let root = structural_root::descriptor(&metadata, value_type)?;
        let engine = AdmissionEngine::prepare_root(metadata, root, bounds, occurrence_limit)?;
        Ok(PreparedRegisteredValue {
            registry: self.clone(),
            engine,
        })
    }
}
pub(crate) struct PreparedRegisteredValue {
    pub(super) registry: NativeLawRegistry,
    pub(super) engine: AdmissionEngine<'static>,
}
pub(super) struct AdmittedRegisteredValue<'a> {
    pub(super) value_type: &'a StructuredInfoType,
    pub(super) bytes: &'a [u8],
    pub(super) registry: &'a NativeLawRegistry,
}
impl PreparedRegisteredValue {
    pub(crate) fn capacity_report(&self) -> PreparedNativeCapacityReport {
        self.engine.capacity_report()
    }
    pub(crate) fn check_value(&mut self, bytes: &[u8]) -> Result<(), PreparedNativeRefusal> {
        self.engine.begin(bytes)?;
        while self.engine.step()? == PreparedNativeProgress::Pending {}
        self.engine.check_admitted()?;
        self.engine.retire_admitted()
    }
    pub(super) fn admitted(&self) -> Result<AdmittedRegisteredValue<'_>, PreparedNativeRefusal> {
        self.engine.check_admitted()?;
        Ok(AdmittedRegisteredValue {
            value_type: self.engine.root.value_type(&self.engine.metadata),
            bytes: &self.engine.input,
            registry: &self.registry,
        })
    }
}
fn refusal(cause: crate::NativeLawCustodyRefusal) -> PreparedNativeRefusal {
    match cause {
        crate::NativeLawCustodyRefusal::Capacity => PreparedNativeRefusal::Bounds,
        crate::NativeLawCustodyRefusal::Laws | crate::NativeLawCustodyRefusal::Type => {
            PreparedNativeRefusal::ConflictingNativeMetadata
        }
        _ => PreparedNativeRefusal::MissingNativeMetadata,
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
