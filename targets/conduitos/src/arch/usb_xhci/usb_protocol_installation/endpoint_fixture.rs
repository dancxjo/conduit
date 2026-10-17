//! Original controller/owner fixture; Configure Endpoint is simulated, below physics.
use super::*;
use crate::usb_base::endpoint_ring::EndpointRingCursor;
use alloc::{boxed::Box, format};
#[repr(C, align(4096))]
struct Dma {
    ring: [[u32; 4]; 64],
    wire: [u8; 2048],
    cursor: EndpointRingCursor,
}
impl ReadyUsbProtocolHost {
    pub(crate) fn with_endpoint_fixture(
        test: impl FnOnce(Self, BaseCapabilityAuthority, BaseCapabilityAuthority),
    ) {
        crate::arch::xhci_mechanism::with_synthetic_stop_controller(1, 1, 0, |controller| {
            let authority = |control: bool| {
                let mut value =
                    crate::usb_base::endpoint_read_owner::tests::fixture::request().authority;
                let (implementation, kind, call, contract, label) = if control {
                    (
                        crate::usb_base::control_factory::CONTROL_IMPLEMENTATION,
                        "machine/usb/control",
                        crate::usb_base::control_contract::CONTROL_CALL,
                        crate::usb_base::control_factory::CONTROL_AUTHORITY,
                        "control",
                    )
                } else {
                    (
                        ENDPOINT_READ_IMPLEMENTATION,
                        "machine/usb/endpoint-read",
                        crate::usb_base::endpoint_read_contract::ENDPOINT_READ_CALL,
                        ENDPOINT_READ_AUTHORITY,
                        "read",
                    )
                };
                value.grant.host_id = "fixture/endpoint-only".into();
                value.grant.boot_id = "fixture/endpoint-only-boot".into();
                value.grant.capability_id = implementation.into();
                value.grant.subject_kind = kind.into();
                value.grant.host_call_contract_id = call.into();
                value.grant.contract_id = contract.into();
                value.grant.grant_id = format!("fixture/endpoint-only/{label}/grant").into();
                value.base_instance_id = format!("fixture/endpoint-only/{label}/provider").into();
                value.base_provider_generation = 1;
                value.resource_pool_id = format!("fixture/endpoint-only/{label}/pool").into();
                value.resource_generation_id =
                    ResourceGenerationId(format!("fixture/endpoint-only/{label}/generation"));
                value.operation_contract_id = call.into();
                value.envelope_id = format!("fixture/endpoint-only/{label}/envelope").into();
                value.maximum_work_units = 1;
                value.maximum_operations = 1;
                value
            };
            let control = authority(true);
            let endpoint = authority(false);
            let identity = |a: &BaseCapabilityAuthority, label| UsbNativeIdentity {
                host_id: a.grant.host_id.clone(),
                boot_id: a.grant.boot_id.clone(),
                base_id: format!("fixture/endpoint-only/{label}/base").into(),
                provider_instance_id: a.base_instance_id.clone(),
                provider_generation: 1,
                resource_pool_id: a.resource_pool_id.clone(),
                resource_generation_id: a.resource_generation_id.clone(),
                envelope_id: a.envelope_id.clone(),
                artifact_id: "fixture/endpoint-only-artifact".into(),
            };
            let device =
                super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
            let device_dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
            unsafe {
                (*device_dma).owner_slot = 1;
                (*device_dma).owner_root_port = 1;
                (*device_dma).owner_epoch = 1;
            }
            let mut dma = Box::new(Dma {
                ring: [[0; 4]; 64],
                wire: [0; 2048],
                cursor: EndpointRingCursor::new(64).unwrap(),
            });
            let pointer = dma.as_mut() as *mut Dma;
            let ring = dma.ring.as_ptr() as u64;
            let wire = dma.wire.as_ptr() as u64;
            // SAFETY: callback cannot return the host. This owning Box outlives
            // original-host cancellation; projections remain private to the test.
            let ready = unsafe {
                Self::new(
                    identity(&control, "control"),
                    identity(&endpoint, "read"),
                    controller,
                    device,
                    Some,
                    ConfiguredInboundEndpoint {
                        slot: 1,
                        device_epoch: 1,
                        endpoint_epoch: 9,
                        dci: 3,
                        ring_physical: ring,
                    },
                    EndpointReceiveDma {
                        ring: &mut (*pointer).ring,
                        buffer: &mut (*pointer).wire,
                        cursor: &mut (*pointer).cursor,
                        ring_physical: ring,
                        buffer_physical: wire,
                    },
                    EndpointReadAttachment {
                        slot: 1,
                        generation: 1,
                        endpoint_generation: 9,
                        endpoint_dci: 3,
                        maximum_data_bytes: 512,
                        resource_bytes: 4096,
                    },
                )
            }
            .unwrap();
            test(ready, control, endpoint);
            // A submitted original TD requires acknowledged retirement before
            // the owning fixture allocation can drop. A pre-effect refusal has
            // no submitted TD and can release its unbound preparation marker.
            if dma.ring[0][3] != 0 {
                assert_eq!(unsafe { (*device_dma).owner_slot }, 0);
            } else {
                unsafe {
                    (*device_dma).owner_slot = 0;
                }
            }
            drop(dma);
        });
    }
}
