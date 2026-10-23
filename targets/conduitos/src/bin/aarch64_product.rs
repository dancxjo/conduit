#![no_std]
#![no_main]

#[cfg(not(target_arch = "aarch64"))]
compile_error!("conduitos-aarch64-product is only the AArch64 product Host");

use core::panic::PanicInfo;

use conduit_core::{BootId, HostId, OfferGeneration};
use conduitos::{
    allocation::BOOT_ARENA,
    arch, boot, dual_region_composition, dual_region_plan,
    front_door::FrontDoor,
    identity, keyboard_text_plan,
    linear_presenter::LinearPresenter,
    make::{EMBEDDED_MAKE, IMPL_LINEAR_PRESENTER},
    observatory,
    offer::CpuFeatures,
    offer_make::ImageBoundHostOffer,
    spore_join,
};

#[unsafe(no_mangle)]
pub extern "C" fn conduitos_aarch64_product_start() -> ! {
    arch::enable_fp_simd();
    let (l0, l1, l2) = arch::mmio_table_addresses();
    let l0 = boot::executable_physical_address(l0).unwrap_or_else(|| refuse("mmio-l0-invalid"));
    let l1 = boot::executable_physical_address(l1).unwrap_or_else(|| refuse("mmio-l1-invalid"));
    let l2 = boot::executable_physical_address(l2).unwrap_or_else(|| refuse("mmio-l2-invalid"));
    arch::install_low_mmio_map(l0, l1, l2);
    arch::initialize_machine();
    let record = boot::normalize_boot_with_arena_bytes(EMBEDDED_MAKE.runtime_arena_ceiling)
        .unwrap_or_else(|error| refuse(error.as_str()));
    arch::initialize_domains(&record);
    let arena = record
        .hhdm_offset
        .checked_add(record.runtime_arena.physical_start)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| refuse("runtime-arena-address-invalid"));
    unsafe {
        BOOT_ARENA
            .initialize(
                arena,
                usize::try_from(record.runtime_arena.length).unwrap_or(0),
            )
            .unwrap_or_else(|_| refuse("runtime-arena-initialization-failed"));
    }
    EMBEDDED_MAKE
        .validate(record.runtime_arena.length)
        .unwrap_or_else(|error| refuse(error.as_str()));
    if !EMBEDDED_MAKE.includes(IMPL_LINEAR_PRESENTER) {
        refuse("linear-presenter-absent-from-image");
    }

    // SAFETY: this ordinary product startup is the sole Root, before other
    // native owners; selected local modules require independent administrator
    // firmware and electrical approval and permanently retain their resources.
    unsafe { conduitos::protocol_boot::run_if_selected(&record) }
        .unwrap_or_else(|reason| refuse(reason));

    let counter = arch::read_counter();
    let identities = identity::derive(
        [
            counter,
            counter.rotate_left(13),
            counter.rotate_left(29),
            counter.rotate_left(47),
        ],
        record.timestamp,
        record.image_physical_start,
    );
    let offer = ImageBoundHostOffer::new(
        &identities,
        &EMBEDDED_MAKE,
        CpuFeatures {
            sse2: false,
            rdrand: false,
            invariant_tsc: false,
        },
        record.runtime_arena.length,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    offer
        .validate()
        .unwrap_or_else(|error| refuse(error.as_str()));

    let host_identity = identity::hex(&identities.host);
    let boot_identity = identity::hex(&identities.boot);
    let host_id = HostId::from(host_identity.as_str());
    let boot_id = BootId::from(boot_identity.as_str());
    let generation = OfferGeneration(offer.generation);
    let plot =
        keyboard_text_plan::checked_plot_identity().unwrap_or_else(|error| refuse(error.as_str()));
    let front_door = FrontDoor::new(
        host_id.clone(),
        boot_id.clone(),
        generation,
        EMBEDDED_MAKE.profile_id,
        EMBEDDED_MAKE.build_id,
        EMBEDDED_MAKE.image_binding,
        plot.source_document_id,
        plot.checked_plot_id,
        5,
        false,
    );
    let face = front_door
        .face()
        .unwrap_or_else(|error| refuse(error.as_str()));
    let mut presenter = LinearPresenter::prepare(
        host_id,
        boot_id,
        generation,
        EMBEDDED_MAKE.profile_id,
        EMBEDDED_MAKE.image_binding,
    )
    .unwrap_or_else(|_| refuse("linear-presenter-plan-refused"));
    let receipt = presenter
        .present(&face)
        .unwrap_or_else(|_| refuse("linear-presenter-manifestation-refused"));
    for line in &receipt.face.lines {
        arch::present(b"CONDUIT_LINEAR_FACE ");
        arch::present(line.as_bytes());
        arch::present(b"\n");
    }

    let mut prepared = dual_region_plan::prepare(&identities, &offer, EMBEDDED_MAKE.build_id)
        .unwrap_or_else(|error| refuse(error.as_str()));
    let region = boot::spore_module().unwrap_or_else(|| refuse("spore-boot-module-missing"));
    if let Some(join) = spore_join::encode_region(
        region,
        EMBEDDED_MAKE.target,
        EMBEDDED_MAKE.profile_id,
        EMBEDDED_MAKE.build_id,
        &prepared.advertisement,
    )
    .unwrap_or_else(|error| refuse(error))
    {
        arch::present(b"CONDUIT_SPORE_JOIN ");
        arch::present(&join);
        arch::present(b"\n");
    }
    #[cfg(feature = "native-compositor-core")]
    let graphical = prepare_graphical_face(&record, &identities, &offer);
    #[cfg(feature = "native-compositor-core")]
    let framebuffer_basis = graphical
        .as_ref()
        .map(|graphic| graphic.framebuffer_basis());
    #[cfg(not(feature = "native-compositor-core"))]
    let framebuffer_basis = None;
    #[cfg(not(feature = "native-compositor-core"))]
    if EMBEDDED_MAKE.includes(conduitos::make::IMPL_NATIVE_PRESENTER) {
        refuse("firmware-face-renderer-not-linked");
    }
    let export = observatory::prepare_export(
        &record,
        &identities,
        &offer,
        &prepared,
        EMBEDDED_MAKE.build_id,
        EMBEDDED_MAKE.image_binding,
        framebuffer_basis,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));
    // Diagnostic Plays prepare independently after the ordinary Play.
    let before = if cfg!(feature = "ordinary-domain-proof") {
        BOOT_ARENA.used()
    } else {
        BOOT_ARENA.seal()
    };
    let mut clock = arch::Clock::new();
    let mut timer = arch::Timer::new();
    let mut serial = arch::Serial::new();
    let mut interrupts = arch::Interrupts::new();
    let mut idle = arch::Idle::new();
    let report = dual_region_composition::run(
        &mut prepared.kernel,
        &mut clock,
        &mut timer,
        &mut serial,
        &mut interrupts,
        &mut idle,
    )
    .unwrap_or_else(|error| refuse(error.as_str()));

    if BOOT_ARENA.used() != before {
        refuse("allocation-during-play");
    }
    #[cfg(feature = "ordinary-domain-proof")]
    conduitos::aarch64_domain_proof::run(&prepared.plan, &offer);

    arch::present(b"CONDUIT_AARCH64_PRODUCT {\"schema\":\"conduit.conduitos/aarch64-product@1\",\"status\":\"ready\",\"profile_id\":\"");
    arch::present(EMBEDDED_MAKE.profile_id.as_bytes());
    arch::present(b"\",\"build_id\":\"");
    arch::present(EMBEDDED_MAKE.build_id.as_bytes());
    arch::present(b"\",\"image_id\":\"");
    arch::present(EMBEDDED_MAKE.image_binding.as_bytes());
    arch::present(b"\",\"host_id\":\"");
    arch::present(host_identity.as_bytes());
    arch::present(b"\",\"boot_id\":\"");
    arch::present(boot_identity.as_bytes());
    arch::present(b"\",\"offer_generation\":1,\"body_id\":null,\"face_id\":\"");
    arch::present(receipt.face.face_id.as_str().as_bytes());
    arch::present(b"\",\"manifestation_id\":\"");
    arch::present(receipt.manifestation_id.as_str().as_bytes());
    arch::present(b"\",\"presenter_implementation_id\":\"");
    arch::present(receipt.presenter_implementation_id.as_str().as_bytes());
    arch::present(b"\",\"presenter_plan_id\":\"");
    arch::present(receipt.plan_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_source_document_id\":\"");
    arch::present(prepared.source_document_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_checked_plot_id\":\"");
    arch::present(prepared.checked_plot_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_expanded_plot_id\":\"");
    arch::present(prepared.expanded_plot_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_plan_id\":\"");
    arch::present(prepared.plan.plan_id.as_str().as_bytes());
    arch::present(b"\",\"ordinary_play_id\":\"");
    arch::present(prepared.active_play.active_play_id.as_str().as_bytes());
    arch::present(b"\",\"semantic_result\":\"");
    let _ = report;
    arch::present(b"HELLO, CONDUITOS");
    arch::present(b"\",\"interactive_local_control\":false,\"long_lived\":true}\n");
    arch::present(observatory::EXPORT_PREFIX.as_bytes());
    arch::present(export.as_bytes());
    arch::present(b"\n");
    arch::present(b"CONDUIT_BOOT_STAGE aarch64-product-ready\n");
    loop {
        unsafe { core::arch::asm!("wfi", options(nomem, nostack)) };
    }
}

