//! Complete fixed Source query schemas, reserved together before revision ingress.
//! Composition supplies representation only. The fixed Session driver chooses
//! fields from retained ancestry; the target owner still readmits every full query.
use crate::{
    parser_canonical_composition::{
        ParserCompositionLimits, ParserCompositionRefusal, PreparedParserCanonicalComposer,
    },
    parser_production_families::{port_descriptors, PreparedProductionParserFamilies},
    parser_session_execution::ParserSessionEntry,
    parser_session_target_registry::REQUIRED,
};
use alloc::vec::Vec;
use conduit_core::ValidatedCanonicalStructuredValue;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParserQueryPreparationLimits {
    pub(crate) maximum_frame_bytes: usize,
    pub(crate) maximum_preparation_requested_bytes: usize,
    pub(crate) maximum_retained_requested_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParserQueryPreparationReceipt {
    pub(crate) preparation_requested_bytes_bound: usize,
    pub(crate) retained_requested_bytes_bound: usize,
}
#[derive(Debug)]
pub(crate) enum ParserQueryRefusal {
    Entry,
    Descriptor,
    Pressure,
    Composition(ParserCompositionRefusal),
}
pub(crate) struct PreparedParserSessionQueries {
    composers: Vec<(ParserSessionEntry, PreparedParserCanonicalComposer)>,
    receipt: ParserQueryPreparationReceipt,
}
fn add(a: usize, b: usize) -> Result<usize, ParserQueryRefusal> {
    a.checked_add(b).ok_or(ParserQueryRefusal::Pressure)
}
impl PreparedParserSessionQueries {
    pub(crate) fn prepare(
        families: &PreparedProductionParserFamilies,
        limits: ParserQueryPreparationLimits,
    ) -> Result<Self, ParserQueryRefusal> {
        use ParserQueryRefusal as R;
        let slots = REQUIRED
            .len()
            .checked_mul(core::mem::size_of::<(
                ParserSessionEntry,
                PreparedParserCanonicalComposer,
            )>())
            .ok_or(R::Pressure)?;
        let mut preparation = slots;
        let mut retained = slots;
        // Complete readiness and cumulative requested allocation reservations
        // precede even the vector allocation, not merely each individual schema.
        for &entry in REQUIRED {
            let (descriptor, _) = port_descriptors(entry).ok_or(R::Entry)?;
            let family = families.for_entry(entry).map_err(|_| R::Descriptor)?;
            let reservation = PreparedParserCanonicalComposer::descriptor_reservation(
                &family.borrow(),
                descriptor,
                &[],
                limits.maximum_frame_bytes,
            )
            .map_err(R::Composition)?;
            preparation = add(preparation, reservation.preparation_requested_bytes_bound)?;
            retained = add(retained, reservation.retained_requested_bytes_bound)?;
        }
        if preparation > limits.maximum_preparation_requested_bytes
            || retained > limits.maximum_retained_requested_bytes
        {
            return Err(R::Pressure);
        }
        let mut composers = Vec::new();
        composers
            .try_reserve_exact(REQUIRED.len())
            .map_err(|_| R::Pressure)?;
        if composers.capacity() != REQUIRED.len() {
            return Err(R::Pressure);
        }
        for &entry in REQUIRED {
            let (descriptor, _) = port_descriptors(entry).ok_or(R::Entry)?;
            let family = families.for_entry(entry).map_err(|_| R::Descriptor)?;
            let composer = PreparedParserCanonicalComposer::prepare_descriptor_field(
                &family.borrow(),
                descriptor,
                &[],
                ParserCompositionLimits {
                    maximum_output_bytes: limits.maximum_frame_bytes,
                    maximum_preparation_requested_bytes: limits.maximum_preparation_requested_bytes,
                    maximum_retained_requested_bytes: limits.maximum_retained_requested_bytes,
                },
            )
            .map_err(R::Composition)?;
            composers.push((entry, composer));
        }
        Ok(Self {
            composers,
            receipt: ParserQueryPreparationReceipt {
                preparation_requested_bytes_bound: preparation,
                retained_requested_bytes_bound: retained,
            },
        })
    }
    pub(crate) fn receipt(&self) -> ParserQueryPreparationReceipt {
        self.receipt
    }
    pub(crate) fn record(
        &mut self,
        entry: ParserSessionEntry,
        fields: &[ValidatedCanonicalStructuredValue<'_>],
    ) -> Result<&[u8], ParserQueryRefusal> {
        self.composers
            .iter_mut()
            .find(|(selected, _)| *selected == entry)
            .ok_or(ParserQueryRefusal::Entry)?
            .1
            .record(fields)
            .map_err(ParserQueryRefusal::Composition)
    }
}
