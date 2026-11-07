//! Explicit CPU-driver fixtures; these tests prove no physical display output.
use super::*;
use crate::display::DisplayFormat;
use alloc::vec;
use conduit_core::{
    BaseImplementationId, BaseInstanceId, HostBaseId, HostBaseKindId, ResourceClassId,
    ResourceOffer, ResourcePoolId,
};

struct CpuDriver {
    live: bool,
    format: DisplayFormat,
    writes: usize,
    commits: usize,
}

impl CpuDriver {
    fn initialized() -> Self {
        Self {
            live: true,
            format: DisplayFormat {
                width: 8,
                height: 8,
                pitch: 32,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            },
            writes: 0,
            commits: 0,
        }
    }
}

impl PixelTarget for CpuDriver {
    fn format(&self) -> DisplayFormat {
        self.format
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        self.writes += 1;
        Ok(())
    }
    fn commit_frame(&mut self) -> Result<(), DisplayError> {
        self.commits += 1;
        Ok(())
    }
}
impl InitializedNativeOutput for CpuDriver {
    fn require_live(&self) -> Result<(), DisplayError> {
        if self.live {
            Ok(())
        } else {
            Err(DisplayError::Lost)
        }
    }
}

fn entry() -> BaseProviderEntry {
    BaseProviderEntry {
        base_id: HostBaseId::from("fixture/display"),
        provider_instance_id: BaseInstanceId::from("fixture/display/1"),
        provider_generation: 1,
        implementation_id: BaseImplementationId::from("fixture/cpu@1"),
        mechanism_family: HostBaseKindId::from("fixture/display@1"),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![ResourceOffer {
            pool_id: ResourcePoolId::from("fixture/display/show"),
            class_id: ResourceClassId::from(conduit_face::SHOW_RESOURCE_CLASS),
            capacity_units: 1,
            compute: None,
            content: None,
        }],
    }
}

#[test]
fn cooperative_driver_bridge_is_inert_and_retains_private_issuer() {
    let mut output = CpuDriver::initialized();
    let descriptor = entry();
    let provider =
        NativeSurfaceProvider::from_initialized_output(descriptor.clone(), [9; 32], &mut output)
            .expect("initialized cooperative CPU driver");
    assert_eq!(provider.entry, descriptor);
    assert_eq!(provider.issuer_key, [9; 32]);
    assert_eq!((output.writes, output.commits), (0, 0));
}

#[test]
fn absent_live_driver_or_invalid_format_never_creates_provider() {
    let mut output = CpuDriver::initialized();
    output.live = false;
    assert!(matches!(
        NativeSurfaceProvider::from_initialized_output(entry(), [9; 32], &mut output),
        Err(SurfaceProviderError::Display(DisplayError::Lost))
    ));
    output.live = true;
    output.format.width = 0;
    assert!(matches!(
        NativeSurfaceProvider::from_initialized_output(entry(), [9; 32], &mut output),
        Err(SurfaceProviderError::Display(DisplayError::InvalidExtent))
    ));
    assert_eq!((output.writes, output.commits), (0, 0));
}

#[test]
fn invalid_issuer_and_descriptor_are_refused_without_effects() {
    let mut output = CpuDriver::initialized();
    assert!(matches!(
        NativeSurfaceProvider::from_initialized_output(entry(), [0; 32], &mut output),
        Err(SurfaceProviderError::InvalidIssuerKey)
    ));
    let mut invalid = vec![];
    let mut candidate = entry();
    candidate.provider_generation = 0;
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.base_id = HostBaseId::from("");
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.lifecycle = BaseLifecycle::Lost;
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.enforcement_class = BaseEnforcementClass::ConduitOsKernelEnforced;
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.resources.clear();
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.resources.push(candidate.resources[0].clone());
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.resources[0].capacity_units = 2;
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.resources[0].pool_id = ResourcePoolId::from("");
    invalid.push(candidate);
    let mut candidate = entry();
    candidate.resources[0].class_id = ResourceClassId::from("fixture/not-show");
    invalid.push(candidate);
    for candidate in invalid {
        assert!(matches!(
            NativeSurfaceProvider::from_initialized_output(candidate, [9; 32], &mut output),
            Err(SurfaceProviderError::InvalidProvider)
        ));
    }
    assert_eq!((output.writes, output.commits), (0, 0));
}

