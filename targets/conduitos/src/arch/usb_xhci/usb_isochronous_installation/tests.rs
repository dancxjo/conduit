use super::*;
use crate::usb_base::{
    control_contract::{CONTROL_CALL, CONTROL_MAXIMUM_BYTES},
    control_factory::CONTROL_IMPLEMENTATION,
    control_proof_plan::{self as planning, ControlProofSubject},
};
#[path = "../../../usb_base/isochronous/owner/tests/fixture.rs"]
mod iso_fixture;
#[path = "../usb_control_kernel_proof/possession.rs"]
mod possession;
fn mapping(address: u64) -> Option<u64> {
    Some(address)
}

#[repr(C, align(65536))]
struct FixtureDma {
    ring: [[u32; 4]; 64],
    padding: [u8; 1024],
    wire: [u8; 2048],
    cursor: crate::usb_base::endpoint_ring::EndpointRingCursor,
}

#[test]
fn full_native_preparation_retained_storage_is_measured_with_allocator_rounding() {
    crate::arch::xhci_mechanism::with_synthetic_stop_controller(1, 1, 0, |controller| {
        let contract = ControlContract::prepare().unwrap();
        let plan = planning::plan(
            &contract,
            &ControlProofSubject {
                host_id: "host/iso-storage",
                boot_id: "boot/iso-storage",
                controller_base_id: "controller/iso-storage",
                device_instance_id: "device/iso-storage",
                root_port: 1,
                slot: 1,
                attachment_epoch: 1,
            },
        )
        .unwrap();
        let (table, handle, claim) = possession::issue(&plan).unwrap();
        validate_preissued_metadata(&table, &claim).unwrap();
        let fragment = &plan.fragments[0];
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let device =
            super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        let global = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            (*global).owner_slot = 1;
            (*global).owner_root_port = 1;
            (*global).owner_epoch = 1;
            (*global).control_cursor = crate::usb_base::control_ring::ControlRingCursor::new();
        }
        // This synthetic resource is prepared outside the trace, as static
        // DMA is in the product. It is never submitted to hardware.
        let storage = alloc::boxed::Box::leak(alloc::boxed::Box::new(FixtureDma {
            ring: [[0; 4]; 64],
            padding: [0; 1024],
            wire: [0; 2048],
            cursor: crate::usb_base::endpoint_ring::EndpointRingCursor::with_maximum_pending(64, 8)
                .unwrap(),
        }));
        let ring_physical = storage.ring.as_ptr() as u64;
        let buffer_physical = storage.wire.as_ptr() as u64;
        let configured = ConfiguredIsochronousOutboundEndpoint {
            configured: super::super::endpoint_setup::ConfiguredOutboundEndpoint {
                slot: 1,
                device_epoch: 1,
                endpoint_epoch: 9,
                dci: 2,
                ring_physical,
            },
            geometry: crate::usb_base::isochronous_schedule::IsochronousOutGeometry::usb2(
                1, 256, 1,
            )
            .unwrap(),
        };
        let dma = EndpointTransmitDma {
            ring: &mut storage.ring,
            buffer: &mut storage.wire,
            cursor: &mut storage.cursor,
            ring_physical,
            buffer_physical,
        };
        let ((host, owner), bytes) = crate::test_allocations::rounded_retained_bytes(|| {
            let mut host = unsafe { UsbNativeHost::new(controller, device, mapping) }.unwrap();
            unsafe {
                host.add_control(
                    table,
                    handle,
                    claim,
                    UsbControlSelection {
                        contract: &contract,
                        fragment,
                        lowered: &lowered,
                        active: &active,
                        placement: &fragment.placements[0].placement_id,
                    },
                )
            }
            .unwrap();
            unsafe { host.add_isochronous(configured, dma) }.unwrap();
            let owner = iso_fixture::owner(88);
            (host, owner)
        });
        let charge = retained_resource_bytes(bytes).unwrap();
        println!(
            "ISO full native retained rounded metadata={bytes}, complete conservative resource charge={charge}, admitted={ISOCHRONOUS_OUT_RESOURCE_BYTES}"
        );
        assert!(
            charge <= ISOCHRONOUS_OUT_RESOURCE_BYTES as usize,
            "full resource envelope must include all retained metadata"
        );
        assert!(
            bytes > 60570,
            "control/native call storage must be counted beyond encoder alone"
        );
        drop((host, owner));
        // No TD was ever submitted; explicit synthetic fixture teardown.
        unsafe {
            (*global).owner_slot = 0;
        }
    });
}

#[test]
fn resource_charge_includes_rounded_metadata_original_tables_inline_and_dma() {
    assert_eq!(
        retained_resource_bytes(0),
        Some(2 * 16384 + core::mem::size_of::<NativeUsbIsochronousHost>() + 8192 + 4 * 4096)
    );
    assert_eq!(retained_resource_bytes(usize::MAX), None);
    let result = retained_resource_bytes(60570).unwrap();
    println!(
        "ISO encoder-only lower bound charge={result}, native Host inline={}",
        core::mem::size_of::<NativeUsbIsochronousHost>()
    );
    assert!(result <= ISOCHRONOUS_OUT_RESOURCE_BYTES as usize);
}

#[test]
fn retained_resource_gate_accepts_exact_charge_and_refuses_one_byte_less() {
    let charge = retained_resource_bytes(107872).unwrap();
    assert_eq!(
        admit_retained_resource(107872, charge as u64).unwrap(),
        charge
    );
    assert!(matches!(
        admit_retained_resource(107872, charge as u64 - 1),
        Err(UsbIsochronousCallRefusal::Binding)
    ));
    assert!(matches!(
        admit_retained_resource(usize::MAX, u64::MAX),
        Err(UsbIsochronousCallRefusal::Binding)
    ));
}
