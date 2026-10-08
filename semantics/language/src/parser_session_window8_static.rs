//! Read-only canonical artifacts are separate from prepared metadata heap.
//! Complete byte lengths are charged; pointers only deduplicate storage owners.
use crate::parser_session_window8_ports::{FAMILY_ROOTS, PORTS};
use conduit_plot::rust_binding::{
    NativeFamilyConstraintDescriptor as Constraint, NativeFamilyTypeDescriptor,
};
use core::mem::size_of;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8StaticResourceReceipt {
    pub descriptors: usize,
    pub canonical_bytes_bound: usize,
    pub descriptor_storage_bytes_bound: usize,
    pub source_canonical_bytes_bound: usize,
}
pub(crate) fn descriptor_resources(
    extra_roots: &[&'static NativeFamilyTypeDescriptor],
) -> Option<Window8StaticResourceReceipt> {
    // Actual descriptors and independent external expectation shadows are
    // separate read-only owners. Finite512 inventory slots cover both; this
    // changes neither generated256 nor runtime64/root16 admission limits.
    let mut seen: [Option<&'static NativeFamilyTypeDescriptor>; 512] = [None; 512];
    let mut count = 0usize;
    // Include the actual top-level table and the two static slice handles,
    // independently of their referenced arrays accounted below.
    let mut storage = FAMILY_ROOTS
        .len()
        .checked_mul(size_of::<&[&NativeFamilyTypeDescriptor]>())?
        .checked_add(size_of::<&[&[&NativeFamilyTypeDescriptor]]>())?
        .checked_add(size_of::<
            &[crate::parser_session_window8_ports::Window8PortSpec],
        >())?;
    let mut canonical = 0usize;
    fn insert(
        seen: &mut [Option<&'static NativeFamilyTypeDescriptor>; 512],
        count: &mut usize,
        descriptor: &'static NativeFamilyTypeDescriptor,
    ) -> Option<()> {
        if seen[..*count]
            .iter()
            .flatten()
            .any(|previous| core::ptr::eq(*previous, descriptor))
        {
            return Some(());
        }
        *seen.get_mut(*count)? = Some(descriptor);
        *count += 1;
        Some(())
    }
    for roots in FAMILY_ROOTS
        .iter()
        .copied()
        .chain(core::iter::once(extra_roots))
    {
        storage = storage.checked_add(
            roots
                .len()
                .checked_mul(size_of::<&NativeFamilyTypeDescriptor>())?,
        )?;
        for descriptor in roots {
            insert(&mut seen, &mut count, descriptor)?;
        }
    }
    let mut index = 0;
    while index < count {
        let descriptor = seen[index]?;
        index += 1;
        storage = storage
            .checked_add(size_of::<NativeFamilyTypeDescriptor>())?
            .checked_add(
                descriptor
                    .children
                    .len()
                    .checked_mul(size_of::<&NativeFamilyTypeDescriptor>())?,
            )?
            .checked_add(descriptor.laws.len().checked_mul(size_of::<&[u8]>())?)?
            .checked_add(descriptor.contracts.len().checked_mul(size_of::<
                conduit_plot::rust_binding::NativeFamilyContractDescriptor,
            >())?)?;
        canonical = canonical.checked_add(descriptor.type_bytes.len())?;
        for law in descriptor.laws {
            canonical = canonical.checked_add(law.len())?;
        }
        for recipe in descriptor.contracts {
            canonical = canonical
                .checked_add(recipe.representation_path.len())?
                .checked_add(recipe.value_kind.len())?;
            storage = storage.checked_add(
                recipe
                    .constraints
                    .len()
                    .checked_mul(size_of::<Constraint>())?,
            )?;
            for constraint in recipe.constraints {
                match constraint {
                    Constraint::CanonicalMembership { members, .. } => {
                        storage =
                            storage.checked_add(members.len().checked_mul(size_of::<&[u8]>())?)?;
                        for member in *members {
                            canonical = canonical.checked_add(member.len())?;
                        }
                    }
                    Constraint::FixedIntegerRange {
                        minimum, maximum, ..
                    } => {
                        canonical = canonical
                            .checked_add(minimum.map_or(0, <[u8]>::len))?
                            .checked_add(maximum.map_or(0, <[u8]>::len))?;
                    }
                }
            }
        }
        for child in descriptor.children {
            insert(&mut seen, &mut count, child)?;
        }
        storage =
            storage.checked_add(descriptor.external_edges.len().checked_mul(size_of::<
                conduit_plot::rust_binding::NativeFamilyExternalEdge,
            >())?)?;
        for edge in descriptor.external_edges {
            insert(&mut seen, &mut count, edge.descriptor)?;
            insert(&mut seen, &mut count, edge.expected)?;
        }
    }
    let mut source = 0usize;
    storage = storage.checked_add(PORTS.len().checked_mul(size_of::<
        crate::parser_session_window8_ports::Window8PortSpec,
    >())?)?;
    for port in PORTS {
        source = source
            .checked_add(port.name.len())?
            .checked_add(port.original_programs.len())?
            .checked_add(port.original_custody.len())?;
    }
    // Bare model boundaries are fixed original Source artifacts even though
    // their primitive aliases deliberately have no Native-family descriptor.
    use crate::parser_session_execution::ParserSessionEntry as E;
    for entry in [E::ProposalWindow8V2Indices, E::ProposalWindow8V2Scores] {
        source = source
            .checked_add(entry.name().len())?
            .checked_add(entry.program_hex().len())?
            .checked_add(entry.chain_custody().len())?;
    }
    Some(Window8StaticResourceReceipt {
        descriptors: count,
        canonical_bytes_bound: canonical,
        descriptor_storage_bytes_bound: storage,
        source_canonical_bytes_bound: source,
    })
}
