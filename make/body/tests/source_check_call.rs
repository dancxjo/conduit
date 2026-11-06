//! Ordinary authored Source checking via the production selected native Host Call.
use conduit_core::*;
use conduit_planner::*;
use conduit_plot::*;
use conduit_std_host::{source_check::*, *};
use std::collections::BTreeMap;
#[path = "source_check_call/allocation.rs"]
mod allocation;
#[path = "source_check_call/support.rs"]
mod support;
const VALID: &str = include_str!("../../../plots/hello/main.conduit");
const INVALID: &str = "# α 🦀\nplot hello {\n first: unavailable/kind\n}\n";
const CHECK: &str = include_str!("source_check_call/check.conduit");
fn bounds() -> SourceCheckPreparationBounds {
    SourceCheckPreparationBounds {
        request_bytes: 16 * 1024,
        report_bytes: 64 * 1024,
        retained_bytes: 128 * 1024,
    }
}
struct Capture {
    bytes: Vec<u8>,
    calls: usize,
}
impl ExternalForeOutputAdapter for Capture {
    fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
        panic!("requires kernel commit")
    }
    fn deliver_committed(
        &mut self,
        witness: CommittedForeValue<'_>,
        delivery: ExternalForeDelivery,
    ) -> Result<(), String> {
        assert_eq!(delivery.front_port_id.as_str(), "reply");
        assert_eq!(witness.sequence(), 0);
        self.bytes.extend_from_slice(witness.bytes());
        self.calls += 1;
        Ok(())
    }
}
#[test]
fn ordinary_plot_returns_actual_valid_and_invalid_source_reports_with_host_call_signs() {
    let (mut good, good_request) = support::owner(VALID, 7);
    let (mut bad, bad_request) = support::owner(INVALID, 8);
    let schema = SourceCheckNativeSchema::prepare().unwrap();
    let good_info = good.project(&good_request).unwrap();
    let bad_info = bad.project(&bad_request).unwrap();
    assert_eq!(good_info.check, CheckOutcome::Accepted);
    assert!(good_info.candidate.checked_plot_id.is_some());
    assert!(good_info.candidate.expanded_plot_id.is_some());
    assert_eq!(bad_info.check, CheckOutcome::Rejected);
    let mut stale_request = good_request.clone();
    stale_request.snapshot.generation += 1;
    let mut other_entry = good_request.clone();
    other_entry.entry = "not-prepared".into();
    let mut other_context = good_request.clone();
    other_context.context.catalog = "different-catalog".into();
    let requests = [
        &good_request,
        &bad_request,
        &stale_request,
        &other_entry,
        &other_context,
    ]
    .map(|r| {
        schema
            .encode_request(r, bounds().request_bytes as usize)
            .unwrap()
    });
    let expected = [
        SourceCheckReply::Checked(&good_info),
        SourceCheckReply::Checked(&bad_info),
        SourceCheckReply::Refused(SourceCheckRefusal::StaleSource),
        SourceCheckReply::NotPrepared,
        SourceCheckReply::Refused(SourceCheckRefusal::UnknownContext),
    ]
    .map(|r| {
        schema
            .encode_reply(r, bounds().report_bytes as usize)
            .unwrap()
    });
    let prepared = PreparedSourceChecker::prepare(
        &mut [
            SourceCheckPreparation {
                owner: &mut good,
                request: &good_request,
            },
            SourceCheckPreparation {
                owner: &mut bad,
                request: &bad_request,
            },
        ],
        bounds(),
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    prepared
        .install_authoring(&mut startup, &mut profiles)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(CHECK), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "check-source", &profiles).unwrap();
    let mut host = StdHost::new();
    assert!(support::plan(&host, &authored, &[], bounds()).is_err());
    let definition = support::definition(host.advertisement(), &prepared);
    let grant = definition.grant.clone();
    // SAFETY: The independently authorized immutable Source owners and current
    // Host definition precede planning. The production provider retains only
    // fixed packets and has no compiler/IO/allocator in its immediate read.
    unsafe { host.install_immediate_native_source(definition, Box::new(prepared.into_provider())) }
        .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
    assert!(support::plan(&host, &authored, &[], bounds()).is_err());
    let mut forged = grant.clone();
    forged.grant_id = "fixture/copied-descriptive-grant".into();
    let unapproved = support::plan(&host, &authored, &[forged], bounds()).unwrap();
    let mut refused = Capture {
        bytes: vec![],
        calls: 0,
    };
    assert!(host
        .run_external_plot_to(
            unapproved.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: requests[0].clone(),
            }],
            &mut refused,
            &mut Vec::new(),
            &mut ThreadTimer
        )
        .is_err());
    assert_eq!(refused.calls, 0);
    let plan = support::plan(&host, &authored, &[grant], bounds()).unwrap();
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("before-source-check").unwrap())
        .unwrap();
    let mut cancelled = Capture {
        bytes: vec![],
        calls: 0,
    };
    let stopped = host
        .run_external_plot_controlled_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "request".into(),
                track: ConnectionTrack::Payload,
                bytes: requests[0].clone(),
            }],
            &mut cancelled,
            &mut Vec::new(),
            &mut ThreadTimer,
            &control,
        )
        .unwrap();
    assert_eq!(cancelled.calls, 0);
    assert!(stopped
        .kernel
        .unwrap()
        .kernel_sign
        .iter()
        .all(|event| event.kind != conduit_kernel::KernelEventKind::HostCallRequested));
    // Subsequent ordinary Plays must still be usable after the cancelled run.
    let mut plays = Vec::new();
    for (request, expected) in requests.into_iter().zip(expected) {
        let mut capture = Capture {
            bytes: vec![],
            calls: 0,
        };
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "request".into(),
                    track: ConnectionTrack::Payload,
                    bytes: request,
                }],
                &mut capture,
                &mut Vec::new(),
                &mut ThreadTimer,
            )
            .unwrap();
        assert_eq!(capture.calls, 1);
        assert_eq!(capture.bytes, expected);
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|e| e.kind == conduit_kernel::KernelEventKind::HostCallRequested));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|e| e.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
        plays.push(kernel.active_play_id);
    }
    assert_ne!(plays[0], plays[1]);
}

#[path = "source_check_call/lifecycle.rs"]
mod lifecycle;

#[test]
fn declaration_checks_source_without_preparing_or_offering_a_checker() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    PreparedSourceChecker::install_authoring_declaration(
        &mut startup,
        &mut profiles,
        bounds().request_bytes,
        bounds().report_bytes,
    )
    .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(CHECK), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "check-source", &profiles).unwrap();
    let host = StdHost::new();
    assert!(!host
        .advertisement()
        .capabilities
        .iter()
        .any(|offer| { offer.kind_id.as_str() == SOURCE_CHECK_KIND }));
    assert!(support::plan(&host, &authored, &[], bounds()).is_err());
}
