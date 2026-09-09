//! Bounded scripted keyboard, audio and kernel proof continuation.
use crate::emit_machine_refusal;
use alloc::format;
use conduitos::{allocation::BOOT_ARENA, arch, boot, identity, sign_format};
use conduitos::{dual_region_plan, pc_speaker_plan, pc_speaker_play};
use core::fmt::Write;

pub struct Context {
    pub record: boot::BootRecord,
    pub identities: identity::BootIdentities,
    pub offer: conduitos::offer_fabrication::ImageBoundHostOffer<'static>,
    pub framebuffer_basis: conduit_observatory::FramebufferBasis,
    pub xhci: arch::XhciReady,
    pub usb: arch::UsbDevice,
    pub hid_session: arch::HidKeyboardSession,
    pub rescue_matcher: conduitos::local_rescue::LocalRescueMatcher,
    pub xhci_base: [u8; 32],
    pub device_id: [u8; 32],
    pub interface_id: [u8; 32],
    pub endpoint_id: [u8; 32],
    pub keyboard_limits: [u16; 3],
}

pub fn run(context: Context) -> ! {
    let Context {
        record,
        identities,
        offer,
        framebuffer_basis,
        mut xhci,
        usb,
        mut hid_session,
        mut rescue_matcher,
        xhci_base,
        device_id,
        interface_id,
        endpoint_id,
        keyboard_limits,
    } = context;
    let fabrication = &conduitos::fabrication::EMBEDDED_FABRICATION;
    let xhci_base_id = identity::hex(&xhci_base);
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
    let keyboard_prepared =
        match conduitos::keyboard_plan::prepare(&identities, &offer, fabrication.build_id) {
            Ok(prepared) => prepared,
            Err(error) => emit_machine_refusal(error.as_str()),
        };
    arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-offer-ready\n");
    arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-plan-ready\n");
    arch::early_write(b"CONDUIT_BOOT_STAGE keyboard-play-started\n");
    let (proof_followup, proof_followup_count) = match hid_session.receive_followup(&mut xhci, &usb)
    {
        Ok(batch) => batch,
        Err(error) => emit_machine_refusal(error.as_str()),
    };
    let hid = match hid_session.scripted_initial_proof(&proof_followup[..proof_followup_count]) {
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
    let keyboard_report = match conduitos::keyboard_play::run(&keyboard_prepared, portable_values) {
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
        keyboard_limits[0],
        keyboard_limits[1],
        keyboard_limits[2],
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
    let keyboard_text_events: [conduit_human::KeyEvent;
        conduitos::keyboard_text_guest::PHYSICAL_TRANSITIONS] = core::array::from_fn(|index| {
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
    let mut prepared = match dual_region_plan::prepare(&identities, &offer, fabrication.build_id) {
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
    let pc_speaker_report = match pc_speaker_play::run(&mut pc_speaker_execution, &mut pc_speaker) {
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
