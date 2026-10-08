//! Native-only immutable child admission. Source derivation and commitment remain
//! separate authorities owned by the caller. No global or mutable skip state.
use super::{
    NativeBindingRefusal, NativeFamilyTypeDescriptor, PreparedNativeFamily,
    PreparedNativeFamilyLimits, PreparedNativeFamilyRefusal, PreparedNativeRustBinding,
};
use alloc::rc::Rc;
use conduit_core::{
    validate_canonical_structured_value, StructuredInfoRefusal, ValidatedCanonicalStructuredValue,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES, MAXIMUM_STRUCTURED_INFO_DEPTH,
};
use core::mem::{align_of, size_of};

pub const MAXIMUM_ADMITTED_NATIVE_CHILDREN: usize = 16;
pub(super) struct NativeAdmissionDomain {
    _private: (),
}
// Preserve the existing family's Send behavior on atomic-capable targets.
#[cfg(target_has_atomic = "ptr")]
pub(super) type NativeAdmissionDomainOwner = alloc::sync::Arc<NativeAdmissionDomain>;
#[cfg(not(target_has_atomic = "ptr"))]
pub(super) type NativeAdmissionDomainOwner = Rc<NativeAdmissionDomain>;
const DOMAIN_BYTES: usize = 2 * size_of::<usize>() + size_of::<NativeAdmissionDomain>();

/// Created only by full fresh Native admission. The complete frame is immutable
/// while any capability retains its Rc. This is neither Source nor fact authority.
#[derive(Clone)]
pub struct AdmittedNativeChild {
    domain: NativeAdmissionDomainOwner,
    descriptor: &'static NativeFamilyTypeDescriptor,
    canonical: Rc<[u8]>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeChildStorageReceipt {
    pub inline_bytes: usize,
    /// Conservatively counts shared domain/frame storage for every capability.
    pub retained_heap_bytes_bound: usize,
    pub combined_bytes_bound: usize,
}
impl AdmittedNativeChild {
    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }
    pub const fn descriptor(&self) -> &'static NativeFamilyTypeDescriptor {
        self.descriptor
    }
    pub fn storage_receipt(&self) -> NativeChildStorageReceipt {
        Self::storage_reservation(self.canonical.len()).expect("admitted finite canonical frame")
    }
    /// Allocation-free reservation, available before the caller creates its Rc.
    pub fn storage_reservation(
        canonical_bytes: usize,
    ) -> Result<NativeChildStorageReceipt, NativeBindingRefusal> {
        if canonical_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(wrong_type());
        }
        let retained_heap_bytes_bound =
            DOMAIN_BYTES + 2 * size_of::<usize>() + canonical_bytes + align_of::<usize>() - 1;
        Ok(NativeChildStorageReceipt {
            inline_bytes: size_of::<Self>(),
            retained_heap_bytes_bound,
            combined_bytes_bound: size_of::<Self>() + retained_heap_bytes_bound,
        })
    }
}

