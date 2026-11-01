//! Exact fidelity and selected-policy permission remain separate after transport.
use super::*;
struct Permission(bool);
fn configuration(allowed: bool) -> [u8; 32] {
    semantic_digest("test/frozen-permission@1", &[u8::from(allowed)])
}
impl ProjectionPolicy<Bits> for Permission {
    fn identity(&self) -> ProjectionText<'_> {
        text("test/frozen-permission-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(configuration(self.0))
    }
    fn permits(&self, _: &u64, _: &u64, _: &[ProjectionFact<'_, Bits>]) -> bool {
        self.0
    }
}
impl ProjectionEvidenceCodec<Bits> for Permission {
    fn identity(&self) -> ProjectionText<'_> {
        text("test/frozen-permission-evidence@1")
    }
    fn configuration_digest(&self) -> [u8; 32] {
        configuration(self.0)
    }
    fn source(
        &self,
        v: &u64,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        Codec(0).source(v, out)
    }
    fn target(
        &self,
        v: &u64,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        Codec(0).target(v, out)
    }
    fn detail(
        &self,
        v: &usize,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        Codec(0).detail(v, out)
    }
    fn policy(
        &self,
        out: &mut ProjectionEvidenceWriter<'_>,
    ) -> Result<(), ProjectionEvidenceRefusal> {
        out.tag(u8::from(self.0))
    }
}
#[test]
fn exact_wire_claims_do_not_waive_the_encoded_selected_policy() {
    let source = 1;
    let facts = [ProjectionFact::Preserved {
        obligation: text("0"),
    }];
    let mut bytes = [0; 4096];
    for allowed in [false, true] {
        let policy = Permission(allowed);
        let report =
            ProjectionReport::new(&Bits, &policy, input(&source, Some(&source), &facts)).unwrap();
        let (_, allocations) = allocation_probe::observe(|| {
            let n = encode_projection_evidence(&report, &policy, limits(), &mut bytes).unwrap();
            verify_projection_evidence(&report, &policy, limits(), &bytes[..n]).unwrap();
            let view = inspect_projection_evidence(&bytes[..n]).unwrap();
            assert_eq!(
                view.claimed_summary.disposition,
                ProjectionDisposition::Completed(ProjectionFidelity::Exact)
            );
            assert_eq!(view.policy_bytes, &[u8::from(allowed)]);
            assert_eq!(report.require_exact(), Ok(&source));
            assert_eq!(
                report
                    .require_bound_policy(
                        ProjectionPolicy::identity(&policy),
                        configuration(allowed)
                    )
                    .is_ok(),
                allowed
            );
        });
        assert_eq!(allocations, 0);
    }
}
