use conduit_core::{HostReportReason, Id, SemanticHash, validate_capability_report};
use conduit_rp2040_hil::{
    FIRMWARE_IDENTITY, FIXED_EXECUTOR_BUDGET, GENERIC_RP2040_BOARD_PROFILE, profile,
    with_capability_report,
};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../conformance/c5/rp2040-firmware-hil-v1.json");

fn expected(id: &str) -> Value {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap()["expected"]
        .clone()
}

#[test]
fn generic_report_excludes_unlinked_radio_services() {
    with_capability_report(10, |report| {
        let actual = serde_json::json!({
            "wifi": report.capabilities.iter().any(|capability| {
                capability.interface.id == Id("conduit/host.wifi-network")
            }),
            "ap": report.capabilities.iter().any(|capability| capability.mode == Id("ap")),
            "cyw43": report.capabilities.iter().any(|capability| {
                capability.subject == Id("cyw43")
            }),
            "zenoh_pico": report.capabilities.iter().any(|capability| {
                capability.interface.id == Id("conduit/transport.zenoh-pico")
                    || capability.subject == Id("zenoh-pico")
            }),
        });
        assert_eq!(
            actual,
            expected("generic-report-excludes-unlinked-radio-services")
        );
    });
}

#[test]
fn generic_report_binds_board_firmware_and_executor_profile() {
    let selected = profile();
    with_capability_report(10, |report| {
        let actual = serde_json::json!({
            "board_profile": report.current_constraints[0]
                == GENERIC_RP2040_BOARD_PROFILE.semantic_hash,
            "executor_profile": report.current_constraints[1] == selected.identity,
            "firmware": report.current_constraints[2] == FIRMWARE_IDENTITY
                && report.reporter.semantic_hash == FIRMWARE_IDENTITY,
            "constraint_count": report.current_constraints.len(),
        });
        assert_eq!(
            actual,
            expected("generic-report-binds-board-firmware-and-executor-profile")
        );
    });
}

#[test]
fn generic_report_has_bounded_resource_shape() {
    with_capability_report(10, |report| {
        let actual = serde_json::json!({
            "memory_bytes": report.available.memory_bytes,
            "timers": report.available.timers,
            "transports": report.available.transports,
            "resources": report.resources.len(),
        });
        assert_eq!(report.available, FIXED_EXECUTOR_BUDGET);
        assert_eq!(
            actual,
            expected("generic-report-has-bounded-resource-shape")
        );
    });
}

#[test]
fn forged_generic_report_identity_is_rejected() {
    with_capability_report(10, |report| {
        let mut forged = report;
        forged.identity = SemanticHash::from_bytes([0; 32]);
        let mut scratch = [SemanticHash::from_bytes([0; 32]); 8];
        let reason =
            validate_capability_report(&forged, Id("clock/boot-ticks"), 10, 9, &mut scratch)
                .unwrap_err();
        assert_eq!(reason, HostReportReason::IdentityMismatch);
        assert_eq!(
            serde_json::json!({"reason": reason.code()}),
            expected("forged-generic-report-identity-rejected")
        );
    });
}

#[test]
fn stale_generic_report_is_rejected() {
    with_capability_report(10, |report| {
        let mut scratch = [SemanticHash::from_bytes([0; 32]); 8];
        let reason =
            validate_capability_report(&report, Id("clock/boot-ticks"), 1_011, 9, &mut scratch)
                .unwrap_err();
        assert_eq!(reason, HostReportReason::Stale);
        assert_eq!(
            serde_json::json!({"reason": reason.code()}),
            expected("stale-generic-report-rejected")
        );
    });
}

#[test]
fn future_generic_report_is_rejected() {
    with_capability_report(10, |report| {
        let mut scratch = [SemanticHash::from_bytes([0; 32]); 8];
        let reason =
            validate_capability_report(&report, Id("clock/boot-ticks"), 9, 9, &mut scratch)
                .unwrap_err();
        assert_eq!(reason, HostReportReason::NotYetObserved);
        assert_eq!(
            serde_json::json!({"reason": reason.code()}),
            expected("future-generic-report-rejected")
        );
    });
}
