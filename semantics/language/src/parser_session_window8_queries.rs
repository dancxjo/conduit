//! Complete fixed Window8 Source query schemas, reserved together before ingress.
//! Composition supplies representation only. The fixed Session driver chooses
//! fields from retained ancestry; the target owner still readmits every full query.
use crate::{
    parser_canonical_composition::{
        ParserCompositionLimits, ParserCompositionRefusal, PreparedParserCanonicalComposer,
    },
    parser_session_window8_ports::{PORTS, family_for},
};
use alloc::{rc::Rc, vec::Vec};
use conduit_plot::rust_binding::PreparedNativeFamily;
use core::cell::RefCell;
type Family = Rc<RefCell<PreparedNativeFamily>>;
use conduit_core::ValidatedCanonicalStructuredValue;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8QueryPreparationLimits {
    pub(crate) maximum_frame_bytes: usize,
    pub(crate) maximum_preparation_requested_bytes: usize,
    pub(crate) maximum_retained_requested_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8QueryPreparationReceipt {
    pub(crate) preparation_requested_bytes_bound: usize,
    pub(crate) retained_requested_bytes_bound: usize,
}
#[derive(Debug)]
pub(crate) enum Window8QueryRefusal {
    Entry,
    Descriptor,
    Pressure,
    Composition(ParserCompositionRefusal),
}
pub(crate) struct PreparedWindow8Queries {
    composers: Vec<PreparedParserCanonicalComposer>,
    receipt: Window8QueryPreparationReceipt,
}
fn add(a: usize, b: usize) -> Result<usize, Window8QueryRefusal> {
    a.checked_add(b).ok_or(Window8QueryRefusal::Pressure)
}
impl PreparedWindow8Queries {
    pub(crate) fn prepare(
        families: &[Family],
        limits: Window8QueryPreparationLimits,
    ) -> Result<Self, Window8QueryRefusal> {
        use Window8QueryRefusal as R;
        let slots = PORTS
            .len()
            .checked_mul(core::mem::size_of::<PreparedParserCanonicalComposer>())
            .ok_or(R::Pressure)?;
        let mut preparation = slots;
        let mut retained = slots;
        // Complete readiness and cumulative requested allocation reservations
        // precede even the vector allocation, not merely each individual schema.
        for port in PORTS {
            let descriptor = port.input;
            let family_index = family_for(families, descriptor).map_err(|_| R::Descriptor)?;
            let family = &families[family_index];
            let reservation = PreparedParserCanonicalComposer::descriptor_reservation(
                &*family.try_borrow().map_err(|_| R::Descriptor)?,
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
            .try_reserve_exact(PORTS.len())
            .map_err(|_| R::Pressure)?;
        if composers.capacity() != PORTS.len() {
            return Err(R::Pressure);
        }
        for port in PORTS {
            let descriptor = port.input;
            let family_index = family_for(families, descriptor).map_err(|_| R::Descriptor)?;
            let family = &families[family_index];
            let composer = PreparedParserCanonicalComposer::prepare_descriptor_field(
                &*family.try_borrow().map_err(|_| R::Descriptor)?,
                descriptor,
                &[],
                ParserCompositionLimits {
                    maximum_output_bytes: limits.maximum_frame_bytes,
                    maximum_preparation_requested_bytes: limits.maximum_preparation_requested_bytes,
                    maximum_retained_requested_bytes: limits.maximum_retained_requested_bytes,
                },
            )
            .map_err(R::Composition)?;
            composers.push(composer);
        }
        Ok(Self {
            composers,
            receipt: Window8QueryPreparationReceipt {
                preparation_requested_bytes_bound: preparation,
                retained_requested_bytes_bound: retained,
            },
        })
    }
    pub(crate) fn receipt(&self) -> Window8QueryPreparationReceipt {
        self.receipt
    }
    pub(crate) fn record(
        &mut self,
        index: usize,
        fields: &[ValidatedCanonicalStructuredValue<'_>],
    ) -> Result<&[u8], Window8QueryRefusal> {
        self.composers
            .get_mut(index)
            .ok_or(Window8QueryRefusal::Entry)?
            .record(fields)
            .map_err(Window8QueryRefusal::Composition)
    }
    pub(crate) fn record_named(
        &mut self,
        name: &str,
        fields: &[ValidatedCanonicalStructuredValue<'_>],
    ) -> Result<&[u8], Window8QueryRefusal> {
        let index = crate::parser_session_window8_ports::port_index(name)
            .ok_or(Window8QueryRefusal::Entry)?;
        self.record(index, fields)
    }
    /// Match fields by the exact retained schema rather than relying on a host
    /// spelling order. This only assembles a representation; Source and Native
    /// admission still occur at the selected ingress.
    pub(crate) fn record_fields(
        &mut self,
        name: &str,
        supplied: &[(&str, ValidatedCanonicalStructuredValue<'_>)],
    ) -> Result<&[u8], Window8QueryRefusal> {
        use Window8QueryRefusal as R;
        let index = crate::parser_session_window8_ports::port_index(name).ok_or(R::Entry)?;
        let crate::parser_canonical_schema::Shape::Record(fields) =
            crate::parser_canonical_schema::shape(PORTS[index].input.type_bytes)
                .map_err(|_| R::Descriptor)?
        else { return Err(R::Descriptor); };
        if supplied.is_empty() || supplied.len() > 64 || fields.len() != supplied.len() {
            return Err(R::Descriptor);
        }
        let mut ordered = [supplied[0].1; 64];
        for (destination, field) in ordered.iter_mut().zip(fields) {
            let (expected, _) = field.map_err(|_| R::Descriptor)?;
            let mut matches = supplied.iter().filter(|(name, _)| *name == expected);
            *destination = matches.next().ok_or(R::Descriptor)?.1;
            if matches.next().is_some() { return Err(R::Descriptor); }
        }
        self.record(index, &ordered[..supplied.len()])
    }
    pub(crate) fn copy_record(
        &mut self,
        name: &str,
        original: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<&[u8], Window8QueryRefusal> {
        use Window8QueryRefusal as R;
        let index = crate::parser_session_window8_ports::port_index(name).ok_or(R::Entry)?;
        if original.type_bytes() != PORTS[index].input.type_bytes {
            return Err(R::Descriptor);
        }
        let crate::parser_canonical_schema::Shape::Record(fields) =
            crate::parser_canonical_schema::shape(original.type_bytes())
                .map_err(|_| R::Descriptor)?
        else { return Err(R::Descriptor); };
        let count = fields.len();
        if count == 0 || count > 64 { return Err(R::Descriptor); }
        let mut ordered = [original; 64];
        for (destination, field) in ordered.iter_mut().zip(fields) {
            let (name, _) = field.map_err(|_| R::Descriptor)?;
            *destination = original.record_field(name).map_err(|_| R::Descriptor)?
                .ok_or(R::Descriptor)?;
        }
        self.record(index, &ordered[..count])
    }
}
