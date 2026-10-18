//! Optional rational wire form; not yet an installed Source/Back profile.
use conduit_core::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static ALLOCATIONS: Cell<Option<usize>> = const {Cell::new(None)}; }
struct Allocator;
fn count() {
    let _ = ALLOCATIONS.try_with(|counter| {
        if let Some(n) = counter.get() {
            counter.set(Some(n + 1));
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn encode(value: RationalQuantity) -> Vec<u8> {
    let mut bytes = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let length = encode_rational_quantity_value(value, &mut bytes).unwrap();
    bytes.truncate(length);
    bytes
}
#[test]
fn exact_thirds_survive_independent_wire_values_without_changing_unit_family_or_role() {
    let source = Quantity::parse_plot_literal("1m").unwrap();
    let native = source.encode();
    let mut budget = work();
    let original = QuantityValueView::decode(&native, &mut budget).unwrap();
    assert_eq!(original.decimal(), Some(source));
    assert_eq!(original.encoded(), native);
    let original_rational = original.rational(&mut budget).unwrap();
    let rational = RationalQuantity::new(
        original_rational
            .coordinate()
            .checked_divide(ExactRational::from_i128(3), &mut budget)
            .unwrap(),
        original_rational.unit(),
        original_rational.role(),
    )
    .unwrap();
    let bytes = encode(rational);
    let third = QuantityValueView::decode(&bytes, &mut budget).unwrap();
    assert_eq!(
        third.representation(),
        QuantityCoordinateRepresentation::Rational
    );
    assert_eq!(
        third.rational(&mut budget).unwrap().coordinate(),
        ExactRational::ratio(1, 3, &mut budget).unwrap()
    );
    assert_eq!(
        (
            third.unit(),
            third.dimension(),
            third.family(),
            third.role()
        ),
        (
            source.unit(),
            source.dimension(),
            source.family(),
            source.role()
        )
    );
    assert!(third.decimal().is_none());
    assert_eq!(
        third.project_decimal(&mut budget),
        Err(QuantityValueRefusal::Rational(
            RationalQuantityRefusal::Numeric(ExactNumericRefusal::InexactProjection)
        ))
    );
    let third_rational = third.rational(&mut budget).unwrap();
    let restored = RationalQuantity::new(
        third_rational
            .coordinate()
            .checked_multiply(ExactRational::from_i128(3), &mut budget)
            .unwrap(),
        third_rational.unit(),
        third_rational.role(),
    )
    .unwrap();
    let restored_bytes = encode(restored);
    let restored = QuantityValueView::decode(&restored_bytes, &mut budget).unwrap();
    assert_eq!(
        restored.compare(original, &mut budget).unwrap(),
        core::cmp::Ordering::Equal
    );
    assert_eq!(restored.project_decimal(&mut budget).unwrap(), source);
    assert_ne!(
        restored.representation_digest(),
        original.representation_digest()
    );
    // The old decimal carrier and decoder keep their exact encoding contract.
    assert_eq!(source.encode(), native);
    assert!(Quantity::decode(&bytes).is_err());
}
#[test]
fn affine_points_differences_prefixes_and_bad_roles_keep_existing_physical_laws() {
    let mut budget = work();
    for (source, target) in [
        ("1°F", "°C"),
        ("TemperatureDelta(1, °F)", "°C"),
        ("0.25turn", "°"),
        ("1KiB", "B"),
    ] {
        let source = Quantity::parse_plot_literal(source).unwrap();
        let value = RationalQuantity::from_quantity(source, &mut budget)
            .unwrap()
            .convert_to_unit(Unit::resolve(target).unwrap(), &mut budget)
            .unwrap();
        let bytes = encode(value);
        let view = QuantityValueView::decode(&bytes, &mut budget).unwrap();
        assert_eq!(view.role(), source.role());
        assert_eq!(view.family(), source.family());
        assert_eq!(view.dimension(), source.dimension());
        assert_eq!(
            view.rational(&mut budget)
                .unwrap()
                .convert_to_unit(source.unit(), &mut budget)
                .unwrap(),
            RationalQuantity::from_quantity(source, &mut budget).unwrap()
        );
    }
    let point = encode(
        RationalQuantity::from_quantity(Quantity::parse_plot_literal("1°F").unwrap(), &mut budget)
            .unwrap(),
    );
    let delta = encode(
        RationalQuantity::from_quantity(
            Quantity::parse_plot_literal("TemperatureDelta(1, °F)").unwrap(),
            &mut budget,
        )
        .unwrap(),
    );
    assert_eq!(
        QuantityValueView::decode(&point, &mut budget)
            .unwrap()
            .compare(
                QuantityValueView::decode(&delta, &mut budget).unwrap(),
                &mut budget
            ),
        Err(QuantityValueRefusal::Rational(
            RationalQuantityRefusal::Physical(QuantityConversionRefusal::IncompatibleQuantityRoles)
        ))
    );
    let mut invalid = encode(
        RationalQuantity::from_quantity(Quantity::parse_plot_literal("1m").unwrap(), &mut budget)
            .unwrap(),
    );
    invalid[1] = 1;
    assert!(QuantityValueView::decode(&invalid, &mut budget).is_err());
}
#[test]
fn malformed_truncated_noncanonical_and_exhausted_work_refuse() {
    let value = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work()).unwrap(),
        Unit::resolve("m").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let bytes = encode(value);
    for length in 0..bytes.len() {
        assert!(QuantityValueView::decode(&bytes[..length], &mut work()).is_err());
    }
    let mut changed = bytes.clone();
    changed[0] = 9;
    assert!(matches!(
        QuantityValueView::decode(&changed, &mut work()),
        Err(QuantityValueRefusal::UnsupportedVersion(9))
    ));
    let mut changed = bytes.clone();
    changed.push(0);
    assert!(QuantityValueView::decode(&changed, &mut work()).is_err());
    let mut changed = bytes.clone();
    changed[2] ^= 1;
    assert!(QuantityValueView::decode(&changed, &mut work()).is_err());
    assert!(matches!(
        QuantityValueView::decode(&bytes, &mut ExactNumericWork::new(0)),
        Err(QuantityValueRefusal::Rational(
            RationalQuantityRefusal::Numeric(ExactNumericRefusal::WorkLimit)
        ))
    ));
    assert!(encode_rational_quantity_value(value, &mut [0; 1]).is_err());
}

#[test]
fn wide_coordinates_and_native_decode_use_admitted_storage_without_allocating() {
    let mut budget = work();
    let huge =
        ExactRational::parse_decimal(&format!("1{}1", "0".repeat(500)), &mut budget).unwrap();
    let coordinate = huge
        .checked_divide(ExactRational::from_i128(3), &mut budget)
        .unwrap();
    let value = RationalQuantity::new(
        coordinate,
        Unit::resolve("m").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut storage = [0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let native = Quantity::parse_plot_literal("3.2m").unwrap().encode();
    ALLOCATIONS.with(|counter| counter.set(Some(0)));
    let length = encode_rational_quantity_value(value, &mut storage).unwrap();
    assert!(length > QUANTITY_ENCODED_LEN);
    let decoded = QuantityValueView::decode(&storage[..length], &mut budget).unwrap();
    assert_eq!(decoded.rational(&mut budget).unwrap(), value);
    let mut native_work = ExactNumericWork::new(1);
    let native_view = QuantityValueView::decode(&native, &mut native_work).unwrap();
    assert_eq!(native_work.spent(), 1);
    assert!(native_view.decimal().is_some());
    let allocations = ALLOCATIONS.with(|counter| counter.replace(None)).unwrap();
    assert_eq!(allocations, 0);
    assert_eq!(
        QuantityValueView::decode(&vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES + 1], &mut work())
            .unwrap_err(),
        QuantityValueRefusal::ExtentExceeded
    );
}
