#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("#588 currently promotes only the executable x86_64 ConduitOS backend");

#[cfg(target_os = "none")]
use core::{fmt::Write, panic::PanicInfo};

#[cfg(target_os = "none")]
use alloc::format;

#[cfg(target_os = "none")]
use conduitos::{
    allocation::BOOT_ARENA, arch, boot, display::PixelTarget, dual_region_plan, identity,
    pc_speaker_plan, pc_speaker_play, presentation_nucleus, proof,
};

#[cfg(target_os = "none")]
#[unsafe(no_mangle)]
extern "C" fn conduitos_start() -> ! {
    match boot::normalize_boot() {
        Ok(record) => {
            let fabrication = &conduitos::fabrication::EMBEDDED_FABRICATION;
            if let Err(error) = fabrication.validate(record.runtime_arena.length) {
                emit_machine_refusal(error.as_str());
            }
            if !fabrication.includes(conduitos::fabrication::IMPL_NATIVE_PRESENTER)
                || !fabrication
                    .includes_facility(conduitos::fabrication::FACILITY_NATIVE_COMPOSITOR)
            {
                emit_machine_refusal("fabrication-presentation-unavailable");
            }
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
            let mut presentation_display = match boot::framebuffer_display() {
                Ok(display) => display,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let display_format = presentation_display.format();
            let display_base =
                identity::derive_base(&identities.boot, "conduitos/display/limine/0");
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
                let presentation = match presentation_nucleus::run(
                    &prepared_presentation,
                    &mut presentation_display,
                ) {
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
            let hid_ready = match arch::prepare_boot_keyboard(
                &mut xhci,
                &usb,
                boot::executable_physical_address,
            ) {
                Ok(ready) => ready,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE local-rescue-ready\n");
            let offer = match conduitos::offer_fabrication::ImageBoundHostOffer::new(
                &identities,
                fabrication,
                arch::feature_basis(),
                record.runtime_arena.length,
            )
            .and_then(|offer| {
                offer.with_keyboard(
                    fabrication,
                    conduitos::keyboard_offer::KeyboardRealization {
                        controller_id: xhci_base,
                        device_id,
                        interface_id,
                        endpoint_id,
                        report_buffers: hid_ready.report_buffers,
                        transition_slots: hid_ready.transition_slots,
                        operation_slots: hid_ready.operation_slots,
                    },
                )
            })
            .and_then(|offer| {
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
            let mut rescue_matcher = conduitos::local_rescue::LocalRescueMatcher::new();
            let mut modifier_prefix = !hid_session.transitions().is_empty()
                && hid_session
                    .transitions()
                    .iter()
                    .all(|transition| (0xe0..=0xe7).contains(&transition.usage()));
            for transition in hid_session.transitions().iter().copied() {
                conduitos::rescue_guest::observe(
                    &identities,
                    &mut rescue_matcher,
                    transition.into_local_rescue(),
                    false,
                );
            }
            while modifier_prefix {
                let (transitions, count) = match hid_session.receive_followup(&mut xhci, &usb) {
                    Ok(batch) => batch,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
                for transition in transitions[..count].iter().copied() {
                    conduitos::rescue_guest::observe(
                        &identities,
                        &mut rescue_matcher,
                        transition.into_local_rescue(),
                        false,
                    );
                }
                modifier_prefix = transitions[..count]
                    .iter()
                    .all(|transition| (0xe0..=0xe7).contains(&transition.usage()));
            }
            if !cfg!(feature = "scripted-keyboard-proof") {
                if let Err(reason) = conduitos::product_front_door::run(
                    &identities,
                    &offer,
                    fabrication,
                    &framebuffer_basis,
                    &mut presentation_display,
                    &mut hid_session,
                    &mut xhci,
                    &usb,
                    &mut rescue_matcher,
                ) {
                    emit_machine_refusal(reason);
                }
                unreachable!("interactive HID loop only returns on refusal");
            }
            let opl2_offer = conduitos::opl2_offer::Opl2Offer {
                artifact_build: fabrication.build_id,
                realization: conduitos::opl2_offer::Opl2Realization {
                    base_id: identity::derive_base(&identities.boot, "conduitos/opl2/0"),
                    clock_hz: conduitos::opl2_offer::OPL2_CLOCK_HZ,
                    channels: conduitos::opl2_offer::OPL2_CHANNELS,
                    maximum_error_parts_per_million: 2_500,
                    event_slots: 32,
                    register_write_slots: 512,
                    patch_profile: conduitos::opl2_offer::OPL2_PATCH_PROFILE,
                },
            };
            let opl2_prepared = match conduitos::opl2_plan::prepare(
                &identities,
                &offer,
                opl2_offer,
                fabrication.build_id,
            ) {
                Ok(prepared) => prepared,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let mut opl2_execution = match conduitos::opl2_play::prepare_execution(
                &opl2_prepared,
                conduitos::opl2_play::reviewed_values(),
            ) {
                Ok(execution) => execution,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let opl2_host_id = identity::hex(&identities.host);
            let opl2_boot_id = identity::hex(&identities.boot);
            let opl2_base_id = identity::hex(&opl2_offer.realization.base_id);
            let keyboard_prepared = match conduitos::keyboard_plan::prepare(
                &identities,
                &offer,
                fabrication.build_id,
            ) {
                Ok(prepared) => prepared,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-offer-ready\n");
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-plan-ready\n");
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-play-started\n");
            let (proof_followup, proof_followup_count) =
                match hid_session.receive_followup(&mut xhci, &usb) {
                    Ok(batch) => batch,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
            let hid =
                match hid_session.scripted_initial_proof(&proof_followup[..proof_followup_count]) {
                    Ok(proof) => proof,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
            let portable_values = [
                match conduitos::keyboard_bridge::portable_key_event(
                    hid.transitions[0].usage(),
                    hid.transitions[0].pressed(),
                    hid.transitions[0].modifiers(),
                ) {
                    Ok(value) => value,
                    Err(_) => emit_machine_refusal("keyboard-portable-value-invalid"),
                },
                match conduitos::keyboard_bridge::portable_key_event(
                    hid.transitions[1].usage(),
                    hid.transitions[1].pressed(),
                    hid.transitions[1].modifiers(),
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
                hid.transitions[0].usage(),
                if hid.transitions[0].pressed() {
                    "pressed"
                } else {
                    "released"
                },
                hid.transitions[0].modifiers(),
                hid.transitions[1].usage(),
                if hid.transitions[1].pressed() {
                    "pressed"
                } else {
                    "released"
                },
                hid.transitions[1].modifiers(),
            );
            arch::early_write(hid_sign.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE hid-transitions\n");
            let keyboard_sign = format!(
                "CONDUIT_KEYBOARD_SIGN {{\"schema\":\"conduit.conduitos.keyboard-offer/v1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"offer_generation\":{},\"kind\":\"input/keyboard\",\"contract_revision\":\"{}\",\"implementation\":\"{}\",\"execution_profile\":\"{}\",\"artifact_build\":\"{}\",\"controller_base_id\":\"{}\",\"device_instance_id\":\"{}\",\"interface_id\":\"{}\",\"endpoint_id\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"resource_bindings\":{},\"report_buffers\":{},\"transition_slots\":{},\"operation_slots\":{},\"cord_item_capacity\":{},\"cord_byte_capacity\":{},\"event_count\":2,\"first_value\":[{},{},{}],\"second_value\":[{},{},{}],\"semantic_usb_facts\":false,\"layout_translation\":false,\"unicode_translation\":false,\"completed\":{}}}\n",
                identity::hex(&identities.host),
                identity::hex(&identities.boot),
                offer.generation,
                conduit_semantic_catalog::KEYBOARD_CONTRACT_REVISION,
                conduitos::keyboard_offer::KEYBOARD_IMPLEMENTATION,
                conduitos::keyboard_offer::KEYBOARD_EXECUTION_PROFILE,
                fabrication.build_id,
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
            arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-text-play-started\n");
            let mut proof_transitions = [arch::HidKeyTransition::default();
                2 + conduitos::keyboard_text_guest::PHYSICAL_TRANSITIONS];
            proof_transitions[..2].copy_from_slice(&hid.transitions);
            let mut proof_transition_count = 2usize;
            if let Err(error) = hid_session.receive_until_observing(
                &mut xhci,
                &usb,
                2 + conduitos::keyboard_text_guest::PHYSICAL_TRANSITIONS,
                |transition| {
                    proof_transitions[proof_transition_count] = transition;
                    proof_transition_count += 1;
                    conduitos::rescue_guest::observe(
                        &identities,
                        &mut rescue_matcher,
                        transition.into_local_rescue(),
                        true,
                    )
                },
            ) {
                emit_machine_refusal(error.as_str());
            }
            let keyboard_text_events: [conduit_core::KeyEvent;
                conduitos::keyboard_text_guest::PHYSICAL_TRANSITIONS] =
                core::array::from_fn(|index| {
                    let transition = proof_transitions[index + 2];
                    match conduitos::keyboard_bridge::portable_key_event(
                        transition.usage(),
                        transition.pressed(),
                        transition.modifiers(),
                    ) {
                        Ok(event) => event,
                        Err(_) => emit_machine_refusal("keyboard-text-portable-value-invalid"),
                    }
                });
            if let Err(reason) = conduitos::keyboard_text_guest::run_reviewed_sequences(
                &record,
                &identities,
                &offer,
                fabrication.build_id,
                fabrication.image_binding,
                &keyboard_text_events,
                Some(&framebuffer_basis),
            ) {
                let _ = reason;
                emit_machine_refusal("keyboard-proof-sequence-mismatch");
            }
            if cfg!(feature = "hotplug-proof") {
                conduitos::hotplug_guest::run(conduitos::hotplug_guest::HotplugProofInputs {
                    record: &record,
                    identities: &identities,
                    controller: &mut xhci,
                    d1: usb,
                    d1_session: hid_session,
                    d1_offer: offer.into_inner(),
                    controller_id: xhci_base,
                    build_id: fabrication.build_id,
                });
            }
            if let Err(error) = offer.validate() {
                emit_machine_refusal(error.as_str());
            }
            arch::early_write(b"CONDUIT_BOOT_STAGE offer\n");
            let pc_speaker_prepared =
                match pc_speaker_plan::prepare(&identities, &offer, fabrication.build_id) {
                    Ok(prepared) => prepared,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
            let mut pc_speaker_execution = match pc_speaker_play::prepare_execution(
                &pc_speaker_prepared,
                pc_speaker_play::reviewed_values(),
            ) {
                Ok(execution) => execution,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE pc-speaker-plan\n");
            let mut prepared =
                match dual_region_plan::prepare(&identities, &offer, fabrication.build_id) {
                    Ok(prepared) => prepared,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
            arch::early_write(b"CONDUIT_BOOT_STAGE plan\n");
            let observatory_export = match conduitos::observatory::prepare_image_bound_export(
                &record,
                &identities,
                &offer,
                &prepared,
                conduitos::observatory::ImageBoundProvenance {
                    profile_id: fabrication.profile_id,
                    build_id: fabrication.build_id,
                    image_binding: fabrication.image_binding,
                },
                Some(&framebuffer_basis),
            ) {
                Ok(export) => export,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            let pc_speaker_base_id = identity::hex(&identity::derive_base(
                &identities.boot,
                "conduitos/pc-speaker/0",
            ));
            let pc_speaker_host_id = identity::hex(&identities.host);
            let pc_speaker_boot_id = identity::hex(&identities.boot);
            arch::early_write(b"CONDUIT_BOOT_STAGE inspection\n");
            let allocation_before_play = BOOT_ARENA.seal();
            arch::initialize_machine();
            let mut opl2 = arch::Opl2::new();
            arch::early_write(b"CONDUIT_BOOT_STAGE opl2-play-started\n");
            let opl2_report = match conduitos::opl2_play::run_with_evidence(
                &opl2_prepared,
                &mut opl2_execution,
                &mut opl2,
            ) {
                Ok(report) => report,
                Err(error) => emit_machine_refusal(error.as_str()),
            };
            arch::early_write(b"CONDUIT_BOOT_STAGE opl2-play-finished\n");
            let mut opl2_sign = sign_format::FixedText::new();
            if writeln!(
                opl2_sign,
                "CONDUIT_OPL2_SIGN {{\"schema\":\"conduit.conduitos.opl2-proof/v1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"base_id\":\"{}\",\"implementation\":\"{}\",\"execution_profile\":\"{}\",\"patch_profile\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"placements\":{},\"cords\":{},\"events\":{},\"peak_voices\":{},\"voice_capacity\":9,\"reset_writes\":{},\"patch_writes\":{},\"event_writes\":{},\"quiesce_writes\":{},\"register_write_capacity\":512,\"kernel_decisions\":{},\"kernel_signs\":{},\"final_active_voices\":{},\"normalized_events\":{},\"normalized_terminal\":\"completed\",\"normalized_plan_id\":\"{}\",\"normalized_implementation\":\"{}\",\"device\":\"qemu-adlib-ym3812\",\"iobase\":904,\"pcm_claimed\":false,\"subtractive_controls_claimed\":false,\"physical_hardware_claimed\":false,\"bounded\":true,\"completed\":{}}}",
                opl2_host_id,
                opl2_boot_id,
                opl2_base_id,
                conduitos::opl2_offer::OPL2_IMPLEMENTATION,
                conduitos::opl2_offer::OPL2_EXECUTION_PROFILE,
                conduitos::opl2_offer::OPL2_PATCH_PROFILE,
                opl2_prepared.plan.plan_id.as_str(),
                opl2_prepared.active_play.active_play_id.as_str(),
                opl2_prepared.plan.fragments[0].placements.len(),
                opl2_prepared.plan.fragments[0].connections.len(),
                opl2_report.play.events,
                opl2_report.play.peak_voices,
                opl2_report.play.reset_writes,
                opl2_report.play.patch_writes,
                opl2_report.play.event_writes,
                opl2_report.play.quiesce_writes,
                opl2_report.play.kernel_decisions,
                opl2_report.play.kernel_signs,
                opl2_report.play.final_active_voices,
                opl2_report.evidence.trace.events.len(),
                opl2_report.evidence.selected.plan_id.as_str(),
                opl2_report.evidence.selected.implementation_id.as_str(),
                opl2_report.play.completed,
            )
            .is_err()
            {
                emit_machine_refusal("opl2-sign-storage-full");
            }
            arch::early_write(opl2_sign.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE opl2-completed\n");
            let mut clock = arch::Clock::new();
            let mut timer = arch::Timer::new();
            let mut serial = arch::Serial::new();
            let mut interrupts = arch::Interrupts::new();
            let mut idle = arch::Idle::new();
            let mut pc_speaker = arch::PcSpeaker::new();
            let pc_speaker_report =
                match pc_speaker_play::run(&mut pc_speaker_execution, &mut pc_speaker) {
                    Ok(report) => report,
                    Err(error) => emit_machine_refusal(error.as_str()),
                };
            let mut pc_speaker_sign = sign_format::FixedText::new();
            if writeln!(
                pc_speaker_sign,
                "CONDUIT_PC_SPEAKER_SIGN {{\"schema\":\"conduit.conduitos.pc-speaker-tone/v1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"base_id\":\"{}\",\"kind\":\"{}\",\"implementation\":\"{}\",\"execution_profile\":\"{}\",\"plan_id\":\"{}\",\"active_play_id\":\"{}\",\"node_count\":{},\"cord_count\":{},\"requested_millihertz\":[{},{},{},{}],\"realized_millihertz\":[{},{},{},{}],\"divisors\":[{},{},{},{}],\"gate_transitions\":[{},{},{},{}],\"transition_count\":{},\"kernel_decisions\":{},\"kernel_signs\":{},\"final_gate_open\":{},\"bounded\":true,\"completed\":{}}}\n",
                pc_speaker_host_id,
                pc_speaker_boot_id,
                pc_speaker_base_id,
                conduit_semantic_catalog::SOUND_TONE_PLAY_KIND,
                conduitos::pc_speaker_offer::PC_SPEAKER_IMPLEMENTATION,
                conduitos::pc_speaker_offer::PC_SPEAKER_EXECUTION_PROFILE,
                pc_speaker_prepared.plan.plan_id.as_str(),
                pc_speaker_prepared.active_play.active_play_id.as_str(),
                pc_speaker_prepared.plan.fragments[0].placements.len(),
                pc_speaker_prepared.plan.fragments[0].connections.len(),
                pc_speaker_report.realized[0].requested_millihertz,
                pc_speaker_report.realized[1].requested_millihertz,
                pc_speaker_report.realized[2].requested_millihertz,
                pc_speaker_report.realized[3].requested_millihertz,
                pc_speaker_report.realized[0].realized_millihertz,
                pc_speaker_report.realized[1].realized_millihertz,
                pc_speaker_report.realized[2].realized_millihertz,
                pc_speaker_report.realized[3].realized_millihertz,
                pc_speaker_report.realized[0].divisor,
                pc_speaker_report.realized[1].divisor,
                pc_speaker_report.realized[2].divisor,
                pc_speaker_report.realized[3].divisor,
                pc_speaker_report.realized[0].gate_open,
                pc_speaker_report.realized[1].gate_open,
                pc_speaker_report.realized[2].gate_open,
                pc_speaker_report.realized[3].gate_open,
                pc_speaker_report.transitions,
                pc_speaker_report.kernel_decisions,
                pc_speaker_report.kernel_signs,
                pc_speaker_report.final_gate_open,
                pc_speaker_report.completed,
            )
            .is_err()
            {
                emit_machine_refusal("pc-speaker-sign-storage-full");
            }
            arch::early_write(pc_speaker_sign.as_bytes());
            arch::early_write(b"CONDUIT_BOOT_STAGE pc-speaker-completed\n");
            arch::early_write(b"CONDUIT_BOOT_STAGE play\n");
            match conduitos::dual_region_composition::run(
                &mut prepared.kernel,
                &mut clock,
                &mut timer,
                &mut serial,
                &mut interrupts,
                &mut idle,
            ) {
                Ok(report) => match sign_format::machine_accepted(
                    &identities,
                    &offer,
                    &report,
                    &prepared,
                    sign_format::AllocationReceipt {
                        before_play: allocation_before_play,
                        after_play: BOOT_ARENA.used(),
                        capacity: BOOT_ARENA.capacity(),
                    },
                    fabrication.build_id,
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
    if let Ok(sign) = sign_format::machine_refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[cfg(not(target_os = "none"))]
fn main() {}

#[cfg(target_os = "none")]
fn emit_refusal(reason: &str) -> ! {
    if let Ok(sign) = sign_format::refused(reason) {
        arch::early_write(sign.as_bytes());
    }
    arch::deterministic_exit(false)
}

#[panic_handler]
#[cfg(target_os = "none")]
fn panic(_info: &PanicInfo<'_>) -> ! {
    emit_refusal("panic")
}
