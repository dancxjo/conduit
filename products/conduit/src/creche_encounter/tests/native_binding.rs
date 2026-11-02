//! Native IMAGE binding over real checked configuration and verified fixture
//! payloads. The payload is not executable; no deployment/Boot/join is claimed.
use super::*;
use conduit_host_make::{bind_native_release_configuration, check_host_configuration};

fn check(configuration: conduit_host_make::HostConfiguration) -> CheckedHostConfiguration {
    check_host_configuration(
        configuration,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap()
}
fn bind(
    admitted: &CheckedHostConfiguration,
    selected: &CheckedHostConfiguration,
    image: &conduit_host_make::HostImage,
) -> Result<(conduit_host_make::HostImage, Vec<u8>), String> {
    bind_native_release_configuration(
        admitted,
        image,
        selected,
        &bundle_common::native_release(image, b"artifact"),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
}
fn image(configuration: &CheckedHostConfiguration) -> conduit_host_make::HostImage {
    build_host_image(
        configuration.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &SporeOutputKind::NativeBundle,
        &BuildInputs {
            source_identity: "source/reviewed-native".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0
}

#[test]
fn authored_name_limits_and_supported_operation_bind_new_image_same_release() {
    let source = include_str!(
        "../../../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit"
    );
    let full = check(conduit_host_make::parse_host_configuration_conduit(source).unwrap());
    let mut baseline = full.configuration().clone();
    baseline.capabilities.clear();
    let baseline = check(baseline);
    let prior = image(&baseline);
    let mut authored = full.configuration().clone();
    authored.name = "threebee-authored-by-body".into();
    authored.limits.queue_items /= 2;
    let authored = check(authored);
    let (built, bytes) = bind(&baseline, &authored, &prior).unwrap();
    assert_ne!(built.manifest.image_id, prior.manifest.image_id);
    assert_eq!(
        built.manifest.source_configuration_id.as_deref(),
        Some(authored.configuration_id())
    );
    assert_eq!(
        built.manifest.source_identity,
        prior.manifest.source_identity
    );
    assert_eq!(built.manifest.bounds, authored.profile().bounds);
    assert!(built
        .manifest
        .implementations
        .contains(&"std/kernel-time-tick@2".to_owned()));
    conduit_host_make::verify_image_binding(&built, &bytes).unwrap();
}

#[test]
fn changed_native_closure_source_or_browser_target_refuses() {
    let baseline = checked();
    let prior = image(&baseline);
    let mut removed_base = baseline.configuration().clone();
    removed_base.bases.pop();
    assert!(bind(&baseline, &check(removed_base), &prior).is_err());
    let browser = check(
        conduit_host_make::parse_host_configuration_conduit(include_str!(
            "../../../../../targets/browser/profiles/environment-recorder.host.conduit"
        ))
        .unwrap(),
    );
    assert!(bind(&baseline, &browser, &prior).is_err());
    let mut different = prior.clone();
    different.manifest.source_identity = "source/unreviewed".into();
    let release = bundle_common::native_release(&different, b"artifact");
    assert!(bind_native_release_configuration(
        &baseline,
        &prior,
        &baseline,
        &release,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set()
    )
    .is_err());
}

#[test]
fn actual_reviewed_name_edit_produces_same_body_seed_without_prebuilt_destination_image() {
    let (mut factory, _directory) = factory();
    let initial_image = factory.inputs.artifacts[0].image.manifest.image_id.clone();
    let (mut adapter, controller) = factory
        .encounter(
            binding(),
            Box::new(Custody {
                admitted: true,
                drained: Arc::new(AtomicBool::new(false)),
            }),
        )
        .unwrap();
    start_fixture(&mut adapter);
    for (action_id, value) in [
        ("host-config/name", Some("body-authored-threebee")),
        ("host-config/check", None),
    ] {
        let face = controller.face().unwrap().unwrap();
        let show = crate::mask_test_common::available_mask_show(&face);
        let action = face
            .actions
            .iter()
            .find(|action| action.identity == action_id)
            .unwrap();
        let arguments = value
            .map(|value| {
                vec![conduit_face::FaceInteractionArgument {
                    name: "value".into(),
                    value_kind: conduit_face::UTF8_TEXT_VALUE_KIND.into(),
                    value: value.as_bytes().to_vec(),
                }]
            })
            .unwrap_or_default();
        let input =
            FaceInteraction::new(&face, &show, action_id, &action.target, arguments, 1).unwrap();
        controller.acknowledge_show(show).unwrap();
        controller.submit(input).unwrap();
        assert!(matches!(adapter.poll().unwrap(), CrechePoll::Pending));
    }
    assert!(controller.start_production_if_reviewed().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    let outcome = loop {
        match adapter.poll().unwrap() {
            CrechePoll::Outcome(outcome) => break outcome,
            CrechePoll::Pending => {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
        }
    };
    assert_eq!(
        outcome.disposition,
        CrecheDisposition::Prepared,
        "{}",
        String::from_utf8_lossy(adapter.retained_evidence(&outcome).unwrap())
    );
    let mut products = controller.take_products().unwrap().unwrap();
    assert_eq!(
        products.spore.manifest.host_entry_name,
        "body-authored-threebee"
    );
    assert_ne!(products.spore.manifest.image_id, initial_image);
    assert_eq!(
        products.spore.manifest.body_id,
        adapter.channel.binding.biography.body_id.as_str()
    );
    bundle_common::assert_seed_package(
        &mut products.artifact,
        &products.spore.manifest,
        &adapter.channel.binding.invitation,
        &products.receipt,
        b"artifact",
    );
    adapter.cancel_and_drain().unwrap();
}
