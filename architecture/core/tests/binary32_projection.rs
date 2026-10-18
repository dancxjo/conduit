//! Single-round finite projection, independently checked with SoftFloat f128.
use conduit_core::{
    projection::*, Binary32ErrorClaim as Claim, Binary32ProjectionDomain as Domain,
    Binary32ProjectionPolicy as Policy, Binary32Refusal as Refusal, Binary32Rounding as Mode,
    ExactNumericRefusal as Numeric, ExactNumericWork as Work, ExactRational as Q, IeeeF32,
    PreparedBinary32Projection as Prepared, PreparedBinary64Projection,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! { static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) }; }
struct Allocator;
fn count() {
    let _ = ALLOCATIONS.try_with(|c| {
        if let Some(n) = c.get() {
            c.set(Some(n + 1));
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
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(p, layout, size) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn work() -> Work {
    Work::new(100_000_000)
}
fn power(exponent: i16) -> Q {
    Q::from_i128(2)
        .checked_power(exponent, &mut work())
        .unwrap()
}
fn mode(n: u8) -> Mode {
    match n {
        0 => Mode::NearestTiesEven,
        1 => Mode::TowardZero,
        2 => Mode::TowardPositive,
        3 => Mode::TowardNegative,
        _ => panic!("mode"),
    }
}
#[test]
fn binary128_oracle_sources_round_directly_to_binary32_in_all_four_modes() {
    let tolerance = power(128);
    let mut cases = 0;
    for row in include_str!("fixtures/binary32_projection_softfloat.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let fields: Vec<_> = row.split_whitespace().collect();
        let hi = u64::from_str_radix(fields[0], 16).unwrap();
        let lo = u64::from_str_radix(fields[1], 16).unwrap();
        let exponent = ((hi >> 48) & 0x7fff) as i16 - 16383;
        let significand =
            (1u128 << 112) | (u128::from(hi & 0xffff_ffff_ffff) << 64) | u128::from(lo);
        let mut source = Q::from_i128(significand as i128)
            .checked_multiply(power(exponent - 112), &mut work())
            .unwrap();
        if hi >> 63 != 0 {
            source = source.negated();
        }
        let expected = u32::from_str_radix(fields[3], 16).unwrap();
        let flags: u8 = fields[4].parse().unwrap();
        let p = Prepared::prepare(
            source,
            mode(fields[2].parse().unwrap()),
            tolerance,
            &mut work(),
        );
        if exponent > 127 || expected & 0x7f80_0000 == 0x7f80_0000 {
            assert_eq!(p, Err(Refusal::Numeric(Numeric::OutOfRange)), "{row}");
        } else {
            let p = p.unwrap();
            assert_eq!(p.source(), source);
            assert_eq!(p.target().bits(), expected, "{row}");
            assert_eq!(p.inexact(), flags & 1 != 0, "{row}");
            assert!(p.spent_limb_work() <= p.admitted_limb_work());
            assert!(p
                .error()
                .maximum()
                .checked_compare(tolerance, &mut work())
                .unwrap()
                .is_le());
        }
        cases += 1;
    }
    assert_eq!(cases, 4296);
}
#[test]
fn exact_rational_projection_does_not_take_a_double_rounding_detour() {
    let source = Q::ONE
        .checked_add(power(-24), &mut work())
        .unwrap()
        .checked_add(power(-100), &mut work())
        .unwrap();
    let direct = Prepared::prepare(source, Mode::NearestTiesEven, power(-20), &mut work()).unwrap();
    assert_eq!(direct.target().bits(), 0x3f80_0001);
    let intermediate =
        PreparedBinary64Projection::prepare(source, Mode::NearestTiesEven, power(-60), &mut work())
            .unwrap();
    let detour = Prepared::prepare(
        intermediate.represented(),
        Mode::NearestTiesEven,
        power(-20),
        &mut work(),
    )
    .unwrap();
    assert_eq!(detour.target().bits(), 0x3f80_0000);
}
#[test]
fn exact_distance_tolerance_and_work_refusals_are_separate_and_allocation_free() {
    let third = Q::ratio(1, 3, &mut work()).unwrap();
    let tolerance = Q::ratio(1, 10_000_000, &mut work()).unwrap();
    ALLOCATIONS.with(|c| c.set(Some(0)));
    for _ in 0..64 {
        let p = Prepared::prepare(third, Mode::NearestTiesEven, tolerance, &mut work()).unwrap();
        assert_eq!(p.target().bits(), 0x3eaa_aaab);
        assert_eq!(
            p.error(),
            Claim::ExactDistance(Q::ratio(1, 100_663_296, &mut work()).unwrap())
        );
        assert_eq!(
            Prepared::prepare(third, Mode::NearestTiesEven, Q::ZERO, &mut work()),
            Err(Refusal::ToleranceExceeded)
        );
        assert_eq!(
            Prepared::prepare(Q::ONE, Mode::NearestTiesEven, Q::ZERO, &mut Work::new(0)),
            Err(Refusal::Numeric(Numeric::WorkLimit))
        );
    }
    let allocations = ALLOCATIONS.with(|c| c.replace(None).unwrap());
    assert_eq!(allocations, 0);
    assert_eq!(
        Prepared::prepare(third, Mode::NearestTiesEven, Q::from_i128(-1), &mut work()),
        Err(Refusal::NegativeTolerance)
    );
    for bits in [0x7f80_0000, 0xff80_0000, 0x7fc0_0001] {
        assert_eq!(
            Q::from_binary32(IeeeF32::from_bits(bits), &mut work()),
            Err(Numeric::OutOfRange)
        );
    }
    let half = Q::ratio(1, 2, &mut work()).unwrap();
    let exact = Prepared::prepare(half, Mode::NearestTiesEven, Q::ZERO, &mut work()).unwrap();
    assert_eq!(exact.error(), Claim::ExactDistance(Q::ZERO));
    assert!(!exact.inexact());
}
#[test]
fn the_general_loss_domain_rejects_false_exactness_target_and_policy() {
    let third = Q::ratio(1, 3, &mut work()).unwrap();
    let p = Prepared::prepare(third, Mode::NearestTiesEven, Q::ONE, &mut work()).unwrap();
    let facts = p.projection_facts();
    assert!(Domain.validate(
        &p,
        Some(&p.target()),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    assert!(Policy {
        maximum_error: Q::ONE,
        rounding: Mode::NearestTiesEven
    }
    .permits(&p, &p.target(), &facts));
    assert!(!Policy {
        maximum_error: Q::ZERO,
        rounding: Mode::NearestTiesEven
    }
    .permits(&p, &p.target(), &facts));
    assert!(!Domain.validate(
        &p,
        Some(&IeeeF32::from_bits(0)),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    let false_exact = [
        ProjectionFact::Preserved {
            obligation: ProjectionText::new("mathematical-denotation").unwrap(),
        },
        ProjectionFact::Transformed {
            obligation: ProjectionText::new("numeric-representation").unwrap(),
            law: ProjectionText::new("finite-binary32-single-round@1").unwrap(),
        },
    ];
    assert!(!Domain.validate(
        &p,
        Some(&p.target()),
        &false_exact,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
}
