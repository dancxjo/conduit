//! Borrowed and durable owned facades over one admission state machine.
use super::*;
use crate::NativeLawWitness;

macro_rules! common_methods {
    ($($field:ident).+) => {
        pub fn capacity_report(&self) -> PreparedNativeCapacityReport {
            self.$($field).+.capacity_report()
        }
        pub fn scope_count(&self) -> usize {
            self.$($field).+.scope_count()
        }
        pub fn scope_native_type(&self, scope: usize) -> Option<&CheckedNativeType> {
            self.$($field).+.scope_native_type(scope)
        }
        pub fn invariant_output(&self, scope: usize, invariant: usize) -> Option<&[u8]> {
            self.$($field).+.invariant_output(scope, invariant)
        }
        pub fn latest_occurrence(&self) -> Option<NativeOccurrence<'_>> {
            self.$($field).+.latest_occurrence()
        }
        pub fn latest_contract_occurrence(&self) -> Option<NativeContractOccurrence<'_>> {
            self.$($field).+.latest_contract_occurrence()
        }
        pub fn actual_contract_occurrences(&self) -> usize {
            self.$($field).+.actual_contract_occurrences()
        }
        pub fn actual_occurrences(&self) -> usize {
            self.$($field).+.actual_occurrences()
        }
        pub fn actual_occurrence_limit(&self) -> usize {
            self.$($field).+.actual_occurrence_limit()
        }
        pub fn retained_bytes(&self) -> &[u8] {
            self.$($field).+.retained_bytes()
        }
        pub fn failure(&self) -> Option<&PreparedNativeRefusal> {
            self.$($field).+.failure()
        }
        pub fn begin(&mut self, bytes: &[u8]) -> Result<(), PreparedNativeRefusal> {
            self.$($field).+.begin(bytes)
        }
        pub fn cancel(&mut self) {
            self.$($field).+.cancel()
        }
        pub fn step(&mut self) -> Result<PreparedNativeProgress, PreparedNativeRefusal> {
            self.$($field).+.step()
        }
        pub fn retire_admitted(&mut self) -> Result<(), PreparedNativeRefusal> {
            self.$($field).+.retire_admitted()
        }
    };
}

pub(super) use common_methods;

pub struct PreparedNativeAdmission<'d> {
    document: &'d CheckedSyntaxDocument,
    native: &'d CheckedNativeType,
    engine: AdmissionEngine<'d>,
}
impl<'d> PreparedNativeAdmission<'d> {
    pub fn new(
        document: &'d CheckedSyntaxDocument,
        identity: &KindId,
        bounds: PreparedNativeBounds,
    ) -> Result<Self, PreparedNativeRefusal> {
        Self::new_with_occurrence_limit(
            document,
            identity,
            bounds,
            MAXIMUM_PREPARED_NATIVE_OCCURRENCES,
        )
    }
    pub fn new_with_occurrence_limit(
        document: &'d CheckedSyntaxDocument,
        identity: &KindId,
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<Self, PreparedNativeRefusal> {
        let engine = AdmissionEngine::prepare(
            metadata::Metadata::Borrowed(document),
            identity,
            bounds,
            occurrence_limit,
        )?;
        let native = traversal::lookup(document, identity)?;
        Ok(Self {
            document,
            native,
            engine,
        })
    }
    pub fn native_type(&self) -> &CheckedNativeType {
        self.native
    }
    pub fn source_document(&self) -> &CheckedSyntaxDocument {
        self.document
    }
    common_methods!(engine);
    pub fn admitted(&self) -> Result<AdmittedNativeCanonical<'_>, PreparedNativeRefusal> {
        self.engine.check_admitted()?;
        Ok(AdmittedNativeCanonical {
            document: self.document,
            native: self.native,
            bytes: &self.engine.input,
        })
    }
}

/// Durable admission whose law authority is an opaque compiler-verified closure.
pub struct OwnedPreparedNativeAdmission {
    witness: NativeLawWitness,
    engine: AdmissionEngine<'static>,
}
impl OwnedPreparedNativeAdmission {
    pub fn new(
        witness: &NativeLawWitness,
        bounds: PreparedNativeBounds,
    ) -> Result<Self, PreparedNativeRefusal> {
        Self::new_with_occurrence_limit(witness, bounds, MAXIMUM_PREPARED_NATIVE_OCCURRENCES)
    }
    pub fn new_with_occurrence_limit(
        witness: &NativeLawWitness,
        bounds: PreparedNativeBounds,
        occurrence_limit: usize,
    ) -> Result<Self, PreparedNativeRefusal> {
        let types = witness
            .admission_metadata()
            .map_err(|refusal| match refusal {
                crate::NativeLawCustodyRefusal::Capacity => PreparedNativeRefusal::Bounds,
                crate::NativeLawCustodyRefusal::Laws | crate::NativeLawCustodyRefusal::Type => {
                    PreparedNativeRefusal::ConflictingNativeMetadata
                }
                _ => PreparedNativeRefusal::MissingNativeMetadata,
            })?;
        let engine = AdmissionEngine::prepare(
            metadata::Metadata::Owned(types),
            &witness.checked_type().identity,
            bounds,
            occurrence_limit,
        )?;
        Ok(Self {
            witness: witness.clone(),
            engine,
        })
    }
    pub fn native_type(&self) -> &CheckedNativeType {
        self.witness.checked_type()
    }
    common_methods!(engine);
    pub fn admitted(&self) -> Result<AdmittedOwnedNativeCanonical<'_>, PreparedNativeRefusal> {
        self.engine.check_admitted()?;
        Ok(AdmittedOwnedNativeCanonical {
            witness: &self.witness,
            native: self.witness.checked_type(),
            bytes: &self.engine.input,
        })
    }
}
/// Borrows the exact admitted bytes and their retained opaque Source custody.
/// It cannot construct a generated Rust value or impersonate a checked document.
pub struct AdmittedOwnedNativeCanonical<'a> {
    witness: &'a NativeLawWitness,
    native: &'a CheckedNativeType,
    bytes: &'a [u8],
}
impl AdmittedOwnedNativeCanonical<'_> {
    pub fn source_witness(&self) -> &NativeLawWitness {
        self.witness
    }
    pub fn native_type(&self) -> &CheckedNativeType {
        self.native
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        self.bytes
    }
}
