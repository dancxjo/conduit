#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("#588 currently promotes only the executable x86_64 ConduitOS backend");

#[cfg(target_os = "none")]
use core::panic::PanicInfo;

#[cfg(target_os = "none")]
use alloc::format;

#[cfg(target_os = "none")]
use conduitos::{allocation::BOOT_ARENA, arch, boot, dual_region_plan, identity, proof};

#[cfg(target_os = "none")]
const BUILD_ID: &str = env!("CONDUITOS_BUILD_ID");
#[cfg(target_os = "none")]
const IMAGE_ID: &str = env!("CONDUITOS_IMAGE_ID");

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
extern "C" fn conduitos_start() -> ! {
    match boot::normalize_boot() {
        Ok(record) => {
            arch::early_write(b"CONDUIT_BOOT_STAGE xhci-start\n");
            let mut xhci = match arch::initialize_xhci(
                record.hhdm_offset,
                boot::executable_physical_address,
            ) {
                Ok(ready) => ready,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE xhci-ready\n");
            arch::early_write(b"CONDUIT_BOOT_STAGE usb-enumeration-start\n");
            let usb = match arch::enumerate_usb(&mut xhci, boot::executable_physical_address) {
                Ok(device) => device,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE usb-configured\n");
            let Some(arena_virtual_start) = record
                .hhdm_offset
                .checked_add(record.runtime_arena.physical_start)
                .and_then(|value| usize::try_from(value).ok())
            else {
                emit_refusal("runtime-arena-address-invalid");
            };
            if unsafe {
                BOOT_ARENA.initialize(
                    arena_virtual_start,
                    usize::try_from(record.runtime_arena.length).unwrap_or(0),
                )
            }
            .is_err()
            {
                emit_refusal("runtime-arena-initialization-failed");
            }
            let entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
            let identities =
                identity::derive(entropy, record.timestamp, record.image_physical_start);
            let xhci_base =
                identity::derive_base(&identities.boot, "conduitos/xhci/0000:00:01.0/1b36:000d");
            let xhci_base_id = identity::hex(&xhci_base);
            let xhci_sign = format!(
                "CONDUIT_XHCI_SIGN {{\"schema\":\"conduit.conduitos.xhci-base/v1\",\"status\":\"ready\",\"proof_class\":\"freestanding-emulator\",\"base_id\":\"{}\",\"boot_id\":\"{}\",\"segment\":{},\"bus\":{},\"device\":{},\"function\":{},\"vendor\":{},\"device_id\":{},\"bar_physical\":{},\"hardware_slots\":{},\"admitted_slots\":{},\"command_trbs\":{},\"event_trbs\":{},\"dma_bytes\":{},\"dma_alignment\":{},\"maximum_pending_commands\":{},\"poll_steps\":{},\"sign_slots\":{},\"semantic_keyboard_offer\":false}}\n",
                xhci_base_id,
                identity::hex(&identities.boot),
                xhci.segment,
                xhci.bus,
                xhci.device,
                xhci.function,
                xhci.vendor,
                xhci.device_id,
                xhci.bar_physical,
                xhci.hardware_slots,
                xhci.admitted_slots,
                xhci.command_trbs,
                xhci.event_trbs,
                xhci.dma_bytes,
                xhci.dma_alignment,
                xhci.maximum_pending_commands,
                xhci.poll_steps,
                xhci.sign_slots,
            );
            arch::early_write(xhci_sign.as_bytes());
            let device_id = identity::derive_usb_device(
                &identities.boot,
                &xhci_base,
                usb.root_port,
                usb.slot,
                usb.attachment_epoch,
            );
            let first_interface = usb.interfaces[0];
            let interface_id = identity::derive_usb_interface(
                &device_id,
                first_interface.number,
                first_interface.alternate_setting,
            );
            let first_endpoint = usb.endpoints[0];
            let endpoint_id = identity::derive_usb_endpoint(&interface_id, first_endpoint.address);
            let usb_sign = format!(
                "CONDUIT_USB_SIGN {{\"schema\":\"conduit.conduitos.usb-device/v1\",\"status\":\"configured\",\"proof_class\":\"freestanding-emulator\",\"controller_base_id\":\"{}\",\"boot_id\":\"{}\",\"device_instance_id\":\"{}\",\"root_port\":{},\"slot\":{},\"address\":{},\"attachment_epoch\":{},\"usb_version\":{},\"device_class\":{},\"device_subclass\":{},\"device_protocol\":{},\"ep0_maximum_packet_size\":{},\"vendor_id\":{},\"product_id\":{},\"device_version\":{},\"configuration_value\":{},\"configuration_bytes\":{},\"descriptor_records\":{},\"interface_count\":{},\"endpoint_count\":{},\"first_interface_id\":\"{}\",\"first_interface_number\":{},\"first_interface_alternate\":{},\"first_interface_class\":{},\"first_interface_subclass\":{},\"first_interface_protocol\":{},\"first_endpoint_id\":\"{}\",\"first_endpoint_address\":{},\"first_endpoint_direction_in\":{},\"first_endpoint_transfer_type\":{},\"first_endpoint_maximum_packet_size\":{},\"first_endpoint_interval\":{},\"configuration_limit_bytes\":{},\"interface_limit\":{},\"endpoint_limit\":{},\"descriptor_record_limit\":{},\"outstanding_control_transfer_limit\":{},\"enumeration_retries\":{},\"control_transfers\":{},\"short_packets\":{},\"transfer_trbs\":{},\"dma_bytes\":{},\"dma_alignment\":{},\"port_poll_steps\":{},\"sign_slots\":{},\"semantic_keyboard_offer\":false}}\n",
                xhci_base_id,
                identity::hex(&identities.boot),
                identity::hex(&device_id),
                usb.root_port,
                usb.slot,
                usb.address,
                usb.attachment_epoch,
                usb.usb_version,
                usb.device_class,
                usb.device_subclass,
                usb.device_protocol,
                usb.ep0_maximum_packet_size,
                usb.vendor_id,
                usb.product_id,
                usb.device_version,
                usb.configuration_value,
                usb.configuration_bytes,
                usb.descriptor_records,
                usb.interface_count,
                usb.endpoint_count,
                identity::hex(&interface_id),
                first_interface.number,
                first_interface.alternate_setting,
                first_interface.class,
                first_interface.subclass,
                first_interface.protocol,
                identity::hex(&endpoint_id),
                first_endpoint.address,
                first_endpoint.direction_in,
                first_endpoint.transfer_type,
                first_endpoint.maximum_packet_size,
                first_endpoint.interval,
                usb.configuration_limit_bytes,
                usb.interface_limit,
                usb.endpoint_limit,
                usb.descriptor_record_limit,
                usb.outstanding_control_transfer_limit,
                usb.enumeration_retries,
                usb.control_transfers,
                usb.short_packets,
                usb.transfer_trbs,
                usb.dma_bytes,
                usb.dma_alignment,
                usb.port_poll_steps,
                usb.sign_slots,
            );
            arch::early_write(usb_sign.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE hid-start\n");
            let hid_ready = match arch::prepare_boot_keyboard(
                &mut xhci,
                &usb,
                boot::executable_physical_address,
            ) {
                Ok(ready) => ready,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let offer = match conduitos::offer::HostOffer::new(
                &identities,
                BUILD_ID,
                arch::feature_basis(),
                record.runtime_arena.length,
            )
            .with_keyboard(
                conduitos::keyboard_offer::KeyboardRealization {
                    controller_id: xhci_base,
                    device_id,
                    interface_id,
                    endpoint_id,
                    report_buffers: hid_ready.report_buffers,
                    transition_slots: hid_ready.transition_slots,
                    operation_slots: hid_ready.operation_slots,
                },
                BUILD_ID,
            ) {
                Ok(offer) => offer,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let keyboard_prepared =
                match conduitos::keyboard_plan::prepare(&identities, &offer, BUILD_ID) {
                    Ok(prepared) => prepared,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-offer-ready\n");
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-plan-ready\n");
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-play-started\n");
            let hid = match arch::receive_boot_keyboard(&mut xhci, &usb, hid_ready) {
                Ok(proof) => proof,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let portable_values = [
                match conduitos::keyboard_bridge::portable_key_event(
                    hid.transitions[0].usage,
                    hid.transitions[0].pressed,
                    hid.transitions[0].modifiers,
                ) {
                    Ok(value) => value,
                    Err(_) => emit_machine_refusal("keyboard-portable-value-invalid"),
                },
                match conduitos::keyboard_bridge::portable_key_event(
                    hid.transitions[1].usage,
                    hid.transitions[1].pressed,
                    hid.transitions[1].modifiers,
                ) {
                    Ok(value) => value,
                    Err(_) => emit_machine_refusal("keyboard-portable-value-invalid"),
                },
            ];
            let keyboard_report =
                match conduitos::keyboard_play::run(&keyboard_prepared, portable_values) {
                    Ok(report) => report,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
            let hid_sign = format!(
                "CONDUIT_HID_SIGN {{\"schema\":\"conduit.conduitos.hid-boot-keyboard/v1\",\"status\":\"transitions-observed\",\"proof_class\":\"freestanding-emulator\",\"controller_base_id\":\"{}\",\"boot_id\":\"{}\",\"device_instance_id\":\"{}\",\"interface_id\":\"{}\",\"endpoint_id\":\"{}\",\"interface_number\":{},\"endpoint_address\":{},\"endpoint_dci\":{},\"endpoint_maximum_packet_size\":{},\"endpoint_interval\":{},\"set_protocol_transfers\":{},\"interrupt_transfers\":{},\"report_bytes\":{},\"report_buffers\":{},\"maximum_outstanding_interrupt_transfers\":{},\"maximum_transitions_per_report\":{},\"transfer_trbs\":{},\"dma_bytes\":{},\"dma_alignment\":{},\"sign_slots\":{},\"interrupt_poll_windows\":{},\"transition_count\":{},\"first_usage_page\":\"keyboard-keypad\",\"first_usage\":{},\"first_state\":\"{}\",\"first_modifiers\":{},\"second_usage_page\":\"keyboard-keypad\",\"second_usage\":{},\"second_state\":\"{}\",\"second_modifiers\":{},\"layout_translation\":false,\"unicode_translation\":false,\"semantic_keyboard_offer\":false}}\n",
                xhci_base_id,
                identity::hex(&identities.boot),
                identity::hex(&device_id),
                identity::hex(&interface_id),
                identity::hex(&endpoint_id),
                hid.interface_number,
                hid.endpoint_address,
                hid.endpoint_dci,
                hid.endpoint_maximum_packet_size,
                hid.endpoint_interval,
                hid.set_protocol_transfers,
                hid.interrupt_transfers,
                hid.report_bytes,
                hid.report_buffers,
                hid.maximum_outstanding_interrupt_transfers,
                hid.maximum_transitions_per_report,
                hid.transfer_trbs,
                hid.dma_bytes,
                hid.dma_alignment,
                hid.sign_slots,
                hid.interrupt_poll_windows,
                hid.transition_count,
                hid.transitions[0].usage,
                if hid.transitions[0].pressed {
                    "pressed"
                } else {
                    "released"
                },
                hid.transitions[0].modifiers,
                hid.transitions[1].usage,
                if hid.transitions[1].pressed {
                    "pressed"
                } else {
                    "released"
                },
                hid.transitions[1].modifiers,
            );
            arch::early_write(hid_sign.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE hid-transitions\n");
            let keyboard_sign = format!(
                "CONDUIT_KEYBOARD_SIGN {{\"schema\":\"conduit.conduitos.keyboard-offer/v1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"offer_generation\":{},\"kind\":\"input/keyboard\",\"contract_revision\":\"{}\",\"implementation\":\"{}\",\"execution_profile\":\"{}\",\"artifact_build\":\"{}\",\"controller_base_id\":\"{}\",\"device_instance_id\":\"{}\",\"interface_id\":\"{}\",\"endpoint_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"resource_bindings\":{},\"report_buffers\":{},\"transition_slots\":{},\"operation_slots\":{},\"cord_item_capacity\":{},\"cord_byte_capacity\":{},\"event_count\":2,\"first_value\":[{},{},{}],\"second_value\":[{},{},{}],\"semantic_usb_facts\":false,\"layout_translation\":false,\"unicode_translation\":false,\"completed\":{}}}\n",
                identity::hex(&identities.host),
                identity::hex(&identities.boot),
                offer.generation,
                conduit_std_catalog::KEYBOARD_CONTRACT_REVISION,
                conduitos::keyboard_offer::KEYBOARD_IMPLEMENTATION,
                conduitos::keyboard_offer::KEYBOARD_EXECUTION_PROFILE,
                BUILD_ID,
                xhci_base_id,
                identity::hex(&device_id),
                identity::hex(&interface_id),
                identity::hex(&endpoint_id),
                keyboard_prepared.plan.plan_id.as_str(),
                keyboard_prepared.active_play.active_play_id.as_str(),
                keyboard_prepared.plan.fragments[0].placements[0]
                    .resources
                    .len(),
                hid_ready.report_buffers,
                hid_ready.transition_slots,
                hid_ready.operation_slots,
                keyboard_report.cord_item_capacity,
                keyboard_report.cord_byte_capacity,
                keyboard_report.values[0].encode()[0],
                keyboard_report.values[0].encode()[1],
                keyboard_report.values[0].encode()[2],
                keyboard_report.values[1].encode()[0],
                keyboard_report.values[1].encode()[1],
                keyboard_report.values[1].encode()[2],
                keyboard_report.completed,
            );
            arch::early_write(keyboard_sign.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-completed\n");
            match proof::accepted(&record, &identities, BUILD_ID, IMAGE_ID) {
                Ok(sign) => {
                    arch::early_write(sign.as_bytes());
                    arch::early_write(b"CONDUIT_BOOT_STAGE identities\n");
                }
                Err(_) => emit_refusal("boot-sign-storage-full"),
            }
            if let Err(error) = offer.validate() {
                emit_machine_refusal(error.as_str());
            }
            arch::early_write(b"CONDUIT_BOOT_STAGE offer\n");
            let mut prepared = match dual_region_plan::prepare(&identities, &offer, BUILD_ID) {
                Ok(prepared) => prepared,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE plan\n");
            let observatory_export = match conduitos::observatory::prepare_export(
                &record,
                &identities,
                &offer,
                &prepared,
                BUILD_ID,
                IMAGE_ID,
            ) {
                Ok(export) => export,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE inspection\n");
            let allocation_before_play = BOOT_ARENA.seal();
            arch::initialize_machine();
            let mut clock = arch::Clock::new();
            let mut timer = arch::Timer::new();
            let mut serial = arch::Serial::new();
            let mut interrupts = arch::Interrupts::new();
            let mut idle = arch::Idle::new();
            arch::early_write(b"CONDUIT_BOOT_STAGE play\n");
            match conduitos::dual_region_composition::run(
                &mut prepared.kernel,
                &mut clock,
                &mut timer,
                &mut serial,
                &mut interrupts,
                &mut idle,
            ) {
                Ok(report) => match proof::machine_accepted(
                    &identities,
                    &offer,
                    &report,
                    &prepared,
                    proof::AllocationProof {
                        before_play: allocation_before_play,
                        after_play: BOOT_ARENA.used(),
                        capacity: BOOT_ARENA.capacity(),
                    },
                    BUILD_ID,
                ) {
                    Ok(sign) => {
                        arch::early_write(sign.as_bytes());
                        arch::early_write(conduitos::observatory::EXPORT_PREFIX.as_bytes());
                        arch::early_write(observatory_export.as_bytes());
                        arch::early_write(b"\n");
                        arch::deterministic_exit(true);
                    }
                    Err(_) => emit_machine_refusal("kernel-sign-storage-full"),
                },
                Err(error) => emit_machine_refusal(error.as_str()),
            }
        }
        Err(error) => emit_refusal(error.as_str()),
    }
}

#[cfg(target_os = "none")]
fn emit_machine_refusal(reason: &str) -> ! {
    if let Ok(sign) = proof::machine_refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[cfg(not(target_os = "none"))]
fn main() {}

#[cfg(target_os = "none")]
fn emit_refusal(reason: &str) -> ! {
    if let Ok(sign) = proof::refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[panic_handler]
#[cfg(target_os = "none")]
fn panic(_info: &PanicInfo<'_>) -> ! {
    emit_refusal("panic")
}
