use super::StdHostComposition;
use crate::{StdHost, StdHostConfig};
use conduit_core::{BootId, HostId, OfferGeneration};

fn host(composition: StdHostComposition) -> StdHost {
    StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("composition-test"),
            boot_id: BootId::from("composition-boot"),
            offer_generation: OfferGeneration(1),
        },
        composition,
    )
}

fn offered(host: &StdHost, kind: &str) -> bool {
    host.advertisement()
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == kind)
}

#[test]
fn a_selected_family_contributes_only_its_exact_operation_offers() {
    let host = host(StdHostComposition::minimal().with_text());

    assert!(offered(&host, "text/literal"));
    assert!(offered(&host, "text/upper"));
    assert!(offered(&host, "text/join"));
    assert!(offered(&host, "presentation/text"));
    assert!(!offered(&host, "flow/pulse"));
    assert!(!offered(&host, "time/every"));
    assert!(!offered(&host, "state/count"));
    assert_eq!(host.advertisement().resources.len(), 1);
    assert_eq!(
        host.advertisement().resources[0].pool_id.as_str(),
        "std/presentation"
    );
}

#[test]
fn external_websocket_listener_is_an_explicit_capability_and_resource_family() {
    let omitted = host(StdHostComposition::minimal());
    assert!(!offered(
        &omitted,
        conduit_net::EXTERNAL_WEBSOCKET_LISTENER_KIND
    ));
    assert!(omitted.advertisement().resources.is_empty());

    let selected = host(StdHostComposition::minimal().with_external_websocket());
    assert!(offered(
        &selected,
        conduit_net::EXTERNAL_WEBSOCKET_LISTENER_KIND
    ));
    assert_eq!(selected.advertisement().resources.len(), 1);
    assert_eq!(
        selected.advertisement().resources[0].class_id.as_str(),
        conduit_net::EXTERNAL_WEBSOCKET_LISTENER_RESOURCE
    );
}

#[test]
fn hosted_http_is_opt_in_and_seals_resources_operations_and_authority() {
    let omitted = host(StdHostComposition::minimal());
    assert!(!offered(&omitted, conduit_web::HTTP_CLIENT_KIND));
    assert!(!offered(&omitted, conduit_web::HTTP_SERVER_KIND));

    let selected = host(StdHostComposition::minimal().with_http());
    let client = selected
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND)
        .unwrap();
    let server = selected
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_web::HTTP_SERVER_KIND)
        .unwrap();
    assert_eq!(client.host_operations.len(), 1);
    assert_eq!(client.authority_requirements.len(), 1);
    assert_eq!(server.host_operations.len(), 2);
    assert_eq!(server.authority_requirements.len(), 2);
    assert_eq!(selected.advertisement().resources.len(), 2);
}

#[test]
fn compiled_families_are_not_ambient_runtime_promises() {
    let minimal = host(StdHostComposition::minimal());
    let reference = host(StdHostComposition::reference());

    for kind in [
        "flow/pulse",
        "presentation/show",
        "time/tick",
        "time/every",
        "time/debounce",
        "time/timeout",
        "time/delay",
        "time/throttle",
        "text/literal",
        "text/upper",
        "text/join",
        "presentation/text",
        "state/count",
        "state/toggle",
        "presentation/count",
        "state/latest",
        "flow/tee",
        "flow/gate",
        "state/select",
        "robotics/observe-bump",
        "robotics/observe-imu",
        "robotics/observe-range",
        "robotics/observe-odometry",
        "robotics/observe-battery",
        "robotics/velocity-intent",
        "robotics/drive-differential",
        "file/copy",
        conduit_web::HTTP_CLIENT_KIND,
        conduit_web::HTTP_SERVER_KIND,
    ] {
        assert!(!offered(&minimal, kind), "minimal host offered {kind}");
        assert!(offered(&reference, kind), "reference host omitted {kind}");
    }
    assert!(minimal.advertisement().resources.is_empty());
}

#[test]
fn planner_cannot_obtain_an_unselected_family_from_a_category_prefix() {
    let host = host(StdHostComposition::minimal().with_text());
    let form = conduit_form::parse_with_startup(
        include_str!("../../../../proof/fixtures/forms/signal-demo.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )
    .expect("Signal form checks independently of host composition");

    assert!(host.plan_local(&form, None).is_err());
}

#[test]
fn reference_host_browser_and_pico_have_different_exact_offer_sets() {
    let std = host(StdHostComposition::reference());
    let browser = conduit_signal_conformance::distributed_browser_sink_advertisement();
    let pico = conduit_signal_conformance::pico_local_advertisement();

    let kinds = |advertisement: &conduit_core::HostAdvertisement| {
        advertisement
            .capabilities
            .iter()
            .map(|offer| offer.kind_id.as_str().to_owned())
            .collect::<std::collections::BTreeSet<_>>()
    };

    assert_ne!(kinds(std.advertisement()), kinds(&browser));
    assert_ne!(kinds(std.advertisement()), kinds(&pico));
    assert_ne!(kinds(&browser), kinds(&pico));
}

#[test]
fn reference_host_advertises_every_supported_std_revision_and_no_legacy_revision() {
    let host = host(StdHostComposition::reference());
    let advertised = host
        .advertisement()
        .capabilities
        .iter()
        .filter(|offer| {
            let revision = offer.kind_contract_revision.as_str();
            offer.kind_id.as_str() != conduit_semantic_catalog::INSTRUMENT_MAP_KIND
                && (revision.starts_with("conduit.std/")
                    || revision.starts_with("conduit.input/")
                    || offer.kind_id.as_str() == conduit_semantic_catalog::BOOL_PRESENTATION_KIND
                    || offer.kind_id.as_str() == conduit_presentation::BITMAP_PRESENTATION_KIND)
        })
        .cloned()
        .collect::<Vec<_>>();
    let supported = super::supported_nucleus_offers()
        .into_iter()
        .filter(|offer| {
            offer
                .implementation
                .implementation_id
                .as_str()
                .starts_with("std/")
        })
        .collect::<Vec<_>>();

    assert_eq!(advertised, supported);
    assert!(host
        .advertisement()
        .capabilities
        .iter()
        .any(|offer| { offer == &conduit_std_offers::instrument_map_std_offer() }));
}

#[test]
fn pulse_observation_is_an_explicit_effect_free_family() {
    let selected = host(StdHostComposition::minimal().with_pulse_observation());
    let baseline = host(StdHostComposition::minimal());
    let mut expected = vec![conduit_std_offers::pulse_observe_offer()];
    expected.extend(baseline.advertisement().capabilities.iter().cloned());
    assert_eq!(selected.advertisement().capabilities, expected);
    assert!(selected.advertisement().resources.is_empty());
    assert!(!offered(
        &host(StdHostComposition::reference()),
        conduit_time::PULSE_OBSERVE_KIND
    ));
    assert!(!offered(
        &host(StdHostComposition::minimal().with_time()),
        conduit_time::PULSE_OBSERVE_KIND
    ));
}
