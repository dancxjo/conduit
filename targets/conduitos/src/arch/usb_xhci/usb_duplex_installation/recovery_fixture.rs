//! Genuine selected owners; Configure Endpoint receipts are simulated below physics.
use super::*;
use crate::usb_base::{
    endpoint_read_owner::tests::fixture as read, endpoint_write_owner::tests::fixture as write,
};
use alloc::boxed::Box;

impl NativeUsbDuplexHost {
    pub(crate) fn with_recovery_fixture(plan: &conduit_core::Plan, test: impl FnOnce(Self)) {
        crate::arch::xhci_mechanism::with_synthetic_stop_controller(1, 1, 0, |controller| {
            test(Self::recovery_fixture(controller, plan));
        });
    }

    pub(crate) fn recovery_fixture(
        controller: crate::arch::xhci_mechanism::XhciReady,
        plan: &conduit_core::Plan,
    ) -> Self {
        let fragment = &plan.fragments[0];
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        let active =
            conduit_core::bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let gear = |implementation: &str| {
            fragment
                .placements
                .iter()
                .find(|g| g.implementation_id.as_str() == implementation)
                .unwrap()
        };
        let read_gear = gear(crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION);
        let write_gear =
            gear(crate::usb_base::endpoint_write_factory::ENDPOINT_WRITE_IMPLEMENTATION);
        let control_gear = gear(crate::usb_base::control_factory::CONTROL_IMPLEMENTATION);
        assert_eq!(fragment.placements.len(), 3);
        let selected =
            |placement_id| crate::machine_membrane::selected_operation::SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id,
            };
        let device =
            super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        let original_dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            (*original_dma).owner_slot = 1;
            (*original_dma).owner_root_port = 1;
            (*original_dma).owner_epoch = 1;
            (*original_dma).control_cursor =
                crate::usb_base::control_ring::ControlRingCursor::new();
        }
        let mut host = unsafe { UsbNativeHost::new(controller, device, Some) }.unwrap();
        let issue = |read_direction: bool| {
            let mut request = if read_direction {
                read::request()
            } else {
                write::request()
            };
            let (implementation, authority) = if read_direction {
                (
                    crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
                    crate::usb_base::endpoint_read_factory::ENDPOINT_READ_AUTHORITY,
                )
            } else {
                (
                    crate::usb_base::endpoint_write_factory::ENDPOINT_WRITE_IMPLEMENTATION,
                    crate::usb_base::endpoint_write_factory::ENDPOINT_WRITE_AUTHORITY,
                )
            };
            request.scope.implementation_id = implementation.into();
            request.scope.authority_contract_id = authority.into();
            request.authority.grant.contract_id = authority.into();
            request.scope.plan_id = plan.plan_id.clone();
            request.scope.active_play_id = active.active_play_id.clone();
            let scope = &request.scope;
            let mut table = conduit_core::BaseCapabilityTable::new(
                scope.host_id.clone(),
                scope.boot_id.clone(),
                scope.base_instance_id.clone(),
                scope.base_provider_generation,
                [if read_direction { 73 } else { 74 }; 32],
                1,
            )
            .unwrap();
            let handle = table.issue(request).unwrap();
            let mut claim = if read_direction {
                read::claim()
            } else {
                write::claim()
            };
            claim.implementation_id = implementation.into();
            claim.plan_id = plan.plan_id.clone();
            claim.active_play_id = active.active_play_id.clone();
            (table, handle, claim)
        };
        // These test-owned allocations are prepared before use and rooted for
        // the test process. No lifetime/Play-allocation proof is claimed here.
        let (table, handle, claim) = issue(true);
        let read_index = unsafe {
            host.add_endpoint(
                super::super::endpoint_setup::ConfiguredInboundEndpoint {
                    slot: 1,
                    device_epoch: 1,
                    endpoint_epoch: 9,
                    dci: 3,
                    ring_physical: 0x40000,
                },
                EndpointReceiveDma {
                    ring: Box::leak(Box::new([[0; 4]; 64])),
                    buffer: Box::leak(Box::new([0; 2048])),
                    cursor: Box::leak(Box::new(
                        crate::usb_base::endpoint_ring::EndpointRingCursor::new(64).unwrap(),
                    )),
                    ring_physical: 0x40000,
                    buffer_physical: 0x50000,
                },
                EndpointReadAttachment {
                    slot: 1,
                    generation: 1,
                    endpoint_generation: 9,
                    endpoint_dci: 3,
                    maximum_data_bytes: 512,
                    resource_bytes: 4096,
                },
                table,
                handle,
                claim,
                &EndpointReadContract::prepare().unwrap(),
                selected(&read_gear.placement_id),
            )
        }
        .unwrap();
        let (table, handle, claim) = issue(false);
        let write_index = unsafe {
            host.add_write(
                super::super::endpoint_setup::ConfiguredOutboundEndpoint {
                    slot: 1,
                    device_epoch: 1,
                    endpoint_epoch: 9,
                    dci: 2,
                    ring_physical: 0x60000,
                },
                EndpointTransmitDma {
                    ring: Box::leak(Box::new([[0; 4]; 64])),
                    buffer: Box::leak(Box::new([0; 2048])),
                    cursor: Box::leak(Box::new(
                        crate::usb_base::endpoint_ring::EndpointRingCursor::new(64).unwrap(),
                    )),
                    ring_physical: 0x60000,
                    buffer_physical: 0x70000,
                },
                EndpointWriteAttachment {
                    slot: 1,
                    generation: 1,
                    endpoint_generation: 9,
                    endpoint_dci: 2,
                    maximum_data_bytes: 512,
                    resource_bytes: 4096,
                },
                table,
                handle,
                claim,
                &EndpointWriteContract::prepare().unwrap(),
                selected(&write_gear.placement_id),
            )
        }
        .unwrap();
        let contract = crate::usb_base::control_contract::ControlContract::prepare().unwrap();
        let (table, handle, claim) =
            super::super::native_host::stop_ack_tests::possession::issue(plan).unwrap();
        let control_index = unsafe {
            host.add_control(
                table,
                handle,
                claim,
                UsbControlSelection {
                    contract: &contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &control_gear.placement_id,
                },
            )
        }
        .unwrap();
        host.prepare_independent().unwrap();
        host.validate_settlement_idle().unwrap();
        let node = |placement| {
            lowered
                .nodes
                .iter()
                .find(|p| &p.placement_id == placement)
                .unwrap()
                .node
        };
        Self {
            host,
            control_index,
            read_index,
            write_index,
            control_node: node(&control_gear.placement_id),
            read_node: node(&read_gear.placement_id),
            write_node: node(&write_gear.placement_id),
        }
    }
}

impl NativeUsbDuplexHost {
    pub(in crate::arch::usb_xhci) fn into_recovery_fixture(
        self,
    ) -> (
        UsbNativeHost<'static>,
        UsbNativeCallIndex,
        UsbNativeCallIndex,
    ) {
        (self.host, self.read_index, self.write_index)
    }
}
