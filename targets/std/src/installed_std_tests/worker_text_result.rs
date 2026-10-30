use super::host;
use crate::installed_std::InstalledRemoteFragment;
use conduit_core::PlanFragment;
use conduit_kernel::{
    scheduler::{HostCallRequest, SchedulerStatus},
    HostCallDisposition, HostCallOutcome,
};

fn pending() -> (
    conduit_core::HostAdvertisement,
    PlanFragment,
    InstalledRemoteFragment,
    HostCallRequest,
) {
    pending_text("Hello, greenhouse")
}

fn pending_text(
    text: &str,
) -> (
    conduit_core::HostAdvertisement,
    PlanFragment,
    InstalledRemoteFragment,
    HostCallRequest,
) {
    let source = format!("plot worker_text {{\n upper: text/upper\n show: face/text(\"Worker result\")\n {} >> upper >> show\n}}\n", serde_json::to_string(text).unwrap());
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&checked, "worker_text", &profile).unwrap();
    let host = host("worker-text-test");
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let fragment = plan.fragments[0].clone();
    let mut kernel = InstalledRemoteFragment::prepare(host.advertisement(), &fragment, 7).unwrap();
    for _ in 0..128 {
        kernel.step().unwrap();
        while let Some(request) = kernel.next_host_request() {
            if !kernel.complete_portable_host_call(request).unwrap() {
                return (host.advertisement().clone(), fragment, kernel, request);
            }
        }
    }
    panic!("ordinary uppercase pipeline did not reach its presenter");
}

#[test]
fn pending_worker_result_uses_actual_uppercase_and_waits_for_delivery() {
    let (host, fragment, mut kernel, request) = pending();
    let result = kernel.pending_worker_text_result(&host, request).unwrap();
    assert_eq!(result.fragment().text[0].text, "HELLO, GREENHOUSE");
    assert_eq!(result.fragment().basis.plan_id, fragment.plan_id);
    assert_eq!(
        result.fragment().basis.checked_plot_id,
        fragment.checked_plot_id
    );
    for _ in 0..4 {
        assert_ne!(kernel.step().unwrap(), SchedulerStatus::Drained);
    }
    assert!(kernel.pending_worker_text_result(&host, request).is_ok());
    kernel
        .complete_host_call(
            request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .unwrap();
    assert!(kernel.pending_worker_text_result(&host, request).is_err());
    let mut drained = false;
    for _ in 0..32 {
        if kernel.step().unwrap() == SchedulerStatus::Drained {
            drained = true;
            break;
        }
    }
    assert!(drained);
}

#[test]
fn cancelled_foreign_and_forged_worker_results_refuse() {
    let (host, _fragment, mut kernel, request) = pending();
    let mut foreign = host.clone();
    foreign.boot_id = "foreign-boot".into();
    assert!(kernel
        .pending_worker_text_result(&foreign, request)
        .is_err());
    let mut stale = host.clone();
    stale.offer_generation.0 += 1;
    assert!(kernel.pending_worker_text_result(&stale, request).is_err());
    let mut forged = request;
    forged.request.0 += 1;
    assert!(kernel.pending_worker_text_result(&host, forged).is_err());
    kernel.cancel().unwrap();
    assert!(kernel.pending_worker_text_result(&host, request).is_err());
}

#[test]
fn worker_text_profile_preserves_admitted_maximum_and_unicode() {
    for text in ["a".repeat(256), "ß".repeat(128)] {
        let (host, _, mut kernel, request) = pending_text(&text);
        let result = kernel.pending_worker_text_result(&host, request).unwrap();
        assert_eq!(result.fragment().text[0].text.len(), 256);
        assert_ne!(kernel.step().unwrap(), SchedulerStatus::Drained);
        kernel.cancel().unwrap();
        assert!(kernel.pending_worker_text_result(&host, request).is_err());
    }
}

#[test]
fn oversized_text_refuses_in_ordinary_canonical_expansion_before_worker_preparation() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let source = format!(
        "plot worker_text {{\n {} >> text/upper >> face/text(\"Worker result\")\n}}\n",
        serde_json::to_string(&"a".repeat(257)).unwrap()
    );
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    assert!(conduit_plot::expand_canonical_plot(&checked, "worker_text", &profile).is_err());
}

#[test]
fn result_path_works_with_std_host_owned_remote_resource_reservation() {
    let (_, fragment, raw, _) = pending();
    drop(raw);
    let mut owner = host("worker-text-test");
    let mut admitted = owner.prepare_remote_fragment(&fragment).unwrap();
    let mut observed = false;
    for _ in 0..128 {
        admitted.step().unwrap();
        while let Some(request) = admitted.next_host_request() {
            if admitted.complete_portable_host_call(request).unwrap() {
                continue;
            }
            let result = admitted
                .pending_worker_text_result(owner.advertisement(), request)
                .unwrap();
            assert_eq!(
                result.fragment().basis.active_play_id,
                admitted.identity().active_play_id
            );
            assert_eq!(result.fragment().text[0].text, "HELLO, GREENHOUSE");
            observed = true;
            admitted.cancel().unwrap();
            break;
        }
        if observed {
            break;
        }
    }
    assert!(observed);
    owner.release_remote_fragment(admitted).unwrap();
    // The same exact work may reserve again after ordinary release.
    let next = owner.prepare_remote_fragment(&fragment).unwrap();
    owner.release_remote_fragment(next).unwrap();
}

#[test]
fn caller_plan_edits_cannot_relabel_prepared_worker_result() {
    let (host, mut original, kernel, request) = pending();
    let expected = kernel.pending_worker_text_result(&host, request).unwrap();
    original.checked_plot_id = "forged-checked-plot".into();
    original.source_document_id = "forged-source".into();
    original.plan_id = "forged-plan".into();
    original.placements.clear();
    let actual = kernel.pending_worker_text_result(&host, request).unwrap();
    assert_eq!(actual, expected);
    assert_ne!(
        actual.fragment().basis.checked_plot_id,
        original.checked_plot_id
    );
}

#[test]
fn delivered_text_performs_selected_effect_once_and_rejects_replay() {
    let (host, _, mut kernel, request) = pending();
    let mut output = Vec::new();
    kernel
        .complete_worker_text_to(&host, request, &mut output)
        .unwrap();
    let first = output.clone();
    assert!(String::from_utf8(first.clone())
        .unwrap()
        .contains("HELLO, GREENHOUSE"));
    assert!(kernel
        .complete_worker_text_to(&host, request, &mut output)
        .is_err());
    assert_eq!(output, first);
}

#[test]
fn failed_text_write_does_not_acknowledge_pending_effect() {
    struct Refuse;
    impl std::io::Write for Refuse {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test refused"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (host, _, mut kernel, request) = pending();
    assert!(kernel
        .complete_worker_text_to(&host, request, &mut Refuse)
        .is_err());
    assert!(kernel.pending_worker_text_result(&host, request).is_ok());
    kernel.cancel().unwrap();
    assert!(kernel.pending_worker_text_result(&host, request).is_err());
}
