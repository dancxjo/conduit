//! Prepared construction retains the accepted canonical denotation protocol.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
type R = MathematicalValueRefusal;
type Node = MathematicalNode;
type Id = MathematicalNodeId;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
trait Insert {
    fn add(&mut self, node: Node, work: &mut ExactNumericWork) -> Result<Id, R>;
}
impl Insert for MathematicalValueBuilder {
    fn add(&mut self, node: Node, w: &mut ExactNumericWork) -> Result<Id, R> {
        self.insert(node, w)
    }
}
impl Insert for PreparedMathematicalValueBuilder {
    fn add(&mut self, node: Node, w: &mut ExactNumericWork) -> Result<Id, R> {
        self.insert(node, w)
    }
}
fn specimen(b: &mut impl Insert, which: u8, w: &mut ExactNumericWork) -> Result<Id, R> {
    Ok(match which {
        0 => {
            let pi = b.add(Node::Pi, w)?;
            let n = b.add(Node::Rational(ExactRational::from_i128(180)), w)?;
            b.add(Node::Divide(pi, n), w)?
        }
        1 => {
            let n = b.add(Node::Rational(ExactRational::from_i128(2)), w)?;
            b.add(Node::PrincipalSquareRoot(n), w)?
        }
        2 => {
            let pi = specimen(b, 0, w)?;
            let root = specimen(b, 1, w)?;
            b.add(Node::Add(pi, root), w)?
        }
        3 => {
            let a = b.add(
                Node::Rational(ExactRational::from_i128(9_007_199_254_740_993)),
                w,
            )?;
            let b_id = b.add(
                Node::Rational(ExactRational::from_i128(9_007_199_254_740_992)),
                w,
            )?;
            b.add(Node::Subtract(a, b_id), w)?
        }
        4 => b.add(Node::Tau, w)?,
        5 => {
            let quarter = ExactRational::ratio(1, 4, w)?;
            let angle = b.add(Node::Rational(quarter), w)?;
            b.add(Node::SineTurns(angle), w)?
        }
        _ => unreachable!(),
    })
}
fn reference(which: u8) -> Vec<u8> {
    let mut b = MathematicalValueBuilder::new();
    let mut w = work();
    let root = specimen(&mut b, which, &mut w).unwrap();
    let value = b.finish(root, &mut w).unwrap();
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
fn ready() -> PreparedMathematicalValueBuilder {
    PreparedMathematicalValueBuilder::new(1_000_000, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES).unwrap()
}

#[test]
fn repeated_runtime_construction_matches_existing_canonical_bytes_without_growth() {
    let (mut b, preparation_allocations) = observe(ready);
    let retained = b.retained_payload_bytes();
    println!("PREPARED_MATH_DATA preparation_allocations={preparation_allocations} retained_payload_bytes={retained} maximum_encoded_bytes={MATHEMATICAL_VALUE_MAX_ENCODED_BYTES}");
    let mut out = vec![0xA5; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    for which in 0..6 {
        let expected = reference(which);
        let (correct, allocations) = observe(|| {
            (0..16).all(|_| {
                b.reset();
                out.fill(0xA5);
                let mut w = work();
                let Ok(root) = specimen(&mut b, which, &mut w) else {
                    return false;
                };
                let Ok(n) = b.encode(root, &mut out, &mut w) else {
                    return false;
                };
                n == expected.len()
                    && out[..n] == expected
                    && out[n..].iter().all(|&v| v == 0xA5)
                    && MathematicalValueView::decode(&out[..n], &mut w).is_ok()
                    && w.spent() > 0
                    && w.spent() <= 100_000_000
            })
        });
        assert!(correct, "specimen {which}");
        assert_eq!(allocations, 0);
        assert_eq!(b.retained_payload_bytes(), retained);
    }
    // Denotation normalizes this rational expression to one; it does not claim
    // to record the binary64 algorithm that rounds the same cancellation to zero.
    let bytes = reference(3);
    assert_eq!(
        MathematicalValue::decode(&bytes, &mut work())
            .unwrap()
            .as_rational(),
        Some(ExactRational::ONE)
    );
}

#[test]
fn preparation_limits_account_for_all_buffers_before_construction() {
    for maximum in [0, 3, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES + 1] {
        assert!(matches!(
            PreparedMathematicalValueBuilder::new(1_000_000, maximum),
            Err(R::Numeric(ExactNumericRefusal::ExtentExceeded))
        ));
    }
    let required = ready().retained_payload_bytes() as u32;
    assert!(
        PreparedMathematicalValueBuilder::new(required, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES)
            .is_ok()
    );
    assert!(matches!(
        PreparedMathematicalValueBuilder::new(required - 1, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES),
        Err(R::Numeric(ExactNumericRefusal::ExtentExceeded))
    ));
}

#[test]
fn short_buffers_invalid_roots_and_work_refusals_leave_output_untouched_then_recover() {
    let mut b = ready();
    let root = b.insert(Node::Pi, &mut work()).unwrap();
    let mut out = [0xA5; 64];
    let mut other = MathematicalValueBuilder::new();
    let foreign = other
        .insert(Node::Rational(ExactRational::from_i128(5)), &mut work())
        .unwrap();
    let (correct, allocations) = observe(|| {
        matches!(
            b.encode(root, &mut out[..3], &mut work()),
            Err(R::Numeric(ExactNumericRefusal::EncodingBufferTooSmall))
        ) && out == [0xA5; 64]
            && matches!(
                b.encode(foreign, &mut out, &mut work()),
                Err(R::InvalidReference)
            )
            && out == [0xA5; 64]
            && matches!(
                b.encode(root, &mut out, &mut ExactNumericWork::new(0)),
                Err(R::Numeric(ExactNumericRefusal::WorkLimit))
            )
            && out == [0xA5; 64]
            && b.encode(root, &mut out, &mut work())
                .is_ok_and(|n| n == 4 && out[..n] == [1, 1, 1, 1])
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}

#[test]
fn internal_encoding_budget_refuses_without_partial_output_and_can_emit_a_smaller_value() {
    let mut b = PreparedMathematicalValueBuilder::new(1_000_000, 4).unwrap();
    let mut out = [0xA5; 64];
    let (correct, allocations) = observe(|| {
        let root = specimen(&mut b, 1, &mut work()).unwrap();
        let refused = matches!(
            b.encode(root, &mut out, &mut work()),
            Err(R::Numeric(ExactNumericRefusal::EncodingBufferTooSmall))
        ) && out == [0xA5; 64];
        b.reset();
        let root = b.insert(Node::Pi, &mut work()).unwrap();
        refused && b.encode(root, &mut out, &mut work()).is_ok_and(|n| n == 4)
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}

#[test]
fn node_and_depth_limits_refuse_without_growth_and_reset_recovers() {
    let mut b = ready();
    let mut out = [0xA5; 64];
    let (correct, allocations) = observe(|| {
        let mut w = work();
        for n in 0..31 {
            b.insert(Node::Rational(ExactRational::from_i128(n)), &mut w)
                .unwrap();
        }
        let full = matches!(b.insert(Node::Tau, &mut w), Err(R::NodeBound));
        b.reset();
        let mut root = b.insert(Node::Pi, &mut w).unwrap();
        for _ in 1..MATHEMATICAL_VALUE_MAX_DEPTH {
            root = b.insert(Node::Negate(root), &mut w).unwrap();
        }
        let deep = matches!(b.insert(Node::Negate(root), &mut w), Err(R::DepthBound));
        b.reset();
        let invalidated = matches!(b.encode(root, &mut out, &mut w), Err(R::InvalidReference))
            && out == [0xA5; 64];
        let root = b.insert(Node::Pi, &mut w).unwrap();
        full && deep && invalidated && b.encode(root, &mut out, &mut w).is_ok_and(|n| n == 4)
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}
