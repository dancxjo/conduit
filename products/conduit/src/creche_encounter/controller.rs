use super::*;
use std::sync::atomic::Ordering;

#[derive(Clone)]
pub struct CrecheEncounterController {
    pub(super) channel: Arc<Channel>,
}
impl CrecheEncounterController {
    pub fn face(&self) -> Result<Option<Arc<Face>>, String> {
        let state = self.channel.lock()?;
        Ok(if state.active {
            state.face.clone()
        } else {
            None
        })
    }
    pub fn acknowledge_show(&self, show: MaskShow) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.command.is_some() || state.checked.is_some() || state.started {
            return Err("Crèche cannot acknowledge inactive or occupied encounter".into());
        }
        let face = state.face.as_ref().ok_or("Crèche Face absent")?;
        show.validate(face)
            .map_err(|e| format!("Crèche Show: {e:?}"))?;
        if show.show.lifecycle != conduit_face::ManifestationLifecycle::Available {
            return Err("Crèche requires actual available Show".into());
        }
        self.channel
            .custody
            .try_lock()
            .map_err(|_| "Crèche selected Mask custody busy")?
            .validate_acknowledged(face, &show)?;
        state.acknowledged = Some(show);
        Ok(())
    }
    pub fn submit(&self, input: FaceInteraction) -> Result<(), String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.command.is_some() || state.checked.is_some() || state.started {
            return Err("Crèche interaction mailbox inactive, reviewed or full".into());
        }
        let face = state.face.as_ref().ok_or("Crèche current Face absent")?;
        let show = state
            .acknowledged
            .as_ref()
            .ok_or("Crèche actual acknowledged Show absent")?;
        input
            .validate_against(face, show)
            .map_err(|e| format!("Crèche interaction: {e:?}"))?;
        let mut custody = self
            .channel
            .custody
            .try_lock()
            .map_err(|_| "Crèche selected Mask custody busy")?;
        custody.validate_acknowledged(face, show)?;
        custody.validate_returned_interaction(face, show, &input)?;
        state.command = Some(Command {
            show: show.clone(),
            input,
        });
        Ok(())
    }

    /// Service-side effect pump, never called from kernel begin/poll. Starts
    /// exactly once after canonical checking; retries cannot repeat production.
    pub fn start_production_if_reviewed(&self) -> Result<bool, String> {
        let mut state = self.channel.lock()?;
        if !state.active || state.retired || state.started {
            return Ok(false);
        }
        let Some(configuration) = state.checked.take() else {
            return Ok(false);
        };
        let request = state
            .request
            .ok_or("Crèche reviewed operation lacks request")?;
        state.started = true;
        let cancelled = state.cancelled.clone();
        let channel = self.channel.clone();
        let worker = std::thread::Builder::new()
            .name("conduit-creche-make".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let artifact = channel
                        .inputs
                        .artifacts
                        .iter()
                        .find(|a| {
                            a.configuration.configuration_id() == configuration.configuration_id()
                                || a.bundle.as_ref().is_some_and(|bundle| {
                                    bundle.admits_native_target(&configuration)
                                })
                        })
                        .ok_or("no owner-reviewed artifact matches this checked Host")?;
                    if let Some(bundle) = &artifact.bundle {
                        return bundle.produce(
                            artifact,
                            &configuration,
                            &channel.binding,
                            &channel.inputs,
                            request,
                            &cancelled,
                        );
                    }
                    let file = artifact
                        .artifact
                        .try_clone()
                        .map_err(|e| format!("Crèche retain artifact handle: {e}"))?;
                    let id = format!("creche/request/{:x}", Sha256::digest(request.0));
                    let clock_failed = std::sync::atomic::AtomicBool::new(false);
                    conduit_body_make::produce_joining_host(
                        conduit_body_make::JoiningProductionRequest {
                            request_id: &id,
                            body: &channel.binding.biography,
                            invitation: &channel.binding.invitation.claim,
                            configuration: &configuration,
                            output: artifact.output.clone(),
                            artifact: conduit_body_make::ReviewedJoiningArtifact {
                                image: &artifact.image,
                                image_manifest: &artifact.image_manifest,
                                artifact_digest: &artifact.artifact_digest,
                                artifact_bytes: artifact.artifact_bytes,
                                source_identity: &artifact.source_identity,
                            },
                            bounds: channel.inputs.installation.production_bounds,
                        },
                        file,
                        &channel.inputs.catalog,
                        &channel.inputs.packages,
                        || match clock() {
                            Ok(now) => now,
                            Err(_) => {
                                clock_failed.store(true, Ordering::Release);
                                u64::MAX
                            }
                        },
                        || {
                            cancelled.load(Ordering::Acquire)
                                || clock_failed.load(Ordering::Acquire)
                        },
                    )
                }))
                .unwrap_or_else(|_| {
                    Err("Crèche production worker panicked; drain required".into())
                });
                // One worker owns the result slot. Keep custody even after a mutex
                // poison; the adapter will refuse further work and drain explicitly.
                channel
                    .state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .result = Some(result);
            });
        match worker {
            Ok(worker) => {
                state.worker = Some(worker);
                Ok(true)
            }
            Err(error) => {
                state.result = Some(Err(format!("Crèche production launch: {error}")));
                Ok(true)
            }
        }
    }

    /// Output custody transfers only after the installed operation retained its
    /// exact Prepared receipt. There is no externally supplied success setter.
    pub fn take_products(&self) -> Result<Option<ProducedJoiningHost<File>>, String> {
        let mut state = self.channel.lock()?;
        Ok(if state.published {
            state.products.take()
        } else {
            None
        })
    }
}
