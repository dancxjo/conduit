//! Explicitly owned, bounded Native admission for complete stable lexical facts.
use crate::LanguageParserWindow8StableLexicalFact;
use conduit_plot::rust_binding::{
    NativeBindingRefusal, PreparedNativeFamily, PreparedNativeFamilyLimits,
    PreparedNativeFamilyRefusal, PreparedNativeFamilyStorageReceipt, PreparedNativeRustBinding,
};

/// Prepare before starting the parser Flow. The owner retains all metadata for
/// the generated complete child closure, and each decode checks every child law.
/// Returned facts and their caller-owned queues are outside this owner's receipt.
pub struct PreparedStableLexicalFactAdmission {
    family: PreparedNativeFamily,
    maximum_input_bytes: usize,
}

impl PreparedStableLexicalFactAdmission {
    pub fn prepare(
        limits: PreparedNativeFamilyLimits,
    ) -> Result<Self, PreparedNativeFamilyRefusal> {
        let family = PreparedNativeFamily::prepare(
            &[LanguageParserWindow8StableLexicalFact::PREPARED_DESCRIPTOR],
            limits,
        )?;
        Ok(Self {
            family,
            maximum_input_bytes: limits.maximum_input_bytes,
        })
    }

    pub fn storage_receipt(&self) -> PreparedNativeFamilyStorageReceipt {
        self.family.storage_receipt()
    }

    pub fn convert_structured(
        &mut self,
        value: &conduit_core::StructuredInfoValue,
    ) -> Result<LanguageParserWindow8StableLexicalFact, NativeBindingRefusal> {
        let bytes = value
            .canonical_bytes_with_limit(self.maximum_input_bytes)
            .map_err(NativeBindingRefusal::InvalidValue)?;
        self.decode(&bytes)
    }

    pub fn decode(
        &mut self,
        canonical: &[u8],
    ) -> Result<LanguageParserWindow8StableLexicalFact, NativeBindingRefusal> {
        self.family.decode(canonical)
    }
}
