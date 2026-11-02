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
