use conduit_core::{
    CAPABILITY_REPORT_SCHEMA_VERSION, CapabilityReport, ExecutorKind, HostReportReason, Id,
    PassportStatus, PassportStatusObservation, PinnedDescriptor, PlanResourceBudget,
    ReportCapability, ReportMembership, SemanticHash, validate_capability_report,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}

const CAP_A: ReportCapability<'static> = ReportCapability {
    interface: pin("fixture/capability-a", 1),
    mode: Id("client"),
    subject: Id("interface-a"),
    details: SemanticHash::from_bytes([2; 32]),
    capacity: PlanResourceBudget {
        memory_bytes: 16,
        transports: 1,
        ..PlanResourceBudget::ZERO
    },
};
const CAP_B: ReportCapability<'static> = ReportCapability {
    interface: pin("fixture/capability-b", 3),
    mode: Id("server"),
    subject: Id("interface-b"),
    details: SemanticHash::from_bytes([4; 32]),
    capacity: PlanResourceBudget {
        memory_bytes: 32,
        transports: 2,
        ..PlanResourceBudget::ZERO
    },
};

fn report<'a>(
    capabilities: &'a [ReportCapability<'a>],
    executors: &'a [ExecutorKind],
    targets: &'a [Id<'a>],
) -> CapabilityReport<'a> {
    let mut report = CapabilityReport {
        schema_version: CAPABILITY_REPORT_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("fixture/report"),
        host: Id("fixture/host"),
        reporter: pin("fixture/reporter", 5),
        trust: pin("fixture/trust", 6),
        membership: None,
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 20,
        available: PlanResourceBudget {
            memory_bytes: 64,
            cpu_units: 2,
            transports: 4,
            ..PlanResourceBudget::ZERO
        },
        capabilities,
        resources: &[],
        topology: &[],
        supported_executors: executors,
        supported_targets: targets,
        supported_abis: &[Id("component-v1")],
        minimum_plan_version: 0,
        maximum_plan_version: 0,
        current_constraints: &[],
    };
    let mut scratch = [ZERO; 16];
    report.identity = report.computed_semantic_hash(&mut scratch).unwrap();
    report
}

#[test]
fn report_identity_is_independent_of_observation_collection_order() {
    let first_capabilities = [CAP_A, CAP_B];
    let second_capabilities = [CAP_B, CAP_A];
    let first_executors = [ExecutorKind::WasmComponent, ExecutorKind::RemoteEndpoint];
    let second_executors = [ExecutorKind::RemoteEndpoint, ExecutorKind::WasmComponent];
    let first_targets = [Id("wasm32-wasip2"), Id("x86_64-unknown-linux-gnu")];
    let second_targets = [Id("x86_64-unknown-linux-gnu"), Id("wasm32-wasip2")];
    let first = report(&first_capabilities, &first_executors, &first_targets);
    let second = report(&second_capabilities, &second_executors, &second_targets);
    assert_eq!(first.identity, second.identity);
    let mut scratch = [ZERO; 16];
    assert_eq!(
        validate_capability_report(&first, Id("fixture/clock"), 15, 0, &mut scratch),
        Ok(())
    );
}

#[test]
fn freshness_time_basis_and_identity_fail_closed() {
    let capabilities = [CAP_A];
    let executors = [ExecutorKind::WasmComponent];
    let targets = [Id("wasm32-wasip2")];
    let valid = report(&capabilities, &executors, &targets);
    let mut scratch = [ZERO; 16];
    assert_eq!(
        validate_capability_report(&valid, Id("fixture/clock"), 21, 0, &mut scratch),
        Err(HostReportReason::Stale)
    );
    assert_eq!(
        validate_capability_report(&valid, Id("other/clock"), 15, 0, &mut scratch),
        Err(HostReportReason::TimeBasisMismatch)
    );
    let changed = CapabilityReport {
        available: PlanResourceBudget {
            memory_bytes: 63,
            ..valid.available
        },
        ..valid
    };
    assert_eq!(
        validate_capability_report(&changed, Id("fixture/clock"), 15, 0, &mut scratch),
        Err(HostReportReason::IdentityMismatch)
    );
}

#[test]
fn membership_binding_is_identified_and_status_checked_at_resolution_time() {
    let capabilities = [CAP_A];
    let executors = [ExecutorKind::WasmComponent];
    let targets = [Id("wasm32-wasip2")];
    let mut bound = report(&capabilities, &executors, &targets);
    let membership = ReportMembership {
        realm: Id("fixture/realm"),
        entity: Id("fixture/entity"),
        passport: SemanticHash::from_bytes([7; 32]),
        status: PassportStatusObservation {
            passport: SemanticHash::from_bytes([7; 32]),
            realm: Id("fixture/realm"),
            entity: Id("fixture/entity"),
            reporter: pin("fixture/status-reporter", 8),
            time_basis: Id("fixture/clock"),
            observed_at_tick: 9,
            valid_until_tick: 18,
            status: PassportStatus::Active,
        },
    };
    let unbound_identity = bound.identity;
    bound.membership = Some(membership);
    let mut scratch = [ZERO; 16];
    bound.identity = bound.computed_semantic_hash(&mut scratch).unwrap();
    assert_ne!(bound.identity, unbound_identity);
    assert_eq!(
        validate_capability_report(&bound, Id("fixture/clock"), 15, 0, &mut scratch),
        Ok(())
    );
    assert_eq!(
        validate_capability_report(&bound, Id("fixture/clock"), 18, 0, &mut scratch),
        Err(HostReportReason::MembershipInvalid)
    );

    let mut mismatched = bound;
    mismatched.membership.as_mut().unwrap().status.entity = Id("fixture/other-entity");
    mismatched.identity = mismatched.computed_semantic_hash(&mut scratch).unwrap();
    assert_eq!(
        validate_capability_report(&mismatched, Id("fixture/clock"), 15, 0, &mut scratch),
        Err(HostReportReason::MembershipInvalid)
    );
}
