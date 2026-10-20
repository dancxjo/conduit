//! Independently prepared/replayed physical Source receipts; no installed Plan/Play claim.
use super::*;
use sha2::{Digest, Sha256};
fn input<'a, 's>(
    s: &'a QualifiedQuantityBinary64Result<'s>,
    t: &'a QualifiedQuantityBinary64Target,
    f: &'a [ProjectionFact<'a, QualifiedQuantityBinary64Domain<'s>>],
) -> ProjectionInput<'a, QualifiedQuantityBinary64Domain<'s>> {
    let text = |s| ProjectionText::new(s).unwrap();
    ProjectionInput {
        basis: ProjectionBasis {
            report: text("fixture/source-result"),
            source: text("fixture/checked-program-and-input"),
            target: Some(text("fixture/physical-output")),
            projector: text("fixture/selected-binary64"),
            requested_route: text("fixture/qualified-computation"),
            boundary: None,
        },
        source: s,
        target: Some(t),
        facts: f,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
#[test]
fn original_program_and_incoming_frame_bind_independently_replayed_shared_evidence() {
    let p = program("0.2Hz", "(. + 10000000000000000Hz) - 10000000000000000Hz");
    let canonical = p.canonical_bytes().unwrap();
    let supplier = incoming("0.1Hz", "0.001Hz", NearQuantityAlgorithm::Binary32);
    let frame = supplier.state.encoding.encoded();
    let mut producer = prepare(&p);
    let mut consumer = prepare(&p);
    let policy = consumer.policy();
    let codec = QualifiedQuantityBinary64EvidenceCodec::new(policy);
    let result = producer.execute(frame, &mut work()).unwrap();
    let replay = consumer.execute(frame, &mut work()).unwrap();
    assert_eq!(result.canonical_program(), canonical);
    assert_eq!(result.incoming_evidence(), frame);
    assert_eq!(
        result.incoming_evidence_digest(),
        <[u8; 32]>::from(Sha256::digest(frame))
    );
    let d = QualifiedQuantityBinary64Domain::new();
    let target = result.target();
    let f = result.projection_facts();
    let r = ProjectionReport::new(&d, &policy, input(&result, &target, &f)).unwrap();
    let target2 = replay.target();
    let g = replay.projection_facts();
    let r2 = ProjectionReport::new(&d, &policy, input(&replay, &target2, &g)).unwrap();
    let mut bytes = vec![0; QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS.total_bytes];
    let guard = crate::allocation_probe::begin();
    let n = encode_projection_evidence(
        &r,
        &codec,
        QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
        &mut bytes,
    )
    .unwrap();
    verify_projection_evidence(
        &r2,
        &codec,
        QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
        &bytes[..n],
    )
    .unwrap();
    assert_eq!(
        r2.require_bound_policy(policy.identity(), policy.configuration_digest().unwrap()),
        Ok(&target2)
    );
    assert!(r2.require_exact().is_err());
    assert_eq!(guard.finish(), 0);
    let view = inspect_projection_evidence(&bytes[..n]).unwrap();
    assert_eq!(view.source_bytes[0], 1);
    assert_eq!(&view.source_bytes[1..33], &producer_digest(&canonical));
    assert_eq!(
        u64::from_le_bytes(view.source_bytes[33..41].try_into().unwrap()),
        canonical.len() as u64
    );
    assert_eq!(
        &view.source_bytes[41..73],
        &result.incoming_evidence_digest()
    );
    assert_eq!(
        u64::from_le_bytes(view.source_bytes[73..81].try_into().unwrap()),
        frame.len() as u64
    );
    assert_eq!(view.facts().count(), 4);
    for offset in [1usize, 40, n / 2, n - 1] {
        bytes[offset] ^= 1;
        assert!(verify_projection_evidence(
            &r2,
            &codec,
            QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
            &bytes[..n]
        )
        .is_err());
        bytes[offset] ^= 1;
    }
    let mut short = vec![0x55; n - 1];
    assert_eq!(
        encode_projection_evidence(
            &r,
            &codec,
            QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
            &mut short
        ),
        Err(ProjectionEvidenceRefusal::BufferTooSmall)
    );
    assert!(short.iter().all(|b| *b == 0x55));
}
fn producer_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
#[test]
fn same_denotation_and_target_do_not_erase_incoming_or_program_custody() {
    let body = "(. + 10000000000000000Hz) - 10000000000000000Hz";
    let p = program("0.2Hz", body);
    let a = incoming("0.1Hz", "0.001Hz", NearQuantityAlgorithm::Binary32);
    let b = incoming("0.1Hz", "0.001Hz", NearQuantityAlgorithm::ExactRational);
    let mut one = prepare(&p);
    let mut two = prepare(&p);
    let policy = one.policy();
    let c = QualifiedQuantityBinary64EvidenceCodec::new(policy);
    let x = one
        .execute(a.state.encoding.encoded(), &mut work())
        .unwrap();
    let y = two
        .execute(b.state.encoding.encoded(), &mut work())
        .unwrap();
    assert_eq!(x.source(), y.source());
    assert_eq!(x.target(), y.target());
    assert_ne!(x.incoming_evidence_digest(), y.incoming_evidence_digest());
    let d = QualifiedQuantityBinary64Domain::new();
    let t = x.target();
    let f = x.projection_facts();
    let r = ProjectionReport::new(&d, &policy, input(&x, &t, &f)).unwrap();
    let u = y.target();
    let g = y.projection_facts();
    let s = ProjectionReport::new(&d, &policy, input(&y, &u, &g)).unwrap();
    let mut bytes = vec![0; QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS.total_bytes];
    let n = encode_projection_evidence(
        &r,
        &c,
        QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
        &mut bytes,
    )
    .unwrap();
    assert!(verify_projection_evidence(
        &s,
        &c,
        QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS,
        &bytes[..n]
    )
    .is_err());
    let changed = prepare(&program("0.3Hz", body)).policy();
    assert_ne!(
        policy.configuration_digest(),
        changed.configuration_digest()
    );
    assert_eq!(
        projection_evidence_len(
            &r,
            &QualifiedQuantityBinary64EvidenceCodec::new(changed),
            QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS
        ),
        Err(ProjectionEvidenceRefusal::PolicyConfigurationMismatch)
    );
    let omitted = &f[..3];
    assert!(ProjectionReport::new(&d, &policy, input(&x, &t, omitted)).is_err());
}

#[test]
fn bounded_consumer_api_checks_artifacts_basis_work_and_transport_before_returning_a_target() {
    use QualifiedQuantityBinary64EvidenceRefusal as E;
    let p = program("0.2Hz", "(. + 10000000000000000Hz) - 10000000000000000Hz");
    let supplier = incoming("0.1Hz", "0.001Hz", NearQuantityAlgorithm::Binary32);
    let frame = supplier.state.encoding.encoded();
    let mut producer = prepare(&p);
    let mut consumer = prepare(&p);
    let result = producer.execute(frame, &mut work()).unwrap();
    let target = result.target();
    let facts = result.projection_facts();
    let basis = input(&result, &target, &facts).basis;
    let mut bytes = vec![0; QUALIFIED_QUANTITY_BINARY64_EVIDENCE_LIMITS.total_bytes];
    let oversized = vec![0; bytes.len() + 1];
    let mut shortened = vec![0xAA; 1];
    let guard = crate::allocation_probe::begin();
    let n = result.encode_evidence(basis, &mut bytes).unwrap();
    assert_eq!(
        consumer.verify_evidence(frame, basis, &bytes[..n], &mut work()),
        Ok(target)
    );
    assert_eq!(
        result.encode_evidence(basis, &mut shortened),
        Err(E::Transport(ProjectionEvidenceRefusal::BufferTooSmall))
    );
    assert_eq!(shortened[0], 0xAA);
    let mut unused = work();
    assert_eq!(
        consumer.verify_evidence(frame, basis, &oversized, &mut unused),
        Err(E::Transport(ProjectionEvidenceRefusal::Extent))
    );
    assert_eq!(unused.spent(), 0);
    let mut changed = basis;
    changed.source = ProjectionText::new("fixture/different-source").unwrap();
    let mut charged = work();
    assert!(matches!(
        consumer.verify_evidence(frame, changed, &bytes[..n], &mut charged),
        Err(E::Transport(ProjectionEvidenceRefusal::EvidenceMismatch))
    ));
    assert!(charged.spent() > 0);
    bytes[n - 1] ^= 1;
    assert!(matches!(
        consumer.verify_evidence(frame, basis, &bytes[..n], &mut work()),
        Err(E::Transport(ProjectionEvidenceRefusal::EvidenceMismatch))
    ));
    bytes[n - 1] ^= 1;
    let mut exhausted = ExactNumericWork::new(1);
    assert!(matches!(
        consumer.verify_evidence(frame, basis, &bytes[..n], &mut exhausted),
        Err(E::Computation(R::Evidence(
            NearQuantityEvidenceRefusal::WorkLimit
        )))
    ));
    assert_eq!(exhausted.spent(), 1);
    assert!(matches!(
        consumer.verify_evidence(&frame[..frame.len() - 1], basis, &bytes[..n], &mut work()),
        Err(E::Computation(R::Evidence(_)))
    ));
    assert_eq!(
        consumer.verify_evidence(frame, basis, &bytes[..n], &mut work()),
        Ok(target)
    );
    assert_eq!(guard.finish(), 0);
}
