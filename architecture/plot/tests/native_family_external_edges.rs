//! External metadata mismatches refuse before any family-owned allocation.
use conduit_core::{KindId, StructuredInfoType};
use conduit_plot::rust_binding::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Allocator;
thread_local! { static TRACK: Cell<Option<usize>> = const { Cell::new(None) }; }
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = TRACK.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let _ = TRACK.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn limits() -> PreparedNativeFamilyLimits {
    PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 64,
        maximum_input_bytes: 1024,
        maximum_retained_bytes: 1024 * 1024,
        maximum_preparation_peak_bytes: 2 * 1024 * 1024,
        maximum_conversion_requested_bytes: 1024 * 1024,
    }
}

fn leaf(kind: &str) -> &'static NativeFamilyTypeDescriptor {
    let ty = StructuredInfoType::nominal(
        KindId::new("test/child"),
        StructuredInfoType::leaf(KindId::new(kind)).unwrap(),
    )
    .unwrap();
    Box::leak(Box::new(NativeFamilyTypeDescriptor {
        type_bytes: Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice()),
        laws: &[],
        contracts: &[],
        children: &[],
        external_edges: &[],
        conversion_profile: NativeFamilyConversionProfile::Nominal,
        maximum_inline_bytes: 8,
    }))
}

fn root(
    actual: &'static NativeFamilyTypeDescriptor,
    expected: &'static NativeFamilyTypeDescriptor,
) -> &'static NativeFamilyTypeDescriptor {
    let ty = StructuredInfoType::nominal(
        KindId::new("test/parent"),
        StructuredInfoType::from_canonical_bytes(actual.type_bytes).unwrap(),
    )
    .unwrap();
    Box::leak(Box::new(NativeFamilyTypeDescriptor {
        type_bytes: Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice()),
        laws: &[],
        contracts: &[],
        children: Box::leak(vec![actual].into_boxed_slice()),
        external_edges: Box::leak(
            vec![NativeFamilyExternalEdge {
                descriptor: actual,
                expected,
            }]
            .into_boxed_slice(),
        ),
        conversion_profile: NativeFamilyConversionProfile::Nominal,
        maximum_inline_bytes: 8,
    }))
}

fn shadow(
    original: &'static NativeFamilyTypeDescriptor,
    laws: &'static [&'static [u8]],
    contracts: &'static [NativeFamilyContractDescriptor],
) -> &'static NativeFamilyTypeDescriptor {
    Box::leak(Box::new(NativeFamilyTypeDescriptor {
        type_bytes: original.type_bytes,
        laws,
        contracts,
        children: original.children,
        external_edges: &[],
        conversion_profile: original.conversion_profile,
        // Shadow layout never grants conversion authority or changes identity.
        maximum_inline_bytes: 0,
    }))
}

fn refused_without_allocation(root: &'static NativeFamilyTypeDescriptor, capacity: bool) {
    TRACK.with(|count| count.set(Some(0)));
    let result = PreparedNativeFamily::prepare(&[root], limits());
    let allocations = TRACK.with(|count| count.replace(None).unwrap());
    assert_eq!(allocations, 0);
    if capacity {
        assert!(matches!(result, Err(PreparedNativeFamilyRefusal::Capacity)));
    } else {
        assert!(matches!(
            result,
            Err(PreparedNativeFamilyRefusal::ConflictingDescriptor)
        ));
    }
}

#[test]
fn exact_owning_descriptor_passes_and_equal_schema_with_different_type_refuses() {
    let actual = leaf("value/u64");
    let expected = shadow(actual, &[], &[]);
    let family = PreparedNativeFamily::prepare(&[root(actual, expected)], limits()).unwrap();
    assert!(family.contains_descriptor(actual));
    assert!(!family.contains_descriptor(expected));
    refused_without_allocation(root(actual, leaf("value/u32")), false);
}

#[test]
fn omitted_reordered_laws_and_changed_leaf_contracts_refuse_before_allocation() {
    let original = leaf("value/u64");
    let actual = shadow(original, &[&[1], &[2]], &[]);
    refused_without_allocation(root(actual, shadow(original, &[&[1]], &[])), false);
    refused_without_allocation(root(actual, shadow(original, &[&[2], &[1]], &[])), false);
    let actual = shadow(
        original,
        &[],
        &[NativeFamilyContractDescriptor {
            representation_path: "representation",
            value_kind: "value/u64",
            maximum_bytes: 8,
            constraints: &[NativeFamilyConstraintDescriptor::CanonicalMembership {
                members: &[&[1]],
                negated: true,
            }],
        }],
    );
    for expected in [
        shadow(original, &[], &[]),
        shadow(
            original,
            &[],
            &[NativeFamilyContractDescriptor {
                representation_path: "representation",
                value_kind: "value/u64",
                maximum_bytes: 8,
                constraints: &[NativeFamilyConstraintDescriptor::CanonicalMembership {
                    members: &[&[2]],
                    negated: true,
                }],
            }],
        ),
    ] {
        refused_without_allocation(root(actual, expected), false);
    }
}

#[test]
fn metadata_member_extent_is_bounded_before_comparison_or_allocation() {
    static OVERSIZED: [u8; conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1] =
        [0; conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1];
    static MEMBERS: [&[u8]; 1] = [&OVERSIZED];
    static CONSTRAINTS: [NativeFamilyConstraintDescriptor; 1] =
        [NativeFamilyConstraintDescriptor::CanonicalMembership {
            members: &MEMBERS,
            negated: true,
        }];
    static CONTRACTS: [NativeFamilyContractDescriptor; 1] = [NativeFamilyContractDescriptor {
        representation_path: "representation",
        value_kind: "value/u64",
        maximum_bytes: 8,
        constraints: &CONSTRAINTS,
    }];
    let original = leaf("value/u64");
    let oversized = shadow(original, &[], &CONTRACTS);
    refused_without_allocation(root(oversized, oversized), true);
}

#[test]
fn referenced_child_must_be_present_as_the_complete_original_parent_type() {
    let actual = leaf("value/u64");
    let correct = root(actual, shadow(actual, &[], &[]));
    let foreign = leaf("value/u32");
    let invalid = Box::leak(Box::new(NativeFamilyTypeDescriptor {
        type_bytes: correct.type_bytes,
        laws: &[],
        contracts: &[],
        children: Box::leak(vec![foreign].into_boxed_slice()),
        external_edges: &[],
        conversion_profile: NativeFamilyConversionProfile::Nominal,
        maximum_inline_bytes: 8,
    }));
    assert!(matches!(
        PreparedNativeFamily::prepare(&[invalid], limits()),
        Err(PreparedNativeFamilyRefusal::ConflictingDescriptor)
    ));
}
