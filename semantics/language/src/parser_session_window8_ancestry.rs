//! Eight fixed representation buffers for actual retained Source walk outputs.
//! The caller retains all 72 original walk executions. This bank never grants
//! state admission: the containing StateProof must receive full Native admission.
use crate::{
    LanguageParserWindow8Ancestry, LanguageParserWindow8StateProof,
    parser_canonical_composition::{ParserCompositionLimits, PreparedParserCanonicalComposer},
    parser_session_window8_collection::PreparedWindow8CanonicalCollection,
    parser_session_window8_values::{field, path, unsigned, view, View},
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
#[derive(Clone, Copy, Debug)]
pub(crate) struct AncestryRefusal;
pub(crate) struct PreparedWindow8Ancestry {
    ancestry: Vec<PreparedParserCanonicalComposer>,
    collection: PreparedWindow8CanonicalCollection,
    preparation_bytes: usize,
    retained_bytes: usize,
}
impl PreparedWindow8Ancestry {
    pub(crate) fn prepare(
        family: &PreparedNativeFamily,
        maximum_frame_bytes: usize,
        maximum_preparation_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, AncestryRefusal> {
        let descriptor = LanguageParserWindow8Ancestry::PREPARED_DESCRIPTOR;
        let proof = LanguageParserWindow8StateProof::PREPARED_DESCRIPTOR;
        if !family.contains_descriptor(descriptor) || !family.contains_descriptor(proof) {
            return Err(AncestryRefusal);
        }
        let reservation = PreparedParserCanonicalComposer::descriptor_reservation(
            family, descriptor, &[], maximum_frame_bytes,
        ).map_err(|_| AncestryRefusal)?;
        let slots = 8usize.checked_mul(core::mem::size_of::<PreparedParserCanonicalComposer>())
            .ok_or(AncestryRefusal)?;
        let preparation = reservation.preparation_requested_bytes_bound.checked_mul(8)
            .and_then(|n| n.checked_add(slots))
            .and_then(|n| n.checked_add(maximum_frame_bytes)).ok_or(AncestryRefusal)?;
        let retained = reservation.retained_requested_bytes_bound.checked_mul(8)
            .and_then(|n| n.checked_add(slots))
            .and_then(|n| n.checked_add(maximum_frame_bytes)).ok_or(AncestryRefusal)?;
        if preparation > maximum_preparation_bytes || retained > maximum_retained_bytes {
            return Err(AncestryRefusal);
        }
        // Complete aggregate reservation precedes this first allocation.
        let mut ancestry = Vec::new();
        ancestry.try_reserve_exact(8).map_err(|_| AncestryRefusal)?;
        if ancestry.capacity() != 8 { return Err(AncestryRefusal); }
        for _ in 0..8 {
            ancestry.push(PreparedParserCanonicalComposer::prepare_descriptor_field(
                family, descriptor, &[], ParserCompositionLimits {
                    maximum_output_bytes: maximum_frame_bytes,
                    maximum_preparation_requested_bytes: reservation.preparation_requested_bytes_bound,
                    maximum_retained_requested_bytes: reservation.retained_requested_bytes_bound,
                },
            ).map_err(|_| AncestryRefusal)?);
        }
        let collection = PreparedWindow8CanonicalCollection::prepare_descriptor(
            family, proof, &["ancestry"], maximum_frame_bytes,
            maximum_frame_bytes, maximum_frame_bytes,
        ).map_err(|_| AncestryRefusal)?;
        Ok(Self { ancestry, collection, preparation_bytes: preparation, retained_bytes: retained })
    }
    pub(crate) fn reservation(&self) -> (usize, usize) {
        (self.preparation_bytes, self.retained_bytes)
    }
    pub(crate) fn compose(&mut self, walks: [View<'_>; 8]) -> Result<&[u8], AncestryRefusal> {
        let heads = path(walks[0], &["query", "heads"]).map_err(|_| AncestryRefusal)?;
        for (index, (walk, composer)) in walks.iter().zip(&mut self.ancestry).enumerate() {
            let actual_heads = path(*walk, &["query", "heads"]).map_err(|_| AncestryRefusal)?;
            let start = path(*walk, &["query", "start"]).map_err(|_| AncestryRefusal)?;
            if heads.type_bytes() != actual_heads.type_bytes()
                || heads.value_node() != actual_heads.value_node()
                || unsigned(start).map_err(|_| AncestryRefusal)? != index as u64
            { return Err(AncestryRefusal); }
            let path = field(*walk, "path").map_err(|_| AncestryRefusal)?;
            // Exact canonical schema order is heads, path, start; the prepared
            // composer checks every complete child Type before writing.
            composer.record(&[heads, path, start]).map_err(|_| AncestryRefusal)?;
        }
        let first = view(self.ancestry[0].encoded()).map_err(|_| AncestryRefusal)?;
        let mut values = [first; 8];
        for (value, composer) in values.iter_mut().zip(&self.ancestry) {
            *value = view(composer.encoded()).map_err(|_| AncestryRefusal)?;
        }
        self.collection.compose(values.into_iter()).map_err(|_| AncestryRefusal)
    }
}
