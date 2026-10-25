//! Foreground native encounter on the installed process's retained Body and Host.
//! Grant paths are Host administration, never authored Plot or UI authority.
mod effects;
mod policy;
mod retention;
use super::owner::Owner;
use conduit::creche_encounter::CrecheEncounterAdapter;
use conduit::creche_native_owner::{
    prepare_linux_creche_output, LinuxCrecheTurn, LocalCrecheNativeLimits,
};
use conduit_std_host::{
    body_execution::BodyForeOutputAdapter,
    hosted_creche::{CrecheLimits, HostedCrecheAdapter},
    ExternalForeDelivery,
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

struct OtherFore;
impl BodyForeOutputAdapter for OtherFore {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        Err("native Crèche has no admitted consumer for this other Plot's external Fore".into())
    }
}

pub(crate) fn run(owner: &mut Owner, root: &Path, grants: &Path) -> Result<(), String> {
    if !owner.selected_speech_host_is_idle() {
        return Err("native Crèche requires the retained idle Body owner".into());
    }
    let identity = super::owner::verified_native_image(root)?;
    let policy = policy::Policy::load(grants)?;
    let draft = policy.draft()?;
    let profiles = draft
        .construction()
        .hosts()
        .iter()
        .map(|h| h.configuration.clone())
        .collect();
    let encounter = crate::birth_identity::fresh_uuid()?;
    let directory = policy.staging_directory.join(&encounter);
    std::fs::create_dir(&directory)
        .map_err(|e| format!("create retained Crèche operation: {e}"))?;
    super::restrict_directory(&directory)?;
    let save_directory = policy.save_directory.clone();
    let maximum_bytes = policy.maximum_total_bytes;
    let maximum_millis = policy.maximum_millis;
    let custody = Arc::new(Mutex::new(effects::Manufacture::default()));
    let mut policy = Some(policy);
    let dimensions = policy
        .as_ref()
        .map(|p| (p.width, p.height))
        .ok_or("Crèche grants absent")?;
    let result =
        conduit_linux_display::with_kmsdrm(dimensions.0, dimensions.1, |scanout, events| {
            let initialized = scanout.initialized_display()?;
            owner
                .host
                .attach_initialized_display(initialized)
                .map_err(|e| format!("native display attachment: {e:?}"))?;
            let resource_pool = format!("creche/operation/{encounter}");
            let slot = conduit_core::resource_offer(
                &resource_pool,
                conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
                1,
            );
            owner
                .attach_creche_encounter(slot.clone())
                .map_err(|e| format!("Crèche attachment: {e:?}"))?;
            // Both attachments precede the generation-bound Mask and ordinary Plan.
            let (mut native, mask_custody) = prepare_linux_creche_output(
                owner.host.current(),
                scanout,
                &identity,
                LocalCrecheNativeLimits {
                    maximum_presentations: 4096,
                    maximum_input_events: 65536,
                    retirement_timeout: Duration::from_secs(10),
                },
            )?;
            let (mut adapter, controller) = CrecheEncounterAdapter::new(
                draft,
                profiles,
                encounter.clone(),
                resource_pool.into(),
                CrecheLimits {
                    maximum_evidence_bytes: 8 * 1024 * 1024,
                    maximum_polls: 1_000_000,
                    minimum_poll_interval_ms: 100,
                },
                4096,
                mask_custody,
            )?;
            let admitted = (|| {
                if adapter.resource_offer() != slot {
                    return Err("Crèche resource attachment differs".into());
                }
                owner.admit_shipped_creche(root, owner.workload_revision())?;
                owner.plan_creche_workset(root)
            })();
            if let Err(error) = admitted {
                let retirement = native.retire();
                let drain = adapter.cancel_and_drain();
                return Err(format!(
                    "{error}; retirement={retirement:?}; drain={drain:?}"
                ));
            }
            let mut worker =
                match owner.start_creche_run(root, maximum_millis, adapter, Box::new(OtherFore)) {
                    Ok(worker) => worker,
                    Err(refusal) => {
                        let (reason, mut adapter, _other) = refusal.into_parts();
                        let retirement = native.retire();
                        let drain = adapter.cancel_and_drain();
                        return Err(format!(
                            "{reason}; retirement={retirement:?}; drain={drain:?}"
                        ));
                    }
                };
            let mut error = None;
            let mut stopping = false;
            let mut stopped_after_outcome = false;
            loop {
                match native.service_retirement() {
                    Ok(true) => {
                        stopping = true;
                        let _ = worker.request_lull();
                    }
                    Ok(false) => {}
                    Err(reason) => {
                        stopping = true;
                        error.get_or_insert(reason);
                        let _ = worker.request_lull();
                    }
                }
                match worker.progress(owner, root, |context, play| {
                    native.bind_execution(context, play)
                }) {
                    Ok(true) => break,
                    Ok(false) => {}
                    Err(reason) => {
                        stopping = true;
                        error.get_or_insert(reason);
                        let _ = worker.request_lull();
                        if worker.requires_owner_restart() {
                            let _ = native.retire();
                            return Err(error
                                .unwrap_or_else(|| "native Body worker requires restart".into()));
                        }
                    }
                }
                if !stopping && worker.outcome_returned() {
                    // Canonical Done/refusal ended this encounter. Restore the
                    // foreground's initially Lulled state even if other Plots loop.
                    // This notification is not ACK; the terminal report proves ACK.
                    stopping = true;
                    stopped_after_outcome = true;
                    let _ = worker.request_lull();
                }
                if error.is_none() && !stopping {
                    match native.pump_linux(&controller, events, 128) {
                        Ok(LinuxCrecheTurn::Quit | LinuxCrecheTurn::Retired) => {
                            stopping = true;
                            let _ = worker.request_lull();
                        }
                        Ok(_) => {}
                        Err(reason) => {
                            error = Some(format!("native Crèche input/presentation: {reason:?}"));
                            let _ = worker.request_lull();
                            let _ = native.retire();
                        }
                    }
                    if error.is_none() && !stopping {
                        let reviewed = match controller.reviewed() {
                            Ok(reviewed) => reviewed,
                            Err(reason) => {
                                error = Some(reason);
                                let _ = worker.request_lull();
                                false
                            }
                        };
                        if reviewed {
                            if let Some(grants) = policy.take() {
                                let destination = directory.clone();
                                let retained = custody.clone();
                                if let Err(reason) =
                                    controller.start_make_worker(move |handoff, cancelled| {
                                        effects::produce(
                                            handoff,
                                            cancelled,
                                            grants,
                                            destination,
                                            retained,
                                        )
                                    })
                                {
                                    error = Some(reason);
                                    let _ = worker.request_lull();
                                }
                            }
                        }
                        if let Err(reason) = effects::service_images(
                            &controller,
                            &save_directory,
                            &directory,
                            maximum_bytes,
                        ) {
                            error = Some(reason);
                            let _ = worker.request_lull();
                        }
                    }
                }
                // This is trusted hosted UI scheduling, never a kernel timing claim.
                std::thread::sleep(Duration::from_millis(1));
            }
            if let Err(reason) = native.retire() {
                error.get_or_insert(reason);
            }
            let completion = worker
                .take_completion()
                .ok_or("native Crèche lacks retained terminal completion")?;
            let facts = retention::CompletionFacts {
                outcome_acknowledged: completion.outcome_acknowledged(),
                stopped_after_outcome,
            };
            let (adapter, delivery, _other) = completion.into_parts();
            retention::finish(
                &controller,
                &adapter,
                delivery.as_ref(),
                &owner.truth()["last_execution"],
                &custody,
                facts,
                &directory,
                error.as_deref(),
                &identity,
            )
        });
    if result.is_ok() {
        println!("Prepared media and exact evidence retained in {}. Boot and join have not been observed.", directory.display());
    }
    result
}
