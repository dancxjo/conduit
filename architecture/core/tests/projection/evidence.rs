//! Complete general-report transport, with explicit domain encodings.
use super::common::*;
use conduit_core::{projection::*, *};
#[path = "../common/numerical_allocation.rs"]
mod allocation_probe;

#[path = "evidence/support.rs"]
mod support;
use support::*;

#[test]
fn complete_inventory_scores_attempts_and_unknown_native_bytes_survive_canonical_verification() {
    let domain = Bits;
    let policy = Policy(1);
    let codec = Codec(1);
    let source = 7;
    let target = 3;
    let detail = 2;
    let facts = [
        ProjectionFact::Preserved {
            obligation: text("0"),
        },
        ProjectionFact::Transformed {
            obligation: text("1"),
            law: text("identity-layout@1"),
        },
        ProjectionFact::Lost {
            obligation: text("2"),
            class: ProjectionLoss::Unrecognized,
            detail: &detail,
            native_fact: Some(text("foreign")),
        },
    ];
    let native = [ProjectionNativeFact {
        identity: text("foreign"),
        contract: text("unknown-cost-contract@1"),
        provider: text("provider@1"),
        encoding: text("opaque-foreign@1"),
        bytes: &[0xff, 0, 0x80, 7],
    }];
    let original = calibrated("cost@1", "calibration/native@1", 17);
    let projected = calibrated("inverse-cost@1", "calibration/projected@1", 83);
    let scores = [
        ProjectionScore::Native {
            fact: text("foreign"),
            score: original,
        },
        ProjectionScore::Normalized {
            fact: text("foreign"),
            original,
            projected,
            policy: text("invert@1"),
        },
    ];
    let plan = PlanId::from("plan/1");
    let host = HostId::from("host/1");
    let boot = BootId::from("boot/1");
    let back = ImplementationId::from("back/1");
    let other = ImplementationId::from("back/2");
    let extra = ImplementationId::from("back/3");
    let artifact = ArtifactId::from("artifact/1");
    let a = ProjectionRealization {
        plan: &plan,
        host: &host,
        boot: &boot,
        back: &back,
        artifact: &artifact,
    };
    let b = ProjectionRealization { back: &other, ..a };
    let admitted = [a, b];
    let failure = TerminalInfo::new(TerminalCategory::ProviderOrResourceLoss, [42; 32]);
    let attempts = [
        ProjectionAttempt {
            realization: a,
            outcome: ProjectionMechanism::Failed(failure),
            fallback_reason: None,
        },
        ProjectionAttempt {
            realization: b,
            outcome: ProjectionMechanism::Completed,
            fallback_reason: Some(text("resource-alternative@1")),
        },
    ];
    let expanded = ExpandedPlotId::from("expanded/1");
    let other_expanded = ExpandedPlotId::from("expanded/2");
    let diagnostics = [text("opaque-retained@1"), text("explicit-alternative@1")];
    let make = || {
        let mut value = input(&source, Some(&target), &facts);
        value.native = &native;
        value.scores = &scores;
        value.admitted = &admitted;
        value.attempts = &attempts;
        value.selected_attempt = Some(1);
        value.diagnostics = &diagnostics;
        value.basis.boundary = Some(ProjectionBoundary {
            expanded_form: &expanded,
            subject: text("port/result"),
        });
        value
    };
    let report = ProjectionReport::new(&domain, &policy, make()).unwrap();
    let mut wire = [0; 4096];
    let (length, allocations) = allocation_probe::observe(|| {
        let n = encode_projection_evidence(&report, &codec, limits(), &mut wire).unwrap();
        assert_eq!(
            n,
            projection_evidence_len(&report, &codec, limits()).unwrap()
        );
        verify_projection_evidence(&report, &codec, limits(), &wire[..n]).unwrap();
        // Every field and payload byte is compared, not only a report summary.
        for i in 0..n {
            wire[i] ^= 1;
            assert!(verify_projection_evidence(&report, &codec, limits(), &wire[..n]).is_err());
            wire[i] ^= 1;
        }
        n
    });
    assert_eq!(allocations, 0);
    assert!(wire[..length]
        .windows(native[0].bytes.len())
        .any(|w| w == native[0].bytes));
    let mut short = [0xa5; 16];
    assert_eq!(
        encode_projection_evidence(&report, &codec, limits(), &mut short),
        Err(ProjectionEvidenceRefusal::BufferTooSmall)
    );
    assert_eq!(short, [0xa5; 16]);
    assert!(verify_projection_evidence(&report, &codec, limits(), &wire[..length - 1]).is_err());
    assert!(verify_projection_evidence(&report, &codec, limits(), &wire[..length + 1]).is_err());
    let changed_diagnostics = [text("changed-diagnostic@1")];
    let mut changed_native = native;
    changed_native[0].bytes = &[0xff, 0, 0x80, 8];
    let mut changed_contract = native;
    changed_contract[0].contract = text("different-contract@1");
    changed_contract[0].encoding = text("different-encoding@1");
    let changed_original = calibrated("cost@1", "calibration/native@1", 18);
    let changed_scores = [
        ProjectionScore::Native {
            fact: text("foreign"),
            score: changed_original,
        },
        ProjectionScore::Normalized {
            fact: text("foreign"),
            original: changed_original,
            projected: calibrated("inverse-cost@1", "calibration/projected@1", 82),
            policy: text("invert@1"),
        },
    ];
    let more_admitted = [a, b, ProjectionRealization { back: &extra, ..a }];
    let mut changed_failure = attempts;
    changed_failure[0].outcome = ProjectionMechanism::Failed(TerminalInfo::new(
        TerminalCategory::ProviderOrResourceLoss,
        [43; 32],
    ));
    let mut changed_fallback = attempts;
    changed_fallback[1].fallback_reason = Some(text("different-alternative@1"));
    let changed_facts = [
        ProjectionFact::Preserved {
            obligation: text("0"),
        },
        ProjectionFact::Transformed {
            obligation: text("1"),
            law: text("identity-layout@1"),
        },
        ProjectionFact::Lost {
            obligation: text("2"),
            class: ProjectionLoss::Precision,
            detail: &detail,
            native_fact: Some(text("foreign")),
        },
    ];
    for mutation in 0..15 {
        let mut draft = make();
        match mutation {
            0 => draft.basis.report = text("report/2"),
            1 => draft.basis.source = text("source/2"),
            2 => draft.basis.target = Some(text("target/2")),
            3 => draft.basis.projector = text("other-projector@1"),
            4 => draft.basis.requested_route = text("different-route@1"),
            5 => draft.basis.boundary.as_mut().unwrap().subject = text("different-subject"),
            6 => draft.basis.boundary.as_mut().unwrap().expanded_form = &other_expanded,
            7 => draft.diagnostics = &changed_diagnostics,
            8 => draft.scores = &changed_scores,
            9 => draft.native = &changed_native,
            10 => draft.native = &changed_contract,
            11 => draft.admitted = &more_admitted,
            12 => draft.attempts = &changed_failure,
            13 => draft.attempts = &changed_fallback,
            _ => draft.facts = &changed_facts,
        }
        let other_report = ProjectionReport::new(&domain, &policy, draft).unwrap();
        assert!(
            verify_projection_evidence(&other_report, &codec, limits(), &wire[..length]).is_err(),
            "mutation {mutation}"
        );
    }
    let other_policy = Policy(2);
    let other_codec = Codec(2);
    let other_report = ProjectionReport::new(&domain, &other_policy, make()).unwrap();
    assert!(
        verify_projection_evidence(&other_report, &other_codec, limits(), &wire[..length]).is_err()
    );
    assert_eq!(
        projection_evidence_len(&report, &other_codec, limits()),
        Err(ProjectionEvidenceRefusal::PolicyConfigurationMismatch)
    );
    let mut small = limits();
    small.source_bytes = 7;
    assert_eq!(
        projection_evidence_len(&report, &codec, small),
        Err(ProjectionEvidenceRefusal::Extent)
    );
}
#[test]
fn terminal_mechanisms_remain_distinct_without_a_target_or_fabricated_success() {
    let domain = Bits;
    let policy = Policy(0);
    let codec = Codec(0);
    let source = 0;
    let mut prior = None;
    for outcome in [
        ProjectionMechanism::Refused(TerminalInfo::new(
            TerminalCategory::RefusalOrAdmission,
            [1; 32],
        )),
        ProjectionMechanism::Failed(TerminalInfo::new(TerminalCategory::ExecutionFault, [2; 32])),
        ProjectionMechanism::Cancelled(TerminalInfo::new(TerminalCategory::Cancelled, [3; 32])),
    ] {
        let mut draft = input(&source, None, &[]);
        draft.mechanism = outcome;
        let report = ProjectionReport::new(&domain, &policy, draft).unwrap();
        let mut bytes = [0; 4096];
        let length = encode_projection_evidence(&report, &codec, limits(), &mut bytes).unwrap();
        verify_projection_evidence(&report, &codec, limits(), &bytes[..length]).unwrap();
        let digest = projection_evidence_digest(&bytes[..length]).unwrap();
        assert_ne!(prior, Some(digest));
        prior = Some(digest);
    }
    let one = 1;
    let preserved = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let exact =
        ProjectionReport::new(&domain, &policy, input(&one, Some(&one), &preserved)).unwrap();
    let mut bytes = [0; 4096];
    let n = encode_projection_evidence(&exact, &codec, limits(), &mut bytes).unwrap();
    verify_projection_evidence(&exact, &codec, limits(), &bytes[..n]).unwrap();
    let source = 3;
    let target = 1;
    let detail = 1;
    let facts = [
        ProjectionFact::Preserved {
            obligation: text("0"),
        },
        ProjectionFact::Lost {
            obligation: text("1"),
            class: ProjectionLoss::Precision,
            detail: &detail,
            native_fact: None,
        },
    ];
    let insufficient =
        ProjectionReport::new(&domain, &policy, input(&source, Some(&target), &facts)).unwrap();
    assert_eq!(
        insufficient.summary().disposition,
        ProjectionDisposition::Insufficient
    );
    let n = encode_projection_evidence(&insufficient, &codec, limits(), &mut bytes).unwrap();
    verify_projection_evidence(&insufficient, &codec, limits(), &bytes[..n]).unwrap();
    assert!(insufficient
        .require_policy(text("test/evidence-policy@1"))
        .is_err());
    let unbound = Budget(0);
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let value = 1;
    let report =
        ProjectionReport::new(&domain, &unbound, input(&value, Some(&value), &facts)).unwrap();
    assert_eq!(
        projection_evidence_len(&report, &codec, limits()),
        Err(ProjectionEvidenceRefusal::PolicyConfigurationUnavailable)
    );
}

