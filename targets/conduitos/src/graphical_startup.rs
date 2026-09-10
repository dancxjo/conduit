//! Graphical x86 startup and initialized input ownership.
use crate::{emit_machine_refusal, emit_refusal};
use alloc::format;
use conduitos::{allocation::BOOT_ARENA, arch, boot, identity, sign_format};
use conduitos::{display::PixelTarget, presentation_nucleus};

pub fn run(record: boot::BootRecord) -> ! {
    let fabrication = &conduitos::fabrication::EMBEDDED_FABRICATION;
    if !fabrication.includes(conduitos::fabrication::IMPL_NATIVE_PRESENTER)
        || !fabrication.includes_facility(conduitos::fabrication::FACILITY_NATIVE_COMPOSITOR)
    {
        emit_machine_refusal("fabrication-presentation-unavailable");
    }
    arch::early_write(b"CONDUIT_BOOT_STAGE xhci-start\n");
    let mut xhci =
        match arch::initialize_xhci(record.hhdm_offset, boot::executable_physical_address) {
            Ok(ready) => ready,
            Err(error) => emit_machine_refusal(error.as_str()),
        };
    arch::early_write(b"CONDUIT_BOOT_STAGE xhci-ready\n");
    arch::early_write(b"CONDUIT_BOOT_STAGE usb-enumeration-start\n");
    let mut usb_devices = match arch::enumerate_attached_at_epochs(
        &mut xhci,
        boot::executable_physical_address,
        [1; 3],
    ) {
        Ok(devices) => devices,
        Err(error) => emit_machine_refusal(error.as_str()),
    };
    let usb = match usb_devices[0].take() {
        Some(device) => device,
        None => emit_machine_refusal("usb-primary-device-absent"),
    };
    let pointer_usb = usb_devices[1].take();
    let line_usb = usb_devices[2].take();
    if line_usb.is_some() {
        arch::early_write(b"CONDUIT_BOOT_STAGE usb-line-device-current\n");
    }
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
    let identities = identity::derive(entropy, record.timestamp, record.image_physical_start);
    let mut presentation_display = match boot::framebuffer_display() {
        Ok(display) => display,
        Err(error) => emit_machine_refusal(error.as_str()),
    };
    let display_format = presentation_display.format();
    let display_base = identity::derive_base(&identities.boot, "conduitos/display/limine/0");
    let framebuffer_basis = conduit_observatory::FramebufferBasis {
        base_id: conduit_core::HostBaseId::from(identity::hex(&display_base)),
        width: display_format.width,
        height: display_format.height,
        pitch_bytes: display_format.pitch,
        bits_per_pixel: display_format.bits_per_pixel,
    };
    if cfg!(feature = "scripted-keyboard-proof") {
        let prepared_presentation = match presentation_nucleus::prepare(
            &identity::hex(&identities.host),
            &identity::hex(&identities.boot),
        ) {
            Ok(prepared) => prepared,
            Err(error) => emit_machine_refusal(error.as_str()),
        };
        let presentation =
            match presentation_nucleus::run(&prepared_presentation, &mut presentation_display) {
                Ok(proof) => proof,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
        let presentation_sign = format!(
            "CONDUIT_PRESENTATION_SIGN {{\"schema\":\"conduit.conduitos.framebuffer-presentation/v1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"realization\":\"recursive\",\"back_kind\":\"{}\",\"back_contract_revision\":\"{}\",\"back_invocation_path\":\"{}\",\"back_source_document_id\":\"{}\",\"back_checked_form_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"display_base_id\":\"{}\",\"display_width\":{},\"display_height\":{},\"display_pitch\":{},\"display_bits_per_pixel\":{},\"execution_profile\":\"{}\",\"artifact\":\"{}\",\"source_document_id\":\"{}\",\"checked_form_id\":\"{}\",\"expanded_form_id\":\"{}\",\"plan_id\":\"{}\",\"fragment_id\":\"{}\",\"node_count\":{},\"cord_count\":{},\"text\":\"{}\",\"layout_children\":{},\"graphics_commands\":{},\"text_commands\":{},\"text_pixels_written\":{},\"graphics_pixels_written\":{},\"kernel_signs\":{},\"bounded\":true,\"completed\":true}}\n",
            presentation.realization_back.kind_id.as_str(),
            presentation
                .realization_back
                .kind_contract_revision
                .as_str(),
            presentation.realization_back.invocation_path,
            presentation.realization_back.source_document_id.as_str(),
            presentation.realization_back.checked_form_id.as_str(),
            identity::hex(&identities.host),
            identity::hex(&identities.boot),
            identity::hex(&display_base),
            display_format.width,
            display_format.height,
            display_format.pitch,
            display_format.bits_per_pixel,
            conduitos::presentation_nucleus::CONDUITOS_PRESENTATION_PROFILE,
            conduitos::presentation_nucleus::CONDUITOS_PRESENTATION_ARTIFACT,
            prepared_presentation.plan.source_document_id.as_str(),
            prepared_presentation.plan.checked_form_id.as_str(),
            prepared_presentation.plan.expanded_form_id.as_str(),
            presentation.plan_id.as_str(),
            presentation.fragment_id.as_str(),
            presentation.node_count,
            presentation.cord_count,
            presentation.text,
            presentation.layout_children,
            presentation.graphics_commands,
            presentation.text_display.commands,
            presentation.text_display.pixels_written,
            presentation.display.pixels_written,
            presentation.kernel_signs,
        );
        arch::early_write(presentation_sign.as_bytes());
    }
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
    let hid_ready =
        match arch::prepare_boot_keyboard(&mut xhci, &usb, boot::executable_physical_address) {
            Ok(ready) => ready,
            Err(error) => emit_machine_refusal(error.as_str()),
        };
    let pointer_ready = pointer_usb.as_ref().map(|device| {
        arch::prepare_boot_pointer(&mut xhci, device, boot::executable_physical_address)
            .unwrap_or_else(|error| emit_machine_refusal(error.as_str()))
    });
    let mut ps2_input = None;
    let mut ps2_ready = None;
    if cfg!(feature = "ps2-input") {
        let (input, ready) = arch::Ps2Input::initialize()
            .unwrap_or_else(|error| emit_machine_refusal(error.as_str()));
        arch::early_write(
        format!(
            "CONDUIT_PS2_INPUT_SIGN {{\"schema\":\"conduit.conduitos.ps2-input/v1\",\"status\":\"ready\",\"proof_class\":\"freestanding-emulator\",\"keyboard_kind\":\"input/keyboard\",\"pointer_kind\":\"input/pointer-source\",\"controller_slots\":{},\"keyboard_transition_slots\":{},\"pointer_packet_slots\":{},\"operation_slots\":{},\"bounded\":true}}\n",
            ready.controller_slots,
            ready.keyboard_transition_slots,
            ready.pointer_packet_slots,
            ready.operation_slots,
        )
        .as_bytes(),
    );
        arch::early_write(b"CONDUIT_BOOT_STAGE ps2-input-ready\n");
        ps2_input = Some(input);
        ps2_ready = Some(ready);
    }
    arch::early_write(b"CONDUIT_BOOT_STAGE local-rescue-ready\n");
    let keyboard_offer = conduitos::offer_fabrication::ImageBoundHostOffer::new(
        &identities,
        fabrication,
        arch::feature_basis(),
        record.runtime_arena.length,
    )
    .and_then(|offer| {
        offer.with_keyboard(
            fabrication,
            if let Some(ready) = ps2_ready {
                conduitos::keyboard_offer::KeyboardRealization {
                    mechanism: conduitos::keyboard_offer::KeyboardMechanism::Ps2,
                    controller_id: identity::derive_base(&identities.boot, "conduitos/i8042/0"),
                    device_id: identity::derive_base(&identities.boot, "conduitos/i8042/keyboard"),
                    interface_id: identity::derive_base(
                        &identities.boot,
                        "conduitos/i8042/keyboard/port",
                    ),
                    endpoint_id: identity::derive_base(
                        &identities.boot,
                        "conduitos/i8042/keyboard/scancode-stream",
                    ),
                    report_buffers: ready.controller_slots,
                    transition_slots: ready.keyboard_transition_slots,
                    operation_slots: ready.operation_slots,
                }
            } else {
                conduitos::keyboard_offer::KeyboardRealization {
                    mechanism: conduitos::keyboard_offer::KeyboardMechanism::UsbHid,
                    controller_id: xhci_base,
                    device_id,
                    interface_id,
                    endpoint_id,
                    report_buffers: hid_ready.report_buffers,
                    transition_slots: hid_ready.transition_slots,
                    operation_slots: hid_ready.operation_slots,
                }
            },
        )
    });
    let input_offer = keyboard_offer.and_then(|offer| {
        if let Some(ready) = ps2_ready {
            return offer.with_pointer(
                fabrication,
                conduitos::pointer_offer::PointerRealization {
                    mechanism: conduitos::pointer_offer::PointerMechanism::Ps2,
                    controller_id: identity::derive_base(&identities.boot, "conduitos/i8042/0"),
                    device_id: identity::derive_base(&identities.boot, "conduitos/i8042/pointer"),
                    interface_id: identity::derive_base(
                        &identities.boot,
                        "conduitos/i8042/pointer/port",
                    ),
                    endpoint_id: identity::derive_base(
                        &identities.boot,
                        "conduitos/i8042/pointer/packet-stream",
                    ),
                    report_buffers: ready.pointer_packet_slots,
                    event_slots: conduitos::pointer_offer::POINTER_EVENT_SLOTS,
                    operation_slots: ready.operation_slots,
                },
            );
        }
        let Some((pointer, ready)) = pointer_usb.as_ref().zip(pointer_ready) else {
            return Ok(offer);
        };
        let realization =
            conduitos::product_pointer::realization(&identities, xhci_base, pointer, ready)
                .unwrap_or_else(|error| emit_machine_refusal(error));
        offer.with_pointer(fabrication, realization)
    });
    let offer = match input_offer.and_then(|offer| {
        offer.with_pc_speaker(
            fabrication,
            conduitos::pc_speaker_offer::PcSpeakerRealization {
                base_id: identity::derive_base(&identities.boot, "conduitos/pc-speaker/0"),
                pit_input_hz: arch::pc_speaker_input_hz(),
                minimum_divisor: 19,
                maximum_divisor: u16::MAX,
                maximum_error_parts_per_million: 2_500,
                event_slots: 8,
                operation_slots: 1,
            },
        )
    }) {
        Ok(offer) => offer,
        Err(error) => emit_machine_refusal(error.as_str()),
    };
    match sign_format::accepted(&record, &identities, fabrication, offer.generation) {
        Ok(sign) => {
            arch::early_write(sign.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE identities\n");
        }
        Err(_) => emit_refusal("boot-sign-storage-full"),
    }
    let mut hid_session = if cfg!(feature = "scripted-keyboard-proof") {
        match arch::receive_first_boot_keyboard_report(&mut xhci, &usb, hid_ready) {
            Ok(session) => session,
            Err(error) => emit_machine_refusal(error.as_str()),
        }
    } else {
        arch::start_boot_keyboard_session(hid_ready)
    };
    let mut pointer_session = pointer_ready.map(arch::start_pointer_session);
    let mut rescue_matcher = conduitos::local_rescue::LocalRescueMatcher::new();
    for transition in hid_session.transitions().iter().copied() {
        conduitos::rescue_guest::observe(
            &identities,
            &mut rescue_matcher,
            transition.into_local_rescue(),
            false,
        );
    }
    if !cfg!(feature = "scripted-keyboard-proof") {
        if let Err(reason) = conduitos::product_front_door::run(
            &identities,
            &offer,
            fabrication,
            &framebuffer_basis,
            &mut presentation_display,
            Some(&mut hid_session),
            &mut xhci,
            xhci_base,
            &usb,
            pointer_session.as_mut(),
            pointer_usb.as_ref(),
            line_usb.as_ref(),
            ps2_input.as_mut(),
            &mut rescue_matcher,
        ) {
            emit_machine_refusal(reason);
        }
        unreachable!("interactive HID loop only returns on refusal");
    }
    crate::scripted_startup::run(crate::scripted_startup::Context {
        record,
        identities,
        offer,
        framebuffer_basis,
        xhci,
        usb,
        hid_session,
        rescue_matcher,
        xhci_base,
        device_id,
        interface_id,
        endpoint_id,
        keyboard_limits: [
            hid_ready.report_buffers,
            hid_ready.transition_slots,
            hid_ready.operation_slots,
        ],
    })
}