#[test]
fn same_base_retains_physical_surface_and_exact_show_pool_in_either_order() {
    for show_first in [false, true] {
        let mut output = CpuDriver::initialized();
        let mut descriptor = entry();
        descriptor.resources.push(conduit_core::resource_offer(
            "fixture/display/raster",
            "face/surface",
            1,
        ));
        if !show_first {
            descriptor.resources.reverse();
        }
        let provider = NativeSurfaceProvider::from_initialized_output(
            descriptor.clone(),
            [9; 32],
            &mut output,
        )
        .unwrap();
        assert_eq!(provider.entry, descriptor);
        assert_eq!(
            exact_show_resource(&provider.entry)
                .unwrap()
                .pool_id
                .as_str(),
            "fixture/display/show"
        );
        let host = crate::product_bases::native_mask_host_advertisement(
            &"fixture/host".into(),
            &"fixture/boot".into(),
            1,
            &provider,
        );
        for resource in &descriptor.resources {
            assert!(host.resources.contains(resource));
        }
        assert_eq!(
            host.bases[0].resource_pool_ids,
            descriptor
                .resources
                .iter()
                .map(|resource| resource.pool_id.clone())
                .collect::<alloc::vec::Vec<_>>()
        );
        let plan = conduit_conduitos_mask_offer::prepare_stage_from_offer(
            conduit_conduitos_mask_offer::Adapter::Native,
            &host,
        )
        .unwrap()
        .planned_mask;
        let connection = plan
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| connection.resource.is_some())
            .unwrap();
        assert_eq!(
            connection
                .resource
                .as_ref()
                .unwrap()
                .source_binding
                .pool_id
                .as_str(),
            "fixture/display/show"
        );
        let active =
            conduit_core::bind_active_play(&plan.plan.plan_id, &host.host_id, &host.boot_id, 1);
        let mut possession =
            crate::native_surface_possession::ActiveNativeSurfacePossession::begin(
                &provider,
                &plan.plan,
                active.active_play_id,
            )
            .unwrap();
        possession.authorize().unwrap();
        assert_eq!(
            possession
                .complete_and_retire()
                .unwrap()
                .resource_pool_id
                .as_str(),
            "fixture/display/show"
        );
        assert_eq!((output.writes, output.commits), (0, 0));
    }
}

#[test]
fn additional_resources_cannot_hide_duplicate_show_or_invalid_inventory() {
    let mut output = CpuDriver::initialized();
    let mut invalid = alloc::vec::Vec::new();
    let mut duplicate_show = entry();
    duplicate_show.resources.push(conduit_core::resource_offer(
        "fixture/another-show",
        conduit_face::SHOW_RESOURCE_CLASS,
        1,
    ));
    invalid.push(duplicate_show);
    for (pool, class, capacity) in [
        ("", "face/surface", 1),
        ("fixture/raster", "", 1),
        ("fixture/raster", "face/surface", 0),
        ("fixture/display/show", "face/surface", 1),
    ] {
        let mut descriptor = entry();
        descriptor
            .resources
            .push(conduit_core::resource_offer(pool, class, capacity));
        invalid.push(descriptor);
    }
    for descriptor in invalid {
        assert!(matches!(
            NativeSurfaceProvider::from_initialized_output(descriptor, [9; 32], &mut output),
            Err(SurfaceProviderError::InvalidProvider)
        ));
    }
    assert_eq!((output.writes, output.commits), (0, 0));
}

#[cfg(feature = "native-compositor-core")]
#[test]
fn multi_resource_provider_completes_actual_cpu_mask_scanout() {
    use crate::native_face_mask::NativeFaceMask;
    let mut output = CpuDriver::initialized();
    output.format.width = 640;
    output.format.height = 480;
    output.format.pitch = 2560;
    let mut descriptor = entry();
    descriptor.resources.insert(
        0,
        conduit_core::resource_offer("fixture/raster", "face/surface", 1),
    );
    let provider =
        NativeSurfaceProvider::from_initialized_output(descriptor, [9; 32], &mut output).unwrap();
    let mut mask = NativeFaceMask::prepare(
        "fixture/host".into(),
        "fixture/boot".into(),
        conduit_core::OfferGeneration(1),
        "fixture/build",
        provider.entry.base_id.clone(),
        "fixture/surface",
        &provider,
    )
    .unwrap();
    let face = crate::native_face_mask::tests::draft()
        .face(&mask.birth_basis("fixture/multi-resource"))
        .unwrap();
    mask.present_read_only(face, 1, 1, &mut output).unwrap();
    assert!(output.writes > 0);
    // Initial presentation and focus repaint may commit separate frames. The
    // contract is a real successful commit before Available, not one upload.
    assert!(output.commits > 0);
    assert_eq!(
        mask.show().unwrap().show.lifecycle,
        conduit_face::ManifestationLifecycle::Available
    );
    let receipt = mask.render_receipt().unwrap();
    assert!(receipt.scanout_frame_sequence > 0);
    assert!(receipt.scanout_pixels_written > 0);
    assert_eq!(receipt.display_base_id, provider.entry.base_id);
}

#[test]
fn supplied_provider_without_show_does_not_gain_a_synthetic_pool() {
    let mut descriptor = entry();
    descriptor.resources[0].class_id = "face/surface".into();
    let host = conduit_conduitos_mask_offer::native_host_advertisement(
        &"fixture/host".into(),
        &"fixture/boot".into(),
        1,
        &descriptor,
    );
    assert!(
        !host
            .resources
            .iter()
            .any(|resource| resource.class_id.as_str() == conduit_face::SHOW_RESOURCE_CLASS)
    );
    assert!(
        conduit_conduitos_mask_offer::prepare_stage_from_offer(
            conduit_conduitos_mask_offer::Adapter::Native,
            &host
        )
        .is_err()
    );
}
