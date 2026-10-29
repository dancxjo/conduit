//! Admitted remote fragment proof with a named finite fake ALSA provider.
use super::*;
use crate::{AdmittedRemoteFragment, StdHost, StdHostComposition, StdHostConfig};
use conduit_kernel::scheduler::HostCallRequest;
use std::collections::BTreeMap;

fn prepared(script: &str, request: &str) -> (PathBuf, StdHost, AdmittedRemoteFragment) {
    prepared_with_format(script, request, AlsaMicrophoneFormat::ExactHardware)
}

fn prepared_with_format(
    script: &str,
    request: &str,
    format: AlsaMicrophoneFormat,
) -> (PathBuf, StdHost, AdmittedRemoteFragment) {
    let (root, discovery) = parsed_fixture(script, "remote");
    let selected = discovery.observations[0].clone();
    let adapter = discovery
        .initialize_with_format(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
            format,
        )
        .unwrap();
    let mut host = StdHost::new_with_microphone(
        StdHostConfig {
            host_id: HostId::from("host/remote-microphone-fixture"),
            boot_id: BootId::from("boot/remote-microphone-fixture"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        adapter,
    )
    .unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_microphone_clip_catalogs(&mut startup, &mut profiles)
        .unwrap();
    let source = format!("plot microphone-remote-fixture {{\n microphone: media/capture-microphone-clip\n \"{request}\" >> microphone.request\n}}.\n");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "microphone-remote-fixture", &profiles)
            .unwrap();
    let advertisements = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded, &advertisements).unwrap();
    let grant = host
        .microphone_authority_grant("grant/explicit-test-microphone")
        .unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 256,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &BTreeMap::new(),
    )
    .unwrap();
    let remote = host.prepare_remote_fragment(&plan.fragments[0]).unwrap();
    (root, host, remote)
}

fn microphone_request(host: &mut StdHost, remote: &mut AdmittedRemoteFragment) -> HostCallRequest {
    for _ in 0..64 {
        if let Some(request) = remote.next_host_request() {
            if remote
                .host_request_view(request)
                .unwrap()
                .contract_id
                .as_str()
                == conduit_std_offers::MICROPHONE_CLIP_OPERATION
            {
                return request;
            }
            assert!(!host
                .complete_remote_microphone_host_call(remote, request, || false)
                .unwrap());
            assert!(remote.complete_portable_host_call(request).unwrap());
        } else {
            remote.step().unwrap();
        }
    }
    panic!("admitted fake-provider fragment never requested microphone capture")
}

#[test]
fn admitted_remote_microphone_uses_selected_provider_and_retains_bounded_capture() {
    let _process = microphone_process();
    for format in [
        AlsaMicrophoneFormat::ExactHardware,
        AlsaMicrophoneFormat::AlsaPlug,
    ] {
        let (root, mut host, mut remote) = prepared_with_format(
            "#!/bin/sh\ndd if=/dev/zero bs=32 count=1 2>/dev/null\n",
            "capture",
            format,
        );
        let request = microphone_request(&mut host, &mut remote);
        assert!(host
            .complete_remote_microphone_host_call(&mut remote, request, || false)
            .unwrap());
        let receipt = host.microphone.as_mut().unwrap().take_receipt().unwrap();
        assert_eq!(receipt.raw_pcm_bytes, 32);
        assert_eq!(receipt.card_id, "DSP");
        assert_eq!(receipt.device, 7);
        assert_eq!(receipt.implementation_id, format.implementation_id());
        host.release_remote_fragment(remote).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn remote_microphone_invalid_request_and_missing_provider_do_not_capture() {
    let _process = microphone_process();
    for (input, missing, expected) in [
        ("not-capture", false, "not exact"),
        ("capture", true, "no initialized adapter"),
    ] {
        let (root, mut host, mut remote) = prepared("#!/bin/sh\nexit 7\n", input);
        let request = microphone_request(&mut host, &mut remote);
        let selected = if missing {
            host.microphone.take()
        } else {
            None
        };
        let error = host
            .complete_remote_microphone_host_call(&mut remote, request, || false)
            .unwrap_err();
        assert!(error.contains(expected), "{error}");
        if let Some(adapter) = selected {
            host.microphone = Some(adapter);
        }
        assert!(host.microphone.as_mut().unwrap().take_receipt().is_none());
        host.release_remote_fragment(remote).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn remote_microphone_provider_failure_and_cancellation_complete_without_audio() {
    let _process = microphone_process();
    for cancelled in [false, true] {
        let (root, mut host, mut remote) = prepared("#!/bin/sh\nprintf '\\000\\000'\n", "capture");
        let request = microphone_request(&mut host, &mut remote);
        assert!(host
            .complete_remote_microphone_host_call(&mut remote, request, || cancelled)
            .unwrap());
        assert!(host.microphone.as_mut().unwrap().take_receipt().is_none());
        host.release_remote_fragment(remote).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn remote_microphone_rejects_foreign_host_boot_and_generation_before_capture() {
    let _process = microphone_process();
    for change in ["host", "boot", "generation"] {
        let (root, mut host, mut remote) = prepared("#!/bin/sh\nexit 7\n", "capture");
        let request = microphone_request(&mut host, &mut remote);
        let original = host.advertisement.clone();
        match change {
            "host" => host.advertisement.host_id = HostId::from("host/different"),
            "boot" => host.advertisement.boot_id = BootId::from("boot/different"),
            _ => host.advertisement.offer_generation.0 += 1,
        }
        assert!(host
            .complete_remote_microphone_host_call(&mut remote, request, || false)
            .unwrap_err()
            .contains("another Host, Boot, or offer generation"));
        assert!(host.microphone.as_mut().unwrap().take_receipt().is_none());
        host.advertisement = original;
        host.release_remote_fragment(remote).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn changed_microphone_format_is_refused_before_provider_execution() {
    let _process = microphone_process();
    let (root, mut host, mut remote) = prepared("#!/bin/sh\nexit 97\n", "capture");
    let (other_root, mut other, other_remote) = prepared_with_format(
        "#!/bin/sh\nexit 98\n",
        "capture",
        AlsaMicrophoneFormat::AlsaPlug,
    );
    let request = microphone_request(&mut host, &mut remote);
    let original = host.microphone.take();
    host.microphone = other.microphone.take();
    assert!(host
        .complete_remote_microphone_host_call(&mut remote, request, || false)
        .unwrap_err()
        .contains("exact planned Back"));
    assert!(host.microphone.as_mut().unwrap().take_receipt().is_none());
    other.microphone = host.microphone.take();
    host.microphone = original;
    host.release_remote_fragment(remote).unwrap();
    other.release_remote_fragment(other_remote).unwrap();
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(other_root).unwrap();
}
