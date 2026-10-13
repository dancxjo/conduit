use super::*;
#[test]
fn native_integer_path_and_exact_rationals() {
    assert_eq!(
        MathValue::integer(2).checked_add(&MathValue::integer(3)),
        Ok(MathValue::integer(5))
    );
    let third = MathValue::rational("1", "3").unwrap();
    assert_eq!(
        third.checked_mul(&MathValue::integer(3)),
        Ok(MathValue::ONE)
    );
    assert_eq!(
        MathValue::parse_decimal("0.5"),
        MathValue::rational("1", "2")
    );
}
#[test]
fn thousand_digit_integer_cancellation() {
    let huge = MathValue::parse_decimal("1e1000").unwrap();
    let plus = huge.checked_add(&MathValue::ONE).unwrap();
    assert_eq!(plus.checked_sub(&huge), Ok(MathValue::ONE));
    assert_eq!(
        MathValue::decode(&plus.canonical_bytes().unwrap()),
        Ok(plus)
    );
    assert_eq!(
        MathValue::parse_decimal("1e1152"),
        Err(NumericRefusal::IntegerBudget)
    );
}
#[test]
fn pi_rational_factors_and_quarter_turn_laws() {
    let pi = MathValue::pi();
    let twice = pi.checked_mul(&MathValue::integer(2)).unwrap();
    assert_eq!(twice.checked_div(&pi), Ok(MathValue::integer(2)));
    assert_eq!(pi.checked_add(&pi), Ok(twice));
    assert_eq!(
        MathValue::rational("1", "4").unwrap().sin_turns(),
        Ok(MathValue::ONE)
    );
    assert_eq!(
        MathValue::rational("1", "4").unwrap().tan_turns(),
        Err(NumericRefusal::Domain)
    );
}
#[test]
fn finite_roots_and_unknown_identity() {
    let root = MathValue::integer(2).checked_root(2).unwrap();
    assert_eq!(
        MathValue::integer(9).checked_root(2),
        Ok(MathValue::integer(3))
    );
    assert_eq!(
        MathValue::integer(-1).checked_root(2),
        Err(NumericRefusal::Domain)
    );
    assert_eq!(
        root.mathematical_equality(&MathValue::pi()),
        Ok(MathematicalEquality::Unknown)
    );
    let bytes = root.canonical_bytes().unwrap();
    assert_eq!(MathValueRef::validate(&bytes).unwrap().to_owned(), Ok(root));
}
#[test]
fn symbolic_commutation_preserves_identity() {
    let root = MathValue::integer(2).checked_root(2).unwrap();
    let a = root.checked_mul(&MathValue::pi()).unwrap();
    let b = MathValue::pi().checked_mul(&root).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.representation_identity(), b.representation_identity());
}
#[test]
fn guards_prevent_invalid_cancellation() {
    let zero = MathValue::ZERO;
    assert_eq!(zero.checked_div(&zero), Err(NumericRefusal::DivisionByZero));
    assert_eq!(
        MathValue::integer(2).asin_turns(),
        Err(NumericRefusal::Domain)
    );
    assert_eq!(
        MathValue::atan2_turns(&zero, &zero),
        Err(NumericRefusal::Domain)
    );
}
#[test]
fn symbolic_cancellation_respects_nonzero_proofs() {
    let twice_pi = MathValue::pi().checked_mul(&MathValue::integer(2)).unwrap();
    let scale = MathValue::ONE.checked_div(&twice_pi).unwrap();
    assert_eq!(scale.checked_mul(&twice_pi), Ok(MathValue::ONE));
    let root = MathValue::integer(2).checked_root(2).unwrap();
    assert_eq!(root.checked_mul(&root), Ok(MathValue::integer(2)));
    let negative = MathValue::integer(-2).checked_root(3).unwrap();
    assert_eq!(negative.checked_div(&negative), Ok(MathValue::ONE));
}
#[test]
fn intrinsic_profiles_admit_small_values_and_refuse_large_custody() {
    let profile = PortableNumericProfile {
        forms: NumericForms::INTEGER,
        maximum_integer_limbs: 3,
        maximum_symbolic_nodes: 0,
        maximum_symbolic_depth: 0,
        maximum_encoded_bytes: 10,
    };
    let id = profile.info_id().unwrap();
    assert_eq!(PortableNumericProfile::from_info_id(&id), Some(profile));
    assert_eq!(
        crate::validate_primitive_info(&id, &MathValue::integer(42).canonical_bytes().unwrap()),
        Ok(())
    );
    assert!(
        crate::validate_primitive_info(&id, &MathValue::pi().canonical_bytes().unwrap()).is_err()
    );
    assert!(
        crate::validate_primitive_info(
            "numeric/profile-01-3-0-0-10@1",
            &MathValue::integer(42).canonical_bytes().unwrap()
        )
        .is_err()
    );
    let huge = MathValue::parse_decimal("1e1000").unwrap();
    assert_eq!(
        profile.validate(&huge.canonical_bytes().unwrap()).err(),
        Some(NumericRefusal::ProfileViolation)
    );
}
#[test]
fn borrowed_codec_refuses_cycles_redundancy_and_domain_forgery() {
    let root = MathValue::integer(2).checked_root(2).unwrap();
    let mut bytes = root.canonical_bytes().unwrap();
    let n = bytes.len();
    bytes[n - 2..].copy_from_slice(&1u16.to_le_bytes());
    assert!(MathValueRef::validate(&bytes).is_err());
    let mut forged = alloc::vec![1, 3, 2, 0, 0, 10, 0];
    forged.extend_from_slice(&MathValue::integer(2).canonical_bytes().unwrap());
    forged.extend_from_slice(&[10, 0, 0, 0]);
    assert_eq!(
        MathValueRef::validate(&forged).err(),
        Some(NumericRefusal::Domain)
    );
    let mut redundant = alloc::vec![1, 3, 2, 0, 0, 10, 0];
    redundant.extend_from_slice(&MathValue::integer(9).canonical_bytes().unwrap());
    redundant.extend_from_slice(&[2, 2, 0, 0]);
    assert_eq!(
        MathValueRef::validate(&redundant).err(),
        Some(NumericRefusal::NonCanonical)
    );
}
#[test]
fn decimal_parts_and_integer_powers_are_exact() {
    assert_eq!(
        MathValue::from_decimal_parts(5, -1),
        MathValue::rational("1", "2")
    );
    assert_eq!(
        MathValue::integer(10).checked_pow(1000),
        MathValue::parse_decimal("1e1000")
    );
    assert_eq!(
        MathValue::integer(2).checked_pow(-3),
        MathValue::rational("1", "8")
    );
    assert_eq!(
        MathValue::integer(2).checked_pow(4097),
        Err(NumericRefusal::WorkBudget)
    );
}

