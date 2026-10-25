//! Owner-started image effects, outside the generic kernel poll. One slot owns
//! make/save/finalize threads; every product batch is restored even on panic.
use super::*;
use crate::creche_production::CrecheSaveRefusal;
use std::{
    fs::File,
    sync::atomic::{AtomicBool, Ordering},
};

enum Effect {
    Save {
        host: String,
        output: File,
        maximum: u64,
    },
    Finish,
}
enum Completion {
    Saved(CrecheSavedReceipt),
    Finished(Vec<u8>),
    Refused(&'static str),
    FinishRefused(CrecheMakeRefusal, &'static str),
}
impl CrecheEncounterController {
    /// Consume only this encounter's pending canonical Save. The Host supplies
    /// an already admitted empty read/write regular file and finite byte grant.
    /// Any partial output remains in that caller-owned filesystem on refusal.
    pub fn start_save(&self, output: File, maximum_bytes: u64) -> Result<(), (String, File)> {
        // Keep the original admitted handle until spawn succeeds. The worker
        // clone refers to the same grant; no path is reopened or authority added.
        let worker_output = match output.try_clone() {
            Ok(file) => file,
            Err(error) => return Err((format!("Crèche save grant clone: {error}"), output)),
        };
        self.start_image_effect(Some((worker_output, maximum_bytes)))
            .map_err(|error| (error, output))
    }
    /// Consume only canonical Done. Revalidate all files and construct retained
    /// evidence off-thread; poll publishes Prepared only from that completion.
    pub fn start_finish_images(&self) -> Result<(), String> {
        self.start_image_effect(None)
    }
    fn start_image_effect(&self, save: Option<(File, u64)>) -> Result<(), String> {
        // Reserve before reaping so concurrent owner calls cannot attach twice.
        let prior = {
            let mut worker = self
                .channel
                .worker
                .lock()
                .map_err(|_| "Crèche worker poisoned")?;
            if worker.image_reserved
                || worker.drain_failed
                || worker.handle.as_ref().is_some_and(|h| !h.is_finished())
            {
                return Err("Crèche previous worker has not drained".into());
            }
            worker.image_reserved = true;
            worker.handle.take()
        };
        // Joining a known finished handle happens only here, never in poll or
        // while holding channel/worker/custody locks.
        if matches!(prior.map(|h| h.join()), Some(Err(_))) {
            let mut worker = self
                .channel
                .worker
                .lock()
                .map_err(|_| "Crèche worker poisoned")?;
            worker.drain_failed = true;
            worker.image_reserved = false;
            return Err("Crèche previous worker panicked".into());
        }
        let mut worker = self
            .channel
            .worker
            .lock()
            .map_err(|_| "Crèche worker poisoned")?;
        let prepared = (|| {
            let mut state = self.channel.lock()?;
            if !state.active || state.products.is_none() {
                return Err("Crèche Images not active".into());
            }
            let images = state.images.as_mut().ok_or("Crèche Images absent")?;
            if images.busy {
                return Err("Crèche Images busy".into());
            }
            let effect = match (&images.pending, save) {
                (Some(Pending::Save(request)), Some((output, maximum))) if maximum > 0 => {
                    Effect::Save {
                        host: request.host_entry().into(),
                        output,
                        maximum,
                    }
                }
                (Some(Pending::Done), None) => Effect::Finish,
                _ => return Err("Crèche effect requires its exact pending canonical action".into()),
            };
            images.pending = None;
            images.busy = true;
            images.notice = "Verifying image bytes. No boot or join has been observed.".into();
            if let Err(error) = refresh(&mut state) {
                if let Some(images) = state.images.as_mut() {
                    images.busy = false;
                }
                state.make_refusal = Some(CrecheMakeRefusal::BoundExhausted);
                return Err(error);
            }
            Ok(effect)
        })();
        let effect = match prepared {
            Ok(effect) => effect,
            Err(error) => {
                worker.image_reserved = false;
                return Err(error);
            }
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        worker.cancelled = Some(cancelled.clone());
        let controller = self.clone();
        let spawned = std::thread::Builder::new().name("creche-images".into()).spawn(move || {
            let (mut products, saved) = {
                let mut state = controller.channel.state.lock().unwrap_or_else(|e| e.into_inner());
                if !state.active || cancelled.load(Ordering::Acquire) { return; }
                let saved = state.images.as_ref().map(|images| images.saved.clone()).unwrap_or_default();
                match state.products.take() { Some(products) => (products, saved), None => return }
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if cancelled.load(Ordering::Acquire) { return Completion::Refused("Image operation cancelled"); }
                match effect {
                    Effect::Save { host, mut output, maximum } => {
                        match products.save_host(&host, &mut output, maximum, &cancelled) {
                            Ok(receipt) => Completion::Saved(receipt),
                            Err(error) => Completion::Refused(save_refusal(&error)),
                        }
                    }
                    Effect::Finish => {
                        if products.revalidate().is_err() { return Completion::FinishRefused(CrecheMakeRefusal::ArtifactRejected, "Prepared image verification failed"); }
                        if cancelled.load(Ordering::Acquire) { return Completion::Refused("Image operation cancelled"); }
                        match completion_evidence(&products, &saved, controller.channel.maximum_evidence_bytes) {
                            Ok(evidence) => Completion::Finished(evidence),
                            Err(_) => Completion::FinishRefused(CrecheMakeRefusal::BoundExhausted, "Prepared evidence bound or validation failed"),
                        }
                    }
                }
            }));
            let panicked = result.is_err();
            {
                let mut state = controller.channel.state.lock().unwrap_or_else(|e| e.into_inner());
                state.products = Some(products);
                let active = state.active && !cancelled.load(Ordering::Acquire);
                let mut final_refusal = None;
                if let Some(images) = state.images.as_mut() {
                    images.busy = false;
                    match result {
                        Ok(Completion::Saved(receipt)) => {
                            images.saved.retain(|saved| saved.host_entry() != receipt.host_entry());
                            images.notice = format!("Saved image for {}. Boot and membership have not been observed.", receipt.host_entry());
                            images.saved.push(receipt);
                        }
                        Ok(Completion::Finished(evidence)) if active => { images.evidence = Some(evidence); }
                        Ok(Completion::Finished(_)) => {}
                        Ok(Completion::Refused(reason)) => images.notice = reason.into(),
                        Ok(Completion::FinishRefused(reason, notice)) => {
                            images.notice = notice.into();
                            final_refusal = Some(reason);
                        }
                        Err(_) => images.notice = "Image worker failed; products retained".into(),
                    }
                }
                if active { state.make_refusal = final_refusal; }
                if active && refresh(&mut state).is_err() {
                    state.make_refusal = Some(CrecheMakeRefusal::BoundExhausted);
                }
            }
            if panicked {
                controller.channel.worker.lock().unwrap_or_else(|e| e.into_inner()).drain_failed = true;
            }
        });
        worker.image_reserved = false;
        match spawned {
            Ok(handle) => {
                worker.handle = Some(handle);
                Ok(())
            }
            Err(error) => {
                let mut state = self.channel.lock()?;
                if let Some(images) = state.images.as_mut() {
                    images.busy = false;
                    images.notice = "Image worker unavailable; products retained".into();
                }
                refresh(&mut state)?;
                Err(format!("Crèche image worker unavailable: {error}"))
            }
        }
    }
}
fn save_refusal(error: &CrecheSaveRefusal) -> &'static str {
    match error {
        CrecheSaveRefusal::UnknownHost => "Save refused: unknown Host",
        CrecheSaveRefusal::InvalidOutput => {
            "Save refused: output must be an admitted empty RW regular file"
        }
        CrecheSaveRefusal::BoundExhausted => "Save refused: byte grant exhausted",
        CrecheSaveRefusal::Cancelled => "Save cancelled; partial output retained",
        CrecheSaveRefusal::SourceChanged => "Save refused: prepared image changed",
        CrecheSaveRefusal::ReadbackMismatch => "Save refused: independent readback mismatch",
        CrecheSaveRefusal::Io(_) => "Save failed: I/O error; partial output retained",
    }
}