struct BytesDomain;
impl ProjectionDomain for BytesDomain {
    type Source = [u8; 2];
    type Target = [u8; 2];
    type Detail = [u8; 2];
    fn source_contract(&self) -> ProjectionText<'_> {
        text("test/two-byte-source@1")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text("test/zero-head@1")
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        native.is_empty()
            && scores.is_empty()
            && mechanism == ProjectionMechanism::Completed
            && target == Some(&[0, source[1]])
            && facts.len() == 1
            && matches!(&facts[0], ProjectionFact::Lost{obligation,class:ProjectionLoss::Truncation,detail,native_fact:None}
                if obligation.as_str()=="value" && detail[0]==source[0] && detail[1]<=1)
    }
}
impl ProjectionPolicy<BytesDomain> for Policy {
    fn identity(&self) -> ProjectionText<'_> {
        text("test/evidence-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(configuration(self.0))
    }
    fn permits(&self, _: &[u8; 2], _: &[u8; 2], _: &[ProjectionFact<'_, BytesDomain>]) -> bool {
        self.0 >= 1
    }
}
struct BytesCodec(u64);
impl ProjectionEvidenceCodec<BytesDomain> for BytesCodec {
    fn identity(&self) -> ProjectionText<'_> {
        text("test/two-byte-evidence@1")
    }
    fn configuration_digest(&self) -> [u8; 32] {
        configuration(self.0)
    }
    fn source(
        &self,
        value: &[u8; 2],
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.raw(value)
    }
    fn target(
        &self,
        value: &[u8; 2],
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.raw(value)
    }
    fn detail(
        &self,
        value: &[u8; 2],
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.raw(value)
    }
    fn policy(
        &self,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.u64(self.0)
    }
}
#[test]
fn a_second_domain_retains_typed_detail_even_when_source_target_and_summary_match() {
    let domain = BytesDomain;
    let policy = Policy(1);
    let codec = BytesCodec(1);
    let source = [7, 0xff];
    let target = [0, 0xff];
    let detail = [7, 0];
    let other_detail = [7, 1];
    let facts = [ProjectionFact::Lost {
        obligation: text("value"),
        class: ProjectionLoss::Truncation,
        detail: &detail,
        native_fact: None,
    }];
    let other_facts = [ProjectionFact::Lost {
        obligation: text("value"),
        class: ProjectionLoss::Truncation,
        detail: &other_detail,
        native_fact: None,
    }];
    let make = |facts| ProjectionInput {
        basis: ProjectionBasis {
            report: text("report/bytes"),
            source: text("source/bytes"),
            target: Some(text("target/bytes")),
            projector: text("zero-head@1"),
            requested_route: text("drop-head@1"),
            boundary: None,
        },
        source: &source,
        target: Some(&target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    };
    let report = ProjectionReport::new(&domain, &policy, make(&facts)).unwrap();
    let other = ProjectionReport::new(&domain, &policy, make(&other_facts)).unwrap();
    assert_eq!(report.summary(), other.summary());
    let mut storage = [0; 4096];
    let n = encode_projection_evidence(&report, &codec, limits(), &mut storage).unwrap();
    verify_projection_evidence(&report, &codec, limits(), &storage[..n]).unwrap();
    assert_eq!(
        verify_projection_evidence(&other, &codec, limits(), &storage[..n]),
        Err(ProjectionEvidenceRefusal::EvidenceMismatch)
    );
    let mut small = limits();
    small.detail_bytes = 1;
    assert_eq!(
        projection_evidence_len(&report, &codec, small),
        Err(ProjectionEvidenceRefusal::Extent)
    );
    let mut invalid = limits();
    invalid.total_bytes = MAX_PROJECTION_EVIDENCE_BYTES + 1;
    assert_eq!(
        projection_evidence_len(&report, &codec, invalid),
        Err(ProjectionEvidenceRefusal::InvalidLimits)
    );
}

#[test]
fn wire_v1_matches_independent_python_framing_and_sha256_reference() {
    let domain = Bits;
    let policy = Policy(0);
    let codec = Codec(0);
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let report =
        ProjectionReport::new(&domain, &policy, input(&source, Some(&source), &facts)).unwrap();
    let mut bytes = [0; 4096];
    let length = encode_projection_evidence(&report, &codec, limits(), &mut bytes).unwrap();
    let hex = include_str!("../fixtures/projection_evidence_exact_v1.hex").trim();
    let (chunks, remainder) = hex.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    let expected = chunks
        .iter()
        .map(|part| u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(&bytes[..length], expected);
    verify_projection_evidence(&report, &codec, limits(), &expected).unwrap();
}
