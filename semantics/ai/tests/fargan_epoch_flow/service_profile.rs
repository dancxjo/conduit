//! Source-admitted finite work before any direct16k sample processing.
use super::*;

const SOURCE: &str = include_str!("../../fargan_direct16k_service.conduit");
pub(super) struct PreparedServiceProfile {
    quantum: u32,
    quanta: u32,
    test_comparison: bool,
    material: Vec<u8>,
}
impl PreparedServiceProfile {
    pub(super) fn production() -> Self {
        Self::prepare(false)
    }
    pub(super) fn boundary_test() -> Self {
        Self::prepare(true)
    }
    pub(super) fn quantum(&self) -> u32 {
        self.quantum
    }
    pub(super) fn quanta(&self) -> u32 {
        self.quanta
    }
    pub(super) fn compares_boundaries(&self) -> bool {
        self.test_comparison
    }
    pub(super) fn material(&self) -> &[u8] {
        &self.material
    }
    fn prepare(test: bool) -> Self {
        let checked =
            check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
        let name = if test {
            "FarganDirect16kBoundaryTestService"
        } else {
            "FarganDirect16kProductionService"
        };
        let entry = if test {
            "ai/fargan-direct16k-boundary-test-service"
        } else {
            "ai/fargan-direct16k-production-service"
        };
        let quantum = if test { 4096u32 } else { 262144 };
        let ty = &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type;
        let request = request(ty, quantum.into(), 3);
        let bytes = request.canonical_bytes().unwrap();
        let admitted_request =
            super::interface::admit_retained_session_native(&checked, name, &bytes).unwrap();
        let read = |name| {
            let StructuredInfoValueShape::Leaf(bytes) =
                super::case_state::field(&admitted_request, name).shape()
            else {
                panic!("admitted service scalar")
            };
            u32::try_from(u64::from_le_bytes(bytes.try_into().unwrap())).unwrap()
        };
        let quantum = read("continuation_quantum");
        let quanta = read("continuation_quanta");
        let graph =
            expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
        let ConfigurationValue::Text(hex) = &graph.expanded.gears[0].configuration[0].value else {
            panic!("service program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
        let raw = program.evaluate(&bytes).unwrap();
        let admitted = StructuredInfoValue::leaf(program.output_type.clone(), raw.clone()).unwrap();
        assert_eq!(
            StructuredInfoValue::from_canonical_bytes(&admitted.canonical_bytes().unwrap())
                .unwrap(),
            admitted
        );
        let total = u64::from_le_bytes(raw.as_slice().try_into().unwrap());
        assert_eq!(u128::from(total), 262144u128 * 4 + u128::from(quantum) * 3);
        let material = serde_json::to_vec(&serde_json::json!({"source":SOURCE,"profile":name,"request":bytes,"program":hex,"raw_output":raw,"admitted_output":admitted.canonical_bytes().unwrap(),"total_services":total,"scope":"finite hosted fixture service; not ModelComputeLifecycle, physical time or target admission"})).unwrap();
        Self {
            quantum,
            quanta,
            test_comparison: test,
            material,
        }
    }
}
fn request(ty: &StructuredInfoType, quantum: u64, quanta: u64) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("service request")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                let value = match f.name() {
                    "preliminary_quantum" => 262144u64,
                    "preliminary_stages" => 4,
                    "continuation_quantum" => quantum,
                    "continuation_quanta" => quanta,
                    _ => panic!("field"),
                };
                StructuredFieldValue::new(
                    f.name(),
                    StructuredInfoValue::leaf(f.value_type().clone(), value.to_le_bytes().to_vec())
                        .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn direct16k_service_admission_retains_source_and_refuses_foreign_work() {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    for (name, quantum) in [
        ("FarganDirect16kProductionService", 262144u64),
        ("FarganDirect16kBoundaryTestService", 4096),
    ] {
        let ty = &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type;
        let encoded = ty.canonical_bytes().unwrap();
        assert_eq!(
            StructuredInfoType::from_canonical_bytes(&encoded).unwrap(),
            *ty
        );
        for (q, n, ok) in [
            (quantum, 3, true),
            (quantum, 0, false),
            (quantum, 4, false),
            (0, 3, false),
            (u64::MAX, 3, false),
            (quantum + 1, 3, false),
        ] {
            let bytes = request(ty, q, n).canonical_bytes().unwrap();
            assert_eq!(
                super::interface::admit_retained_session_native(&checked, name, &bytes).is_ok(),
                ok
            );
        }
        eprintln!(
            "{name} canonical Type{}B/frame{}B",
            encoded.len(),
            request(ty, quantum, 3).canonical_bytes().unwrap().len()
        );
    }
    assert!(!PreparedServiceProfile::production().material().is_empty());
    assert!(PreparedServiceProfile::boundary_test().compares_boundaries());
}
