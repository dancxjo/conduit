//! Separately bounded exact Native family ownership for production ports.
//! Model, canonical histories, Source evaluators and target buffers are separate.
//!
//! The checked 111-Type checkpoint measured 1,536,305,290 retained heap bytes
//! across these six owners, with a 1,538,758,058 preparation peak bound and a
//! 478,412,897 conversion ceiling for each live value in the largest family.
//! Immutable artifacts and the complete Source evaluators are additional.
//! These measurements establish no small-memory or target suitability claim;
//! construction always uses the caller's simultaneous declared reservations.
#[path = "parser_production_family_definitions.rs"]
mod definitions;
#[path = "parser_production_static_resources.rs"]
mod static_resources;
use crate::parser_session_execution::ParserSessionEntry;
use alloc::rc::Rc;
use conduit_plot::rust_binding::{
    PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeFamilyRefusal,
    PreparedNativeFamilyStorageReceipt, PreparedNativeRustBinding,
};
use core::cell::RefCell;
use definitions::ProductionFamily;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionFamilyLimits {
    pub family: PreparedNativeFamilyLimits,
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_static_artifact_bytes: usize,
    /// Whole construction ceiling, including immutable artifacts and every
    /// declared concurrently live allocation reservation below.
    pub maximum_combined_bytes: usize,
    /// Source evaluators, model, canonical frame pools, composers and target
    /// execution must be admitted independently, then included here.
    pub other_reserved_bytes: usize,
    pub concurrent_native_values: usize,
}
#[derive(Debug)]
pub(crate) enum ProductionFamilyRefusal {
    Pressure,
    Native(PreparedNativeFamilyRefusal),
    MissingDescriptor,
    PrimitiveOutput,
}
/// Conversion envelopes must additionally be reserved per concurrent decode.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionFamilyReceipt {
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
    pub families: [PreparedNativeFamilyStorageReceipt; 6],
    pub static_resources: static_resources::ParserStaticResourceReceipt,
}
pub(crate) struct PreparedProductionParserFamilies {
    families: [Rc<RefCell<PreparedNativeFamily>>; 6],
    receipt: ProductionFamilyReceipt,
}
impl PreparedProductionParserFamilies {
    pub(crate) fn prepare(limits: ProductionFamilyLimits) -> Result<Self, ProductionFamilyRefusal> {
        use ProductionFamilyRefusal as R;
        let static_resources = static_resources::descriptor_resources().ok_or(R::Pressure)?;
        let static_bytes = static_resources
            .canonical_bytes_bound
            .checked_add(static_resources.descriptor_storage_bytes_bound)
            .and_then(|n| n.checked_add(static_resources.source_canonical_bytes_bound))
            .ok_or(R::Pressure)?;
        if static_bytes > limits.maximum_static_artifact_bytes {
            return Err(R::Pressure);
        }
        // Rc counters and conservative alignment padding are charged separately
        // from the SDK's owned metadata allocations.
        let owner_bytes = core::mem::size_of::<RefCell<PreparedNativeFamily>>()
            .checked_add(2 * core::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(4 * core::mem::align_of::<RefCell<PreparedNativeFamily>>()))
            .ok_or(R::Pressure)?;
        // Refuse before constructing the first owner. Six separately admitted
        // families do not establish a simultaneous whole-session envelope.
        let all_retained = limits
            .family
            .maximum_retained_bytes
            .checked_add(owner_bytes)
            .and_then(|n| n.checked_mul(6))
            .ok_or(R::Pressure)?;
        let construction = limits
            .family
            .maximum_retained_bytes
            .checked_add(owner_bytes)
            .and_then(|n| n.checked_mul(5))
            .and_then(|n| n.checked_add(limits.family.maximum_preparation_peak_bytes))
            .and_then(|n| n.checked_add(owner_bytes))
            .ok_or(R::Pressure)?;
        let live = limits
            .family
            .maximum_conversion_requested_bytes
            .checked_mul(limits.concurrent_native_values)
            .ok_or(R::Pressure)?;
        let combined = construction
            .max(all_retained)
            .checked_add(static_bytes)
            .and_then(|n| n.checked_add(live))
            .and_then(|n| n.checked_add(limits.other_reserved_bytes))
            .ok_or(R::Pressure)?;
        if all_retained > limits.maximum_retained_bytes
            || construction > limits.maximum_preparation_peak_bytes
            || combined > limits.maximum_combined_bytes
        {
            return Err(R::Pressure);
        }
        let mut retained = 0usize;
        let mut peak = 0usize;
        let mut prepare = |kind: ProductionFamily| -> Result<_, R> {
            // Earlier complete owners stay charged while the next family is
            // decoded. Admit the entire declared preparation ceiling first.
            let admitted = retained
                .checked_add(limits.family.maximum_preparation_peak_bytes)
                .and_then(|n| n.checked_add(owner_bytes))
                .ok_or(R::Pressure)?;
            if admitted > limits.maximum_preparation_peak_bytes {
                return Err(R::Pressure);
            }
            let family =
                PreparedNativeFamily::prepare(kind.roots(), limits.family).map_err(R::Native)?;
            let receipt = family.storage_receipt();
            peak = peak.max(
                retained
                    .checked_add(receipt.preparation_peak_heap_bytes_bound)
                    .and_then(|n| n.checked_add(owner_bytes))
                    .ok_or(R::Pressure)?,
            );
            retained = retained
                .checked_add(receipt.retained_heap_bytes_bound)
                .and_then(|n| n.checked_add(owner_bytes))
                .ok_or(R::Pressure)?;
            if retained > limits.maximum_retained_bytes {
                return Err(R::Pressure);
            }
            Ok((Rc::new(RefCell::new(family)), receipt))
        };
        let (family0, receipt0) = prepare(ProductionFamily::Derivation)?;
        let (family1, receipt1) = prepare(ProductionFamily::Lifecycle)?;
        let (family2, receipt2) = prepare(ProductionFamily::StabilizeCommit)?;
        let (family3, receipt3) = prepare(ProductionFamily::OriginsRebase)?;
        let (family4, receipt4) = prepare(ProductionFamily::BranchMask)?;
        let (family5, receipt5) = prepare(ProductionFamily::IndependentCommit)?;
        Ok(Self {
            families: [family0, family1, family2, family3, family4, family5],
            receipt: ProductionFamilyReceipt {
                retained_heap_bytes_bound: retained,
                preparation_peak_heap_bytes_bound: peak,
                families: [receipt0, receipt1, receipt2, receipt3, receipt4, receipt5],
                static_resources,
            },
        })
    }
    pub(crate) fn receipt(&self) -> ProductionFamilyReceipt {
        self.receipt
    }
    pub(crate) fn for_entry(
        &self,
        entry: ParserSessionEntry,
    ) -> Result<Rc<RefCell<PreparedNativeFamily>>, ProductionFamilyRefusal> {
        let (input, output) =
            definitions::port_descriptors(entry).ok_or(ProductionFamilyRefusal::PrimitiveOutput)?;
        self.families
            .iter()
            .find(|owner| {
                let family = owner.borrow();
                family.contains_descriptor(input) && family.contains_descriptor(output)
            })
            .cloned()
            .ok_or(ProductionFamilyRefusal::MissingDescriptor)
    }
    pub(crate) fn for_values<I: PreparedNativeRustBinding, O: PreparedNativeRustBinding>(
        &self,
    ) -> Result<Rc<RefCell<PreparedNativeFamily>>, ProductionFamilyRefusal> {
        self.families
            .iter()
            .find(|owner| {
                let family = owner.borrow();
                family.contains_descriptor(I::PREPARED_DESCRIPTOR)
                    && family.contains_descriptor(O::PREPARED_DESCRIPTOR)
            })
            .cloned()
            .ok_or(ProductionFamilyRefusal::MissingDescriptor)
    }
}
