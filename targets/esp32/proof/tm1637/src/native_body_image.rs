//! Trusted installed-image boundary. Membership is separate from Play.
use alloc::boxed::Box;
use conduit_body::SealedBodyLifecycleSession;
mod material {
    include!(concat!(env!("OUT_DIR"), "/body-bootstrap.rs"));
}

/// The actual exclusive flash region must be installed before Root can create
/// or publish its signer. This reconstructs the linked pristine evidence once.
#[cfg(feature = "body-persistence")]
pub fn initialize_restart(
    region: crate::native_evidence_region::BodyRegion,
) -> Result<(), &'static str> {
    crate::native_restart::initialize(region, material::evidence()?)
}

pub fn load(
    root: &crate::native_root::NativeRoot,
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    #[cfg(feature = "body-persistence")]
    let retained = crate::native_restart::take_body()?;
    #[cfg(feature = "body-persistence")]
    let restart_resumed = !retained.pristine;
    #[cfg(feature = "body-persistence")]
    let evidence = retained.evidence;
    #[cfg(not(feature = "body-persistence"))]
    let evidence = material::evidence()?;
    #[cfg(not(feature = "body-persistence"))]
    let restart_resumed = false;
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
    #[cfg(feature = "body-persistence")]
    let mut owner = Box::new(if retained.pristine {
        conduit_environment_runtime::body_image::open_installed_image_sealed(evidence, &initialized)
            .map_err(|_| "body-image-installation")?
    } else {
        SealedBodyLifecycleSession::resume_here(
            evidence,
            &initialized.host_id,
            &initialized.boot_id,
        )
        .map_err(|_| "body-image-restart-continuity")?
    });
    #[cfg(not(feature = "body-persistence"))]
    let owner = Box::new(
        conduit_environment_runtime::body_image::open_installed_image_sealed(
            evidence,
            &initialized,
        )
        .map_err(|_| "body-image-installation")?,
    );
    #[cfg(feature = "body-persistence")]
    {
        // Move old membership history into durable archive custody while cold
        // software is still absent. The exact segment is acknowledged only by
        // the following verified journal commit; no history is discarded.
        if owner.evidence().membership.events.len() >= 8 {
            owner
                .seal_membership_history_for_checkpoint()
                .map_err(|_| "body-image-membership-archive")?;
        }
        crate::native_restart::checkpoint(&mut owner, &root.restart_verifying_key())?;
    }
    #[cfg(feature = "body-persistence")]
    let checkpoint_generation = crate::native_restart::generation();
    #[cfg(not(feature = "body-persistence"))]
    let checkpoint_generation = None;
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
        restart_resumed: bool,
        checkpoint_generation: Option<u64>,
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
        restart_resumed,
        checkpoint_generation,
    };
    let mut output = [0; 1024];
    let len =
        serde_json_core::to_slice(&report, &mut output).map_err(|_| "body-image-report-bound")?;
    let text = core::str::from_utf8(&output[..len]).map_err(|_| "body-image-report-encoding")?;
    esp_println::println!("{}", text);
    Ok(owner)
}
