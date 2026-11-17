//! Public selected-IMAGE denial must precede copying unbounded caller metadata.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;

use conduit_core::OfferGeneration;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, HostImage, MakeCatalog,
};
use conduit_std_host::{StdHost, StdHostConfig};

#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

const FRAME: usize = 64 * 1024;
const SOURCE: &str = include_str!("../profiles/raspberry-pi-3-tick-workset.host.conduit");

fn config() -> StdHostConfig {
    StdHostConfig {
        host_id: "fixture/preparation".into(),
        boot_id: "boot/preparation".into(),
        offer_generation: OfferGeneration(1),
    }
}

fn fixture() -> (HostImage, MakeCatalog) {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(SOURCE).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/selected-timer-allocation".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    // The unmodified checked public profile genuinely initializes successfully.
    assert!(StdHost::from_selected_timer_image(config(), &image, &catalog).is_ok());
    (image, catalog)
}

fn assert_bounded_refusal(image: &HostImage, catalog: &MakeCatalog, config: StdHostConfig) {
    let (result, allocation) =
        allocation_probe::observe(|| StdHost::from_selected_timer_image(config, image, catalog));
    let error = result.err().expect("oversized preparation must refuse");
    assert!(error.contains("finite preparation frame"), "{error}");
    assert!(
        allocation.requested_bytes < 2 * FRAME,
        "denial copied oversized input: {allocation:?}"
    );
}

#[test]
fn oversized_matching_timer_budget_ids_refuse_before_cloning() {
    let (mut image, catalog) = fixture();
    let huge = "b".repeat(512 * 1024);
    image.manifest.resource_budgets[0].id = huge.clone();
    image.payload.resource_budgets[0].id = huge;
    assert_bounded_refusal(&image, &catalog, config());
}

#[test]
fn oversized_matching_timer_base_ids_refuse_before_cloning() {
    let (mut image, catalog) = fixture();
    let huge = "b".repeat(512 * 1024);
    let manifest = image
        .manifest
        .base_selections
        .iter_mut()
        .find(|base| base.kind == "timer/monotonic")
        .unwrap();
    manifest.id = huge.clone();
    let payload = image
        .payload
        .base_selections
        .iter_mut()
        .find(|base| base.kind == "timer/monotonic")
        .unwrap();
    payload.id = huge;
    assert_bounded_refusal(&image, &catalog, config());
}

#[test]
fn oversized_host_and_boot_refuse_before_derived_identity_allocation() {
    let (image, catalog) = fixture();
    for oversized_host in [true, false] {
        let mut config = config();
        let huge = "i".repeat(512 * 1024);
        if oversized_host {
            config.host_id = huge.into();
        } else {
            config.boot_id = huge.into();
        }
        assert_bounded_refusal(&image, &catalog, config);
    }
}

#[test]
fn image_and_identity_share_one_frame_even_when_each_individually_fits() {
    let (mut image, catalog) = fixture();
    let budget_id = "b".repeat(FRAME / 8);
    image.manifest.resource_budgets[0].id = budget_id.clone();
    image.payload.resource_budgets[0].id = budget_id;
    let mut config = config();
    config.host_id = "i".repeat(3 * FRAME / 4).into();
    let image_bytes = serde_json::to_vec(&image).unwrap().len();
    let identity_bytes = config.host_id.as_str().len() + config.boot_id.as_str().len();
    assert!(image_bytes < FRAME);
    assert!(identity_bytes + 128 < FRAME);
    assert!(image_bytes + identity_bytes > FRAME);
    assert_bounded_refusal(&image, &catalog, config);
}

#[test]
fn empty_identities_and_zero_generation_refuse_before_input_preparation() {
    let (image, catalog) = fixture();
    for invalid in 0..3 {
        let mut config = config();
        match invalid {
            0 => config.host_id = " ".into(),
            1 => config.boot_id = "".into(),
            _ => config.offer_generation = OfferGeneration(0),
        }
        let (result, allocation) = allocation_probe::observe(|| {
            StdHost::from_selected_timer_image(config, &image, &catalog)
        });
        let error = result.err().expect("invalid identity must refuse");
        assert!(error.contains("nonempty Host/Boot"), "{error}");
        assert!(allocation.requested_bytes < 1024, "{allocation:?}");
    }
}
