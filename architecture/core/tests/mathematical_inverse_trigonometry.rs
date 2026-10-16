use conduit_core::*;
use core::cmp::Ordering;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn denote(
    input: ExactRational,
    operation: fn(MathematicalNodeId) -> MathematicalNode,
) -> Result<MathematicalValue, MathematicalValueRefusal> {
    let mut b = MathematicalValueBuilder::new();
    let a = b.insert(MathematicalNode::Rational(input), &mut work())?;
    let root = b.insert(operation(a), &mut work())?;
    b.finish(root, &mut work())
}
fn requirements() -> MathematicalEnclosureRequirements {
    MathematicalEnclosureRequirements {
        maximum_width: q(1, 1_000_000),
        maximum_leaf_width: q(1, 100_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
#[test]
fn exact_inverse_nodes_round_trip_and_project_without_changing_source() {
    for operation in [
        MathematicalNode::ArctangentTurns,
        MathematicalNode::ArcsineTurns,
        MathematicalNode::ArccosineTurns,
    ] {
        let source = denote(q(3, 5), operation).unwrap();
        assert_eq!(source.as_rational(), None);
        let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let len = source.encode(&mut bytes).unwrap();
        bytes.truncate(len);
        let mut validation = ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
        let view = MathematicalValueView::decode(&bytes, &mut validation).unwrap();
        assert_eq!(view.node_count(), 2);
        assert_eq!(view.semantic_digest(), source.semantic_digest());
        assert_eq!(
            MathematicalValue::decode(&bytes, &mut work()).unwrap(),
            source
        );
        let projected =
            PreparedMathematicalEnclosure::prepare(&source, requirements(), &mut work()).unwrap();
        assert_eq!(projected.source(), &source);
        assert_ne!(
            projected
                .width()
                .checked_compare(requirements().maximum_width, &mut work())
                .unwrap(),
            Ordering::Greater
        );
        assert!(projected.maximum_used_constant_terms() > 0);
        let mut exact = requirements();
        exact.maximum_width = ExactRational::ZERO;
        exact.maximum_leaf_width = ExactRational::ZERO;
        assert!(PreparedMathematicalEnclosure::prepare(&source, exact, &mut work()).is_err());
    }
}
#[test]
fn principal_identities_and_domain_proof_are_shared_by_builder_and_decoder() {
    for (operation, input, output) in [
        (
            MathematicalNode::ArctangentTurns as fn(_) -> _,
            q(1, 1),
            q(1, 8),
        ),
        (MathematicalNode::ArcsineTurns, q(-1, 2), q(-1, 12)),
        (MathematicalNode::ArccosineTurns, q(-1, 2), q(1, 3)),
    ] {
        assert_eq!(
            denote(input, operation).unwrap(),
            MathematicalValue::rational(output)
        );
    }
    for operation in [
        MathematicalNode::ArcsineTurns,
        MathematicalNode::ArccosineTurns,
    ] {
        assert_eq!(
            denote(q(2, 1), operation),
            Err(MathematicalValueRefusal::InverseTrigonometricDomain)
        );
        let mut b = MathematicalValueBuilder::new();
        let pi = b.insert(MathematicalNode::Pi, &mut work()).unwrap();
        assert_eq!(
            b.insert(operation(pi), &mut work()),
            Err(MathematicalValueRefusal::UnsupportedDomainProof)
        );
    }
    for (argument, tag, expected) in [
        (
            q(2, 1),
            13,
            MathematicalValueRefusal::InverseTrigonometricDomain,
        ),
        (q(1, 2), 13, MathematicalValueRefusal::NonCanonicalEncoding),
        (q(1, 1), 12, MathematicalValueRefusal::NonCanonicalEncoding),
    ] {
        let mut rational = vec![0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let n = argument.encode(&mut rational).unwrap();
        let mut encoded = vec![1, 1, 2, 0];
        encoded.extend_from_slice(&(n as u16).to_le_bytes());
        encoded.extend_from_slice(&rational[..n]);
        encoded.extend_from_slice(&[tag, 0]);
        assert_eq!(
            MathematicalValueView::decode(&encoded, &mut work()).unwrap_err(),
            expected
        );
    }
}
