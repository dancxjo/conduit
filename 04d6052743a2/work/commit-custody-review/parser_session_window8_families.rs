//! Simultaneous finite ownership of the four complete Window8 Native families.
//! Construction, immutable schema artifacts and concurrent conversion envelopes
//! are admitted together before the first family allocation.
use crate::{
    parser_session_window8_ports::{family_for, FAMILY_ROOTS, PORTS},
    parser_session_window8_static::{descriptor_resources, Window8StaticResourceReceipt},
};
use alloc::{rc::Rc, vec::Vec};
use conduit_plot::rust_binding::{
    NativeFamilyTypeDescriptor, PreparedNativeFamily, PreparedNativeFamilyLimits,
    PreparedNativeFamilyRefusal, PreparedNativeFamilyStorageReceipt,
};
use core::{
    cell::RefCell,
    mem::{align_of, size_of},
};
#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8FamilyLimits {
    pub(crate) families: [PreparedNativeFamilyLimits; 4],
    pub(crate) maximum_retained_bytes: usize,
    pub(crate) maximum_preparation_peak_bytes: usize,
    pub(crate) maximum_static_bytes: usize,
    pub(crate) concurrent_native_values: usize,
    pub(crate) other_reserved_bytes: usize,
    pub(crate) maximum_combined_bytes: usize,
}
#[derive(Debug)]
pub(crate) enum Window8FamilyRefusal {
    Capacity,
    Descriptor,
    Native(PreparedNativeFamilyRefusal),
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8FamilyReceipt {
    pub(crate) retained_heap_bytes_bound: usize,
    pub(crate) preparation_peak_bytes_bound: usize,
    pub(crate) active_native_bytes_bound: usize,
    pub(crate) static_resources: Window8StaticResourceReceipt,
}
pub(crate) struct Window8Families {
    pub(crate) owners: Vec<Rc<RefCell<PreparedNativeFamily>>>,
    pub(crate) receipt: Window8FamilyReceipt,
}
fn add(a: usize, b: usize) -> Result<usize, Window8FamilyRefusal> {
    a.checked_add(b).ok_or(Window8FamilyRefusal::Capacity)
}
impl Window8Families {
    /// Extra descriptor roots retain read-only model-signature expectations too;
    /// their prepared heap owners are separately part of other_reserved_bytes.
    pub(crate) fn prepare(
        limits: Window8FamilyLimits,
        extra_static_roots: &[&'static NativeFamilyTypeDescriptor],
    ) -> Result<Self, Window8FamilyRefusal> {
        use Window8FamilyRefusal as R;
        if FAMILY_ROOTS.len() != 4 || limits.concurrent_native_values == 0 {
            return Err(R::Capacity);
        }
        let resources = descriptor_resources(extra_static_roots).ok_or(R::Capacity)?;
        let static_bytes = add(
            add(
                resources.canonical_bytes_bound,
                resources.descriptor_storage_bytes_bound,
            )?,
            resources.source_canonical_bytes_bound,
        )?;
        let slots = 4usize
            .checked_mul(size_of::<Rc<RefCell<PreparedNativeFamily>>>())
            .ok_or(R::Capacity)?;
        let owner = add(
            add(
                size_of::<RefCell<PreparedNativeFamily>>(),
                2 * size_of::<usize>(),
            )?,
            4 * align_of::<RefCell<PreparedNativeFamily>>(),
        )?;
        let mut retained = slots;
        let mut peak = slots;
        let mut conversion = 0usize;
        for bound in limits.families {
            peak = peak.max(add(
                add(retained, bound.maximum_preparation_peak_bytes)?,
                owner,
            )?);
            retained = add(add(retained, bound.maximum_retained_bytes)?, owner)?;
            conversion = conversion.max(bound.maximum_conversion_requested_bytes);
        }
        let active = conversion
            .checked_mul(limits.concurrent_native_values)
            .ok_or(R::Capacity)?;
        let combined = add(
            add(add(peak.max(retained), static_bytes)?, active)?,
            limits.other_reserved_bytes,
        )?;
        if static_bytes > limits.maximum_static_bytes
            || retained > limits.maximum_retained_bytes
            || peak > limits.maximum_preparation_peak_bytes
            || combined > limits.maximum_combined_bytes
        {
            return Err(R::Capacity);
        }
        let mut owners = Vec::new();
        owners.try_reserve_exact(4).map_err(|_| R::Capacity)?;
        if owners.capacity() != 4 {
            return Err(R::Capacity);
        }
        let mut retained = slots;
        let mut peak = slots;
        let mut active = 0usize;
        for (roots, bound) in FAMILY_ROOTS.iter().zip(limits.families) {
            let family = PreparedNativeFamily::prepare(roots, bound).map_err(R::Native)?;
            let r: PreparedNativeFamilyStorageReceipt = family.storage_receipt();
            peak = peak.max(add(
                add(retained, r.preparation_peak_heap_bytes_bound)?,
                owner,
            )?);
            retained = add(add(retained, r.retained_heap_bytes_bound)?, owner)?;
            active = active.max(r.conversion_requested_bytes_bound);
            owners.push(Rc::new(RefCell::new(family)));
        }
        // Every fixed original Source I/O descriptor must be owned exactly,
        // before any target ingress. Equal schema bytes are insufficient.
        for port in PORTS {
            family_for(&owners, port.input).map_err(|_| R::Descriptor)?;
            family_for(&owners, port.output).map_err(|_| R::Descriptor)?;
        }
        active = active
            .checked_mul(limits.concurrent_native_values)
            .ok_or(R::Capacity)?;
        Ok(Self {
            owners,
            receipt: Window8FamilyReceipt {
                retained_heap_bytes_bound: retained,
                preparation_peak_bytes_bound: peak,
                active_native_bytes_bound: active,
                static_resources: resources,
            },
        })
    }
}