/// Authenticated immutable conversion scope. Public only for generated bindings;
/// callers cannot manufacture one or select an unchecked conversion mode.
#[derive(Clone, Copy)]
pub struct NativeChildAdmissionScope<'a> {
    domain: &'a NativeAdmissionDomainOwner,
    children: &'a [&'a AdmittedNativeChild],
    depth: usize,
    authenticated: Option<ValidatedCanonicalStructuredValue<'a>>,
}
pub struct NativeNodeAdmissionScope<'a> {
    scope: NativeChildAdmissionScope<'a>,
}
impl<'a> NativeChildAdmissionScope<'a> {
    /// Bounds the additional logical scope records across the canonical depth.
    /// Generated Native layouts/conversion allocations are charged separately.
    pub const fn maximum_scope_state_bytes() -> usize {
        (MAXIMUM_STRUCTURED_INFO_DEPTH + 1)
            * (size_of::<Self>() + size_of::<NativeNodeAdmissionScope<'a>>())
            + size_of::<NativeAdmissionDomainOwner>()
    }
    pub fn for_node<'node>(
        &'node self,
        family: &PreparedNativeFamily,
        descriptor: &'static NativeFamilyTypeDescriptor,
        value: ValidatedCanonicalStructuredValue<'node>,
    ) -> Result<NativeNodeAdmissionScope<'node>, NativeBindingRefusal> {
        if self.depth > MAXIMUM_STRUCTURED_INFO_DEPTH
            || !family
                .admission_domain
                .as_ref()
                .is_some_and(|domain| NativeAdmissionDomainOwner::ptr_eq(domain, self.domain))
        {
            return Err(wrong_type());
        }
        family.check_type(descriptor, value)?;
        // The root parent always executes every original own contract and law.
        let inherited = self.authenticated.filter(|ancestor| {
            contained(ancestor.type_bytes(), value.type_bytes())
                && contained(ancestor.value_node(), value.value_node())
        });
        let direct = self.depth > 0
            && self.children.iter().any(|child| {
                core::ptr::eq(child.descriptor, descriptor)
                    && NativeAdmissionDomainOwner::ptr_eq(&child.domain, self.domain)
                    && child.canonical.strip_prefix(descriptor.type_bytes)
                        == Some(value.value_node())
            });
        let authenticated = if direct { Some(value) } else { inherited };
        Ok(NativeNodeAdmissionScope {
            scope: NativeChildAdmissionScope {
                domain: self.domain,
                children: self.children,
                depth: self.depth,
                authenticated,
            },
        })
    }
}
impl<'a> NativeNodeAdmissionScope<'a> {
    pub const fn requires_validation(&self) -> bool {
        self.scope.authenticated.is_none()
    }
    pub fn child_scope(&self) -> Result<NativeChildAdmissionScope<'a>, NativeBindingRefusal> {
        let depth = self.scope.depth.checked_add(1).ok_or_else(wrong_type)?;
        if depth > MAXIMUM_STRUCTURED_INFO_DEPTH + 1 {
            return Err(wrong_type());
        }
        Ok(NativeChildAdmissionScope {
            depth,
            ..self.scope
        })
    }
}
impl PreparedNativeFamily {
    /// Opt-in unique family domain. Its shared allocation is admitted before any
    /// family preparation; the default preparation/conversion path is unchanged.
    pub fn prepare_with_child_admission(
        roots: &[&'static NativeFamilyTypeDescriptor],
        mut limits: PreparedNativeFamilyLimits,
    ) -> Result<Self, PreparedNativeFamilyRefusal> {
        limits.maximum_retained_bytes = limits
            .maximum_retained_bytes
            .checked_sub(DOMAIN_BYTES)
            .ok_or(PreparedNativeFamilyRefusal::Capacity)?;
        limits.maximum_preparation_peak_bytes = limits
            .maximum_preparation_peak_bytes
            .checked_sub(DOMAIN_BYTES)
            .ok_or(PreparedNativeFamilyRefusal::Capacity)?;
        let mut family = Self::prepare(roots, limits)?;
        family.admission_domain = Some(NativeAdmissionDomainOwner::new(NativeAdmissionDomain {
            _private: (),
        }));
        family.receipt.retained_heap_bytes_bound = family
            .receipt
            .retained_heap_bytes_bound
            .checked_add(DOMAIN_BYTES)
            .ok_or(PreparedNativeFamilyRefusal::Capacity)?;
        family.receipt.preparation_peak_heap_bytes_bound = family
            .receipt
            .preparation_peak_heap_bytes_bound
            .checked_add(DOMAIN_BYTES)
            .ok_or(PreparedNativeFamilyRefusal::Capacity)?;
        Ok(family)
    }
    /// Full original recursive admission occurs before issuing the capability.
    /// Caller admits/owns the incoming Rc frame allocation and Native conversion
    /// peak separately; cloning its ownership here allocates no heap.
    pub fn decode_admitted<T: PreparedNativeRustBinding>(
        &mut self,
        canonical: Rc<[u8]>,
        maximum_capability_retained_bytes: usize,
    ) -> Result<(T, AdmittedNativeChild), NativeBindingRefusal> {
        let domain = self.admission_domain.clone().ok_or_else(wrong_type)?;
        if AdmittedNativeChild::storage_reservation(canonical.len())?.combined_bytes_bound
            > maximum_capability_retained_bytes
        {
            return Err(wrong_type());
        }
        let value = self.decode::<T>(&canonical)?;
        // Capability issuance independently checks every named subtree. Public
        // custom binding implementations cannot manufacture an admission proof
        // by omitting their recursive validation calls.
        let view = validate_canonical_structured_value(&canonical)
            .map_err(NativeBindingRefusal::InvalidValue)?;
        view.try_visit_nodes(&mut |node| {
            // All matching descriptors are checked, so even a public family
            // containing equal Type bytes with distinct metadata cannot hide a law.
            for index in 0..self.types.len() {
                let descriptor = self.types[index].descriptor;
                if descriptor.type_bytes == node.type_bytes() {
                    self.validate(descriptor, node)
                        .map_err(AdmissionVisitRefusal::Native)?;
                }
            }
            Ok::<(), AdmissionVisitRefusal>(())
        })
        .map_err(|refusal| match refusal {
            AdmissionVisitRefusal::Native(refusal) => refusal,
            AdmissionVisitRefusal::Structured(refusal) => {
                NativeBindingRefusal::InvalidValue(refusal)
            }
        })?;
        Ok((
            value,
            AdmittedNativeChild {
                domain,
                descriptor: T::PREPARED_DESCRIPTOR,
                canonical,
            },
        ))
    }
    /// Reuses only fully admitted byte-identical named child subtrees. Every
    /// root-parent law and every unmatched child's original recursive path runs.
    pub fn decode_with_admitted_children<T: PreparedNativeRustBinding>(
        &mut self,
        canonical: &[u8],
        children: &[&AdmittedNativeChild],
        maximum_scope_state_bytes: usize,
    ) -> Result<T, NativeBindingRefusal> {
        let domain = self.admission_domain.clone().ok_or_else(wrong_type)?;
        if children.len() > MAXIMUM_ADMITTED_NATIVE_CHILDREN
            || canonical.len() > self.maximum_input_bytes
            || maximum_scope_state_bytes < NativeChildAdmissionScope::maximum_scope_state_bytes()
        {
            return Err(wrong_type());
        }
        for child in children {
            if !NativeAdmissionDomainOwner::ptr_eq(&child.domain, &domain)
                || !self.contains_descriptor(child.descriptor)
            {
                return Err(wrong_type());
            }
        }
        let value = validate_canonical_structured_value(canonical)
            .map_err(NativeBindingRefusal::InvalidValue)?;
        self.check_type(T::PREPARED_DESCRIPTOR, value)?;
        let scope = NativeChildAdmissionScope {
            domain: &domain,
            children,
            depth: 0,
            authenticated: None,
        };
        T::from_borrowed_prepared_with_children(value, self, &scope)
    }
}
fn wrong_type() -> NativeBindingRefusal {
    NativeBindingRefusal::InvalidValue(StructuredInfoRefusal::WrongType)
}

fn contained(ancestor: &[u8], child: &[u8]) -> bool {
    let start = ancestor.as_ptr() as usize;
    let child_start = child.as_ptr() as usize;
    match (
        start.checked_add(ancestor.len()),
        child_start.checked_add(child.len()),
    ) {
        (Some(end), Some(child_end)) => start <= child_start && child_end <= end,
        _ => false,
    }
}

enum AdmissionVisitRefusal {
    Structured(StructuredInfoRefusal),
    Native(NativeBindingRefusal),
}
impl From<StructuredInfoRefusal> for AdmissionVisitRefusal {
    fn from(refusal: StructuredInfoRefusal) -> Self {
        Self::Structured(refusal)
    }
}
