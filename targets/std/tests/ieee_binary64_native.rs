//! Component proof; production factory/Source availability is a separate boundary.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../src/numerical_host/ieee_native.rs"]
mod native;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;

const MODES: [Binary64Rounding; 4] = [
    Binary64Rounding::NearestTiesEven,
    Binary64Rounding::TowardZero,
    Binary64Rounding::TowardPositive,
    Binary64Rounding::TowardNegative,
];
const OPS: [IeeeBinary64Operation; 4] = [
    IeeeBinary64Operation::Add,
    IeeeBinary64Operation::Subtract,
    IeeeBinary64Operation::Multiply,
    IeeeBinary64Operation::Divide,
];

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[test]
fn native_results_and_flags_match_6400_independent_cases_without_allocating() {
    assert!(native::available());
    let cases: Vec<_> =
        include_str!("../../../architecture/core/tests/fixtures/ieee_binary64_softfloat.tsv")
            .lines()
            .filter(|line| line.starts_with("a "))
            .map(|line| {
                let fields: Vec<_> = line.split_ascii_whitespace().collect();
                (
                    fields[1].parse::<usize>().unwrap(),
                    fields[2].parse::<usize>().unwrap(),
                    IeeeF64::from_bits(u64::from_str_radix(fields[3], 16).unwrap()),
                    IeeeF64::from_bits(u64::from_str_radix(fields[4], 16).unwrap()),
                    u64::from_str_radix(fields[5], 16).unwrap(),
                    fields[6].parse::<u8>().unwrap(),
                )
            })
            .collect();
    assert_eq!(cases.len(), 6400);
    let ((), count) = allocation_probe::observe(|| {
        for (index, (operation, rounding, left, right, bits, flags)) in cases.iter().enumerate() {
            let result = native::execute(OPS[*operation], *left, *right, MODES[*rounding]).unwrap();
            assert_eq!(
                (result.value.bits(), result.flags.bits()),
                (*bits, *flags),
                "oracle case={index}"
            );
        }
    });
    assert_eq!((count.allocations, count.reallocations), (0, 0));
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[test]
fn scoped_instruction_restores_control_and_sticky_state_before_rust_resumes() {
    // All flags set, masked exceptions, toward-zero and FTZ. DAZ remains zero
    // because its availability is optional on earlier SSE2 implementations.
    let injected = 0xffbf;
    for mode in MODES {
        let (result, before, after) = native::preservation_probe(mode, injected);
        assert_eq!(before, after);
        let reference = evaluate_ieee_binary64(
            IeeeBinary64Operation::Divide,
            IeeeF64::from_bits(0x3ff0_0000_0000_0000),
            IeeeF64::from_bits(0x4008_0000_0000_0000),
            mode,
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
        assert_eq!(result, reference);
    }
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
#[test]
fn unsupported_target_has_no_native_availability_or_implicit_reference_fallback() {
    assert!(!native::available());
    assert_eq!(
        native::execute(
            OPS[0],
            IeeeF64::from_bits(0),
            IeeeF64::from_bits(0),
            MODES[0]
        ),
        Err(native::NativeIeeeRefusal::UnsupportedTarget)
    );
}