#[cfg(feature = "native-compositor-core")]
fn prepare_graphical_face(
    record: &boot::BootRecord,
    identities: &identity::BootIdentities,
    offer: &ImageBoundHostOffer<'_>,
) -> Option<conduitos::firmware_face::FirmwareFace<arch::FirmwareScanout>> {
    if !EMBEDDED_MAKE.includes(conduitos::make::IMPL_NATIVE_PRESENTER) {
        return None;
    }
    let display = boot::framebuffer_display().unwrap_or_else(|error| refuse(error.as_str()));
    let display =
        arch::FirmwareScanout::prepare(display, record).unwrap_or_else(|reason| refuse(reason));
    let mapping = display.mapping().clone();
    let cache = *display.cache_policy();
    let source = arch::DomainEntropy::detect(1).unwrap_or_else(|error| refuse(error.as_str()));
    let mut entropy =
        conduitos::cryptographic_entropy::CryptographicEntropyBase::<_, 1>::admit(source)
            .unwrap_or_else(|error| refuse(error.as_str()));
    let graphic = entropy
        .with_secret::<32, _>(|key, _| {
            conduitos::firmware_face::FirmwareFace::prepare(
                display,
                identities,
                offer,
                &EMBEDDED_MAKE,
                *key,
            )
        })
        .unwrap_or_else(|error| refuse(error.as_str()))
        .unwrap_or_else(|reason| refuse(reason));
    let proof = serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.conduitos/aarch64-framebuffer-mapping@1",
        "host_id":identity::hex(&identities.host),"boot_id":identity::hex(&identities.boot),
        "base_id":graphic.framebuffer_basis().base_id,"mapping":mapping,"cache":cache,
        "physical_scanout_proved":false,"gpu_acceleration_proved":false
    }))
    .unwrap_or_else(|_| refuse("framebuffer-mapping-evidence-encoding"));
    if proof.len() > 2048 {
        refuse("framebuffer-mapping-evidence-bound");
    }
    arch::present(b"CONDUIT_FRAMEBUFFER_MAPPING ");
    arch::present(&proof);
    arch::present(b"\n");
    arch::present(conduitos::firmware_face::SIGN_PREFIX);
    arch::present(graphic.evidence());
    arch::present(b"\n");
    Some(graphic)
}

fn refuse(reason: &str) -> ! {
    arch::disable_interrupts();
    arch::present(b"CONDUIT_AARCH64_PRODUCT_REFUSAL ");
    arch::present(reason.as_bytes());
    arch::present(b"\n");
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    use core::fmt::Write;
    arch::disable_interrupts();
    let mut diagnostic = conduitos::sign_format::FixedText::new();
    if let Some(location) = info.location() {
        let _ = writeln!(
            diagnostic,
            "CONDUIT_PANIC_LOCATION {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }
    let _ = writeln!(diagnostic, "CONDUIT_PANIC_DETAIL {}", info.message());
    let _ = writeln!(
        diagnostic,
        "CONDUIT_PANIC_ARENA live={} capacity={}",
        BOOT_ARENA.live_bytes(),
        BOOT_ARENA.capacity()
    );
    arch::present(diagnostic.as_bytes());
    refuse("panic")
}
