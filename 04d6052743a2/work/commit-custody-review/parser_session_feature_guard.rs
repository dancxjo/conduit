//! Exact Native feature-bound wrapper construction; representation only.
//! The numeric owner supplies an opaque actual Source execution and retains the
//! whole wrapper frame. Every original guard is freshly admitted separately.
use crate::parser_canonical_composition::{
    ParserCompositionLimits, ParserCompositionRefusal, PreparedParserCanonicalComposer as Composer,
};
use conduit_plot::rust_binding::{NativeFamilyTypeDescriptor, PreparedNativeFamily};
pub(crate) struct PreparedParserFeatureGuard {
    descriptor: &'static NativeFamilyTypeDescriptor,
    composer: Composer,
}
impl PreparedParserFeatureGuard {
    pub(crate) fn prepare(
        family: &PreparedNativeFamily,
        raw: &'static NativeFamilyTypeDescriptor,
        guard: &'static NativeFamilyTypeDescriptor,
        limits: ParserCompositionLimits,
    ) -> Result<Self, ParserCompositionRefusal> {
        // Complete descriptor identity and nested raw Type must match before the
        // descriptor-backed composer performs any allocation.
        if !family.contains_descriptor(raw)
            || !family.contains_descriptor(guard)
            || crate::parser_canonical_schema::select_field(guard.type_bytes, &["raw"]).ok()
                != Some(raw.type_bytes)
        {
            return Err(ParserCompositionRefusal::Descriptor);
        }
        let composer = Composer::prepare_descriptor_field(family, guard, &[], limits)?;
        Ok(Self {
            descriptor: guard,
            composer,
        })
    }
    pub(crate) fn descriptor(&self) -> &'static NativeFamilyTypeDescriptor {
        self.descriptor
    }
    pub(crate) fn compose(&mut self, raw: &[u8]) -> Result<&[u8], ParserCompositionRefusal> {
        let raw = conduit_core::validate_canonical_structured_value(raw)
            .map_err(|_| ParserCompositionRefusal::Descriptor)?;
        self.composer.record(&[raw])
    }
}
