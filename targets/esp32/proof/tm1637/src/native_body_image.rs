//! Trusted installed-image boundary. Membership is separate from Play.
use alloc::boxed::Box;
use conduit_body::SealedBodyLifecycleSession;
mod material {
    include!(concat!(env!("OUT_DIR"), "/body-bootstrap.rs"));
}

pub fn load(
    root: &crate::native_root::NativeRoot,
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    let evidence = material::evidence()?;
    let source = crate::native_software::planning_source()?;
    if source.gear_count() != conduit_environment_runtime::appliance::assigned::NODES {
        return Err("body-image-source-shape");
    }
    let source_identity = source
        .identity()
        .map_err(|_| "body-image-source-metadata")?;
    if source_identity != crate::native_software::source_identity()
        || !evidence.body.workset.plots().iter().any(|plot| {
            plot.source_document_id == source_identity.source_document_id
                && plot.checked_plot_id == source_identity.checked_plot_id
        })
    {
        return Err("body-image-source-workset");
    }
    drop(source_identity);
    let initialized = root.advertisement().map_err(|_| "body-image-root-scope")?;
    let owner = Box::new(
        conduit_environment_runtime::body_image::open_installed_image_sealed(
            evidence,
            &initialized,
        )
        .map_err(|_| "body-image-installation")?,
    );
    drop(initialized);
    let memory = esp_alloc::HEAP.stats();
    let current = owner
        .evidence()
        .membership
        .parts
        .iter()
        .find_map(|part| part.current.as_ref())
        .ok_or("body-image-current-member")?;
    // This is metadata from ordinary biography, not an inferred execution state.
    #[derive(serde::Serialize)]
    struct Report<'a> {
        event: &'static str,
        body_id: &'a conduit_body::BodyId,
        friendly_name: &'a str,
        host_id: &'a conduit_core::HostId,
        boot_id: &'a conduit_core::BootId,
        offer_generation: u64,
        membership_revision: u64,
        member_admitted: bool,
        linked_source_gears: usize,
        root_heap_size_bytes: usize,
        root_heap_used_bytes: usize,
        root_heap_peak_bytes: usize,
        play_started: bool,
    }
    let report = Report {
        event: "body-image-loaded",
        body_id: &owner.evidence().body_id,
        friendly_name: &owner.evidence().friendly_name,
        host_id: &current.host_id,
        boot_id: &current.boot_id,
        offer_generation: current.offer_generation.0,
        membership_revision: owner.evidence().membership.revision.0,
        member_admitted: true,
        linked_source_gears: source.gear_count(),
        root_heap_size_bytes: memory.size,
        root_heap_used_bytes: memory.current_usage,
        root_heap_peak_bytes: memory.max_usage,
        play_started: false,
    };
    let mut output = [0; 1024];
    let len =
        serde_json_core::to_slice(&report, &mut output).map_err(|_| "body-image-report-bound")?;
    let text = core::str::from_utf8(&output[..len]).map_err(|_| "body-image-report-encoding")?;
    esp_println::println!("{}", text);
    Ok(owner)
}