extern crate std;
struct AdmissionAllocator;
std::thread_local! {
    static TRACK_ALLOCATIONS: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    static ALLOCATIONS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}
fn count_allocation() {
    let _ = TRACK_ALLOCATIONS.try_with(|track| {
        if track.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    });
}
// SAFETY: Every request is forwarded unchanged to the system allocator.
unsafe impl std::alloc::GlobalAlloc for AdmissionAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        count_allocation();
        unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, pointer, layout) }
    }
    unsafe fn realloc(
        &self,
        pointer: *mut u8,
        layout: std::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        count_allocation();
        unsafe { std::alloc::GlobalAlloc::realloc(&std::alloc::System, pointer, layout, new_size) }
    }
}
#[global_allocator]
static ADMISSION_ALLOCATOR: AdmissionAllocator = AdmissionAllocator;
#[test]
fn borrowed_validation_and_profile_checks_do_not_allocate() {
    let value = MathValue::integer(2)
        .checked_root(2)
        .unwrap()
        .checked_add(&MathValue::pi())
        .unwrap();
    let bytes = value.canonical_bytes().unwrap();
    let mut output = [0u8; MAX_NUMERIC_ENCODED_BYTES];
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    for _ in 0..100 {
        let admitted = PortableNumericProfile::MAXIMAL.validate(&bytes).unwrap();
        admitted.encode_into(&mut output).unwrap();
        let _ = admitted.representation_identity();
    }
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
}
#[test]
fn symbolic_angle_bridge_retains_exact_pi_cancellation() {
    let radians = MathValue::pi().checked_div(&MathValue::integer(2)).unwrap();
    let turns = radians
        .checked_div(&MathValue::pi().checked_mul(&MathValue::integer(2)).unwrap())
        .unwrap();
    assert_eq!(turns, MathValue::rational("1", "4").unwrap());
    assert_eq!(turns.sin_turns(), Ok(MathValue::ONE));
    let seventh = MathValue::pi()
        .checked_div(&MathValue::integer(7))
        .unwrap()
        .checked_div(&MathValue::pi().checked_mul(&MathValue::integer(2)).unwrap())
        .unwrap();
    let symbolic = seventh.sin_turns().unwrap();
    assert!(matches!(symbolic, MathValue::Symbolic(_)));
    assert_eq!(
        MathValue::decode(&symbolic.canonical_bytes().unwrap()),
        Ok(symbolic)
    );
}
#[test]
fn symbolic_depth_and_small_native_allocation_bounds() {
    let mut value = MathValue::pi();
    let mut refused = false;
    for _ in 0..MAX_SYMBOLIC_DEPTH + 1 {
        match value.sin_turns() {
            Ok(next) => value = next,
            Err(NumericRefusal::DepthBudget) => {
                refused = true;
                break;
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
    assert!(refused);
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    let result = MathValue::integer(4)
        .checked_add(&MathValue::integer(7))
        .unwrap()
        .checked_mul(&MathValue::integer(3))
        .unwrap();
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(result, MathValue::integer(33));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
}
#[test]
fn dyadic_boundaries_and_borrowed_symbolic_nodes() {
    let minimum = ExactRational::dyadic(1, -1074).unwrap();
    let doubled = minimum.checked_mul(&ExactRational::from_i64(2)).unwrap();
    assert_eq!(doubled, ExactRational::dyadic(1, -1073).unwrap());
    let root = MathValue::integer(2).checked_root(2).unwrap();
    let MathValue::Symbolic(dag) = root else {
        panic!("root remains symbolic")
    };
    assert_eq!(dag.nodes().len(), 2);
    assert!(matches!(
        dag.node(1),
        Some(SymbolicNodeRef::Apply {
            operation: SymbolicOperation::Root(2),
            ..
        })
    ));
}

#[test]
fn reciprocal_pi_coordinate_laws_cancel_exactly() {
    let pi = MathValue::pi();
    let radian_scale = MathValue::ONE
        .checked_div(&MathValue::integer(2).checked_mul(&pi).unwrap())
        .unwrap();
    let quarter_rad = pi.checked_div(&MathValue::integer(2)).unwrap();
    let quarter_turn = quarter_rad.checked_mul(&radian_scale).unwrap();
    assert_eq!(quarter_turn, MathValue::rational("1", "4").unwrap());
    assert_eq!(
        MathValue::ONE.checked_div(&radian_scale).unwrap(),
        MathValue::integer(2).checked_mul(&pi).unwrap()
    );
    assert_eq!(
        radian_scale.checked_mul(&pi).unwrap(),
        MathValue::rational("1", "2").unwrap()
    );
    assert_eq!(
        pi.checked_mul(&radian_scale).unwrap(),
        MathValue::rational("1", "2").unwrap()
    );
    let encoded = radian_scale.canonical_bytes().unwrap();
    assert!(MathValueRef::validate(&encoded).unwrap().proven_positive());
    assert_eq!(MathValue::decode(&encoded).unwrap(), radian_scale);
}
#[test]
fn borrowed_integer_and_micro_realization_projection_is_exact() {
    let third = MathValue::rational("1", "3")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        MathValueRef::validate(&third)
            .unwrap()
            .checked_microscalar(),
        Err(NumericProjectionRefusal::Inexact)
    );
    let exact = MathValue::rational("1", "1000000")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert_eq!(
        MathValueRef::validate(&exact)
            .unwrap()
            .checked_microscalar()
            .unwrap(),
        crate::MicroScalar::from_raw_microunits(1)
    );
    assert_eq!(
        MathValueRef::validate(&exact).unwrap().checked_i64(),
        Err(NumericProjectionRefusal::Inexact)
    );
    let pi = MathValue::pi().canonical_bytes().unwrap();
    assert_eq!(
        MathValueRef::validate(&pi).unwrap().checked_i64(),
        Err(NumericProjectionRefusal::UnresolvedSymbolic)
    );
}

#[test]
fn captured_math_quantity_affine_roles_profiles_and_angle_laws() {
    use crate::QuantityRole as R;
    use crate::quantity::numeric_quantity::{Quantity as Q, QuantityNumericRefusal, QuantityRef};
    use crate::{PrefixPolicy, UnitDefinition as U};
    let turn = U::new_math_role(
        crate::BUILTIN_TURN.family(),
        "turn",
        MathValue::ONE,
        MathValue::ZERO,
        PrefixPolicy::NONE,
        R::Linear,
    )
    .unwrap();
    let rad = U::related_math_role(
        &turn,
        "rad",
        MathValue::ONE
            .checked_div(&MathValue::integer(2).checked_mul(&MathValue::pi()).unwrap())
            .unwrap(),
        MathValue::ZERO,
        PrefixPolicy::NONE,
        R::Linear,
    )
    .unwrap();
    let radians = Q::from_math(
        MathValue::pi().checked_div(&MathValue::integer(2)).unwrap(),
        &rad,
    )
    .unwrap();
    let converted = radians.as_ref().convert(turn.as_ref()).unwrap();
    assert_eq!(
        converted.coordinate().to_owned().unwrap(),
        MathValue::rational("1", "4").unwrap()
    );
    let big = Q::from_math(
        MathValue::parse_decimal("1e1000")
            .unwrap()
            .checked_add(&MathValue::ONE)
            .unwrap(),
        &turn,
    )
    .unwrap();
    assert_eq!(
        big.as_ref().compare(big.as_ref()).unwrap(),
        Some(core::cmp::Ordering::Equal)
    );
    let broad = turn.reprofile(PortableNumericProfile::MAXIMAL).unwrap();
    assert_ne!(turn.encoded(), broad.encoded());
    assert_eq!(turn.identity(), broad.identity());
    assert!(turn.same_physical_definition(&broad));
    let narrow = Q::from_math(MathValue::integer(1), &turn).unwrap();
    let required = narrow.as_ref().physical_profile();
    narrow.as_ref().validate_profile(required).unwrap();
    assert!(required.maximum_quantity_encoded_bytes().unwrap() < 1024);
    let excessive = Q::from_math(MathValue::integer(1), &broad).unwrap();
    assert_eq!(
        excessive.as_ref().validate_profile(required),
        Err(QuantityNumericRefusal::Numeric(
            NumericRefusal::ProfileViolation
        ))
    );
    let kernel = U::new_math_role(
        crate::BUILTIN_KELVIN.family(),
        "K",
        MathValue::ONE,
        MathValue::ZERO,
        PrefixPolicy::NONE,
        R::Point,
    )
    .unwrap();
    let celsius = U::related_math_role(
        &kernel,
        "°C",
        MathValue::ONE,
        MathValue::parse_decimal("273.15").unwrap(),
        PrefixPolicy::NONE,
        R::Point,
    )
    .unwrap();
    let point = Q::from_math(MathValue::integer(21), &celsius).unwrap();
    let delta = Q::from_math_role(MathValue::integer(21), &celsius, R::Delta).unwrap();
    assert_eq!(
        point
            .as_ref()
            .convert(kernel.as_ref())
            .unwrap()
            .coordinate()
            .to_owned()
            .unwrap(),
        MathValue::parse_decimal("294.15").unwrap()
    );
    assert_eq!(
        delta
            .as_ref()
            .convert(kernel.as_ref())
            .unwrap()
            .coordinate()
            .checked_i64()
            .unwrap(),
        21
    );
    assert_eq!(
        point.as_ref().compare(delta.as_ref()),
        Err(QuantityNumericRefusal::IncompatibleRole)
    );
    let mut corrupted = point.encode();
    corrupted[0] = 1;
    assert!(QuantityRef::validate(&corrupted).is_err());
    let mut output = alloc::vec![0;point.encoded().len()];
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    for _ in 0..100 {
        let admitted = QuantityRef::validate(point.encoded()).unwrap();
        assert_eq!(
            admitted.coordinate().checked_microscalar().unwrap(),
            crate::MicroScalar::from_raw_microunits(21_000_000)
        );
        assert!(admitted.unit().exact_scale().proven_positive());
        assert_eq!(
            admitted.compare_borrowed(point.as_ref()).unwrap(),
            Some(core::cmp::Ordering::Equal)
        );
        assert!(
            admitted
                .unit()
                .same_physical_definition(point.as_ref().unit())
        );
        point.encode_into(&mut output).unwrap();
    }
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
}

#[test]
fn retained_storage_counts_owned_carriers_and_dag_capacity() {
    let small = MathValue::integer(7);
    assert_eq!(
        small.owned_storage_bytes(),
        core::mem::size_of::<MathValue>()
    );
    let rational = MathValue::rational("1", "3").unwrap();
    assert_eq!(
        rational.owned_storage_bytes(),
        core::mem::size_of::<MathValue>() + core::mem::size_of::<ExactRational>()
    );
    let symbolic = MathValue::pi().checked_add(&rational).unwrap();
    let MathValue::Symbolic(graph) = &symbolic else {
        panic!("unresolved exact sum");
    };
    let expected = core::mem::size_of::<MathValue>()
        + core::mem::size_of::<SymbolicDag>()
        + graph.nodes.capacity() * core::mem::size_of::<symbolic::Node>()
        + graph
            .nodes
            .iter()
            .map(|node| match node {
                symbolic::Node::Literal(MathValue::Rational(_)) => {
                    core::mem::size_of::<ExactRational>()
                }
                symbolic::Node::Literal(MathValue::SmallInteger(_)) => 0,
                symbolic::Node::Literal(_) => panic!("canonical DAG literals are rational"),
                symbolic::Node::Operation { operands, .. } => {
                    operands.capacity() * core::mem::size_of::<u16>()
                }
            })
            .sum::<usize>();
    assert_eq!(symbolic.owned_storage_bytes(), expected);
    assert!(symbolic.owned_storage_bytes() > symbolic.canonical_bytes().unwrap().len());
}

#[test]
fn borrowed_comparison_preserves_exactness_without_allocating() {
    let rational = MathValue::rational("1", "3")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let decimal = MathValue::parse_decimal("0.33333333333333333333333333333333333333")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let pi = MathValue::pi().canonical_bytes().unwrap();
    let root = MathValue::integer(2)
        .checked_root(2)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let a = MathValueRef::validate(&rational).unwrap();
    let b = MathValueRef::validate(&decimal).unwrap();
    let p = MathValueRef::validate(&pi).unwrap();
    let r = MathValueRef::validate(&root).unwrap();
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    for _ in 0..100 {
        assert_eq!(a.compare(b).unwrap(), Some(core::cmp::Ordering::Greater));
        assert_eq!(p.compare(p).unwrap(), Some(core::cmp::Ordering::Equal));
        assert_eq!(p.compare(r).unwrap(), None);
    }
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
}

#[test]
fn decimal_evidence_requires_exact_termination_and_budget() {
    for (n, d, text) in [
        ("1", "8", "0.125"),
        ("-125", "2", "-62.5"),
        ("1", "1000", "0.001"),
    ] {
        assert_eq!(
            MathValue::rational(n, d)
                .unwrap()
                .terminating_decimal_literal(96)
                .unwrap()
                .as_deref(),
            Some(text)
        );
    }
    assert_eq!(
        MathValue::rational("1", "3")
            .unwrap()
            .terminating_decimal_literal(96)
            .unwrap(),
        None
    );
    assert_eq!(
        MathValue::pi().terminating_decimal_literal(96).unwrap(),
        None
    );
    assert_eq!(
        MathValue::rational("1", "8")
            .unwrap()
            .terminating_decimal_literal(4)
            .unwrap(),
        None
    );
    let quantity =
        crate::Quantity::from_math(MathValue::rational("1", "8").unwrap(), &crate::Unit::Meter)
            .unwrap();
    assert_eq!(quantity.canonical_literal().unwrap(), "0.125m");
}

#[test]
fn explicit_floor_projection_preserves_signed_rational_law() {
    for (text, expected) in [("1.5", 1), ("-1.5", -2), ("-2", -2), ("0.0001", 0)] {
        let value = MathValue::parse_decimal(text).unwrap();
        assert_eq!(value.checked_floor_i64(), Ok(expected));
        let bytes = value.canonical_bytes().unwrap();
        assert_eq!(
            MathValueRef::validate(&bytes).unwrap().checked_floor_i64(),
            Ok(expected)
        );
    }
    assert_eq!(
        MathValue::pi().checked_floor_i64(),
        Err(crate::NumericProjectionRefusal::UnresolvedSymbolic)
    );
    assert_eq!(
        MathValue::integer(10)
            .checked_pow(100)
            .unwrap()
            .checked_floor_i64(),
        Err(crate::NumericProjectionRefusal::Overflow)
    );
}

#[test]
fn borrowed_quantity_integer_projection_checks_full_unsigned_range_without_allocation() {
    let q = crate::Quantity::from_math(
        MathValue::from_integer_decimal("18446744073709551615").unwrap(),
        &crate::Unit::Meter,
    )
    .unwrap();
    let fractional = crate::Quantity::parse_plot_literal("1.5m").unwrap();
    let meter = crate::Unit::Meter;
    let millimeter = crate::Unit::Millimeter;
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    for _ in 0..100 {
        assert_eq!(q.as_ref().project_u64_in_unit(meter.as_ref()), Ok(u64::MAX));
        assert_eq!(
            fractional.as_ref().project_u64_in_unit(meter.as_ref()),
            Err(crate::QuantityConversionRefusal::Inexact)
        );
        assert_eq!(
            fractional.as_ref().project_u64_in_unit(millimeter.as_ref()),
            Ok(1500)
        );
    }
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
}

#[test]
fn borrowed_quantity_writer_preserves_canonical_profiles_without_allocation() {
    let unit = crate::Unit::Meter;
    let expected = crate::Quantity::new(42, unit.clone());
    let profile = expected.as_ref().coordinate_profile();
    let mut output = [0; 2048];
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    for _ in 0..100 {
        let length = crate::Quantity::encode_integer_into(
            42,
            unit.as_ref(),
            crate::QuantityRole::Linear,
            profile,
            &mut output,
        )
        .unwrap();
        assert_eq!(&output[..length], expected.encoded());
        assert!(crate::QuantityRef::validate(&output[..length]).is_ok());
    }
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
    assert!(
        crate::Quantity::encode_integer_into(
            42,
            unit.as_ref(),
            crate::QuantityRole::Point,
            profile,
            &mut output
        )
        .is_err()
    );
    assert!(
        crate::Quantity::encode_integer_into(
            42,
            unit.as_ref(),
            crate::QuantityRole::Linear,
            profile,
            &mut output[..10]
        )
        .is_err()
    );
}

#[test]
fn borrowed_owned_storage_bound_accounts_for_compressed_rational_literals() {
    for value in [
        MathValue::integer(7),
        MathValue::rational("1", "3").unwrap(),
        MathValue::pi()
            .checked_add(&MathValue::rational("1", "3").unwrap())
            .unwrap(),
    ] {
        let wire = value.canonical_bytes().unwrap();
        let borrowed = MathValueRef::validate(&wire).unwrap();
        let decoded = MathValue::decode(&wire).unwrap();
        assert!(borrowed.owned_storage_upper_bound_bytes() >= decoded.owned_storage_bytes());
    }
}

#[test]
fn borrowed_quantity_signed_projection_checks_minimum_and_negative_fractions() {
    let unit = crate::Unit::Meter;
    let minimum = crate::Quantity::new(i64::MIN, unit.clone());
    let negative = crate::Quantity::parse_plot_literal("-1.5m").unwrap();
    let millimeter = crate::Unit::Millimeter;
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    for _ in 0..100 {
        assert_eq!(
            minimum.as_ref().project_i64_in_unit(unit.as_ref()),
            Ok(i64::MIN)
        );
        assert_eq!(
            negative.as_ref().project_i64_in_unit(unit.as_ref()),
            Err(crate::QuantityConversionRefusal::Inexact)
        );
        assert_eq!(
            negative.as_ref().project_i64_in_unit(millimeter.as_ref()),
            Ok(-1500)
        );
    }
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
}

#[test]
fn symbolic_subgraph_cancellation_retains_one_graph_instead_of_all_prefixes() {
    let mut value = MathValue::integer(2).checked_root(2).unwrap();
    for prime in [3, 5, 7, 11, 13, 17, 19, 23] {
        value = value
            .checked_add(&MathValue::integer(prime).checked_root(2).unwrap())
            .unwrap();
    }
    let negative = value.checked_neg().unwrap();
    ALLOCATIONS.with(|n| n.set(0));
    TRACK_ALLOCATIONS.with(|n| n.set(true));
    let result = value.checked_add(&negative);
    TRACK_ALLOCATIONS.with(|n| n.set(false));
    assert_eq!(result, Ok(MathValue::ZERO));
    assert!(
        ALLOCATIONS.with(|n| n.get()) < 64,
        "reachable subgraph is copied once"
    );
}
