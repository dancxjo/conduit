//! Existing bounded live staging with original IMAGE memory and Body Play custody.
use super::*;
use crate::body_live_fore::image::ImageLiveAdmission;
use crate::{BodyLiveForeAdmission, BodyLiveForeStatus};
use std::sync::Arc;

/// Private, non-deserializable admission. Clones retain the SAME shared queue
/// allocation/charge without cloning ID strings or granting another Play.
#[derive(Clone)]
pub struct ImageLiveForeQueue {
    queue: Arc<BodyLiveForeQueue>,
}
impl ImageLiveForeQueue {
    /// Producer codec/validation workspace, thread and control backing retain
    /// their independent Root owner. Acceptance is staging, not kernel commit.
    pub fn submit(&self, canonical: &[u8]) -> Result<BodyLiveForeAdmission, String> {
        self.queue.require_image_started()?;
        self.queue.submit(canonical)
    }
    pub fn close(&self) -> Result<(), String> {
        self.queue.require_image_started()?;
        self.queue.close()
    }
    pub fn status(&self) -> BodyLiveForeStatus {
        self.queue.status()
    }
    pub fn wait_until_play_started(&self) -> Result<(), String> {
        self.queue.wait_until_image_started()
    }
}
impl StdHost {
    /// Reserve before construction; retain observed requested backing on the
    /// original counter until the last queue/producer/runner owner drops.
    ///
    /// # Safety
    /// Root must independently qualify `preparation_bytes` for this COMPLETE
    /// synchronous construction peak, including Plan validation/identity work,
    /// all Arc/Vec/String capacities, binding metadata and bounded destruction.
    /// The argument is a requested envelope, not evidence that peak was verified;
    /// observing retained capacities afterwards does not qualify earlier peaks.
    /// Shared Plan/control backing and producer validation/codecs/threads have
    /// separate custody outliving their use. Effects must quiesce on refusal or
    /// unwind. This is cooperative requested backing, not allocator/RSS proof,
    /// operation authority, physical input ownership or permission to run.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn prepare_selected_image_live_fore_queue(
        &mut self,
        preparation_bytes: usize,
        wake: &Wake,
        plan: &conduit_body::BodyPlan,
        control: RunControl,
        front: &str,
        queue_slots: usize,
        maximum_items: u16,
    ) -> Result<ImageLiveForeQueue, String> {
        use crate::kernel_preparation::ImageMemoryLease;
        if !cfg!(target_os = "linux") || usize::BITS != 64 {
            return Err("selected live queue profile requires native 64-bit Linux".into());
        }
        if preparation_bytes < core::mem::size_of::<BodyLiveForeQueue>() {
            return Err("IMAGE live preparation envelope is too small".into());
        }
        let admitted = preparation_bytes
            .checked_add(ImageMemoryLease::owned_backing_bytes())
            .ok_or("IMAGE live preparation overflow")?;
        let memory = self
            .kernel_resources
            .reserve_managed_base_memory(admitted)?;
        let play = self.prospective_body_play(wake, plan)?;
        if plan.plots.len() != 1 || plan.plots[0].plan.fragments.len() != 1 {
            return Err("IMAGE live queue requires one exact local fragment".into());
        }
        let fragment = &plan.plots[0].plan.fragments[0];
        let queue = BodyLiveForeQueue::for_local_closing_flow_fragment(
            fragment,
            front,
            control,
            queue_slots,
            maximum_items,
        )?;
        queue.bind_image_admission(ImageLiveAdmission {
            host: self.advertisement.host_id.clone(),
            boot: self.advertisement.boot_id.clone(),
            offer: self.advertisement.offer_generation,
            play,
            memory,
        })?;
        let queue = Arc::new(queue);
        let retained = core::mem::size_of::<BodyLiveForeQueue>()
            .checked_add(2 * core::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(queue.image_clone_metadata_bytes()?))
            .and_then(|bytes| bytes.checked_add(queue.image_shared_backing_bytes()?))
            .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .ok_or("IMAGE live queue backing overflow")?;
        if retained > admitted {
            return Err("IMAGE live retained backing exceeds declared preparation envelope".into());
        }
        queue.shrink_image_retention(retained)?;
        Ok(ImageLiveForeQueue { queue })
    }
    /// Run the original admitted queue through the existing installed ingress.
    /// Neither a copied IMAGE hash nor an ordinary uncharged queue is accepted.
    #[allow(clippy::too_many_arguments)]
    pub fn run_selected_image_body_plan_with_live_face_to_with_start<W: Write, T: TimerAdapter, F>(
        &mut self,
        request: BodyRunRequest<'_>,
        queue: &ImageLiveForeQueue,
        fore: &mut dyn BodyForeOutputAdapter,
        face: &mut dyn BodyFaceContributionObserver,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<ImageBodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        if !self.kernel_resources.has_image_bounds() {
            return Err("selected live execution requires an IMAGE-bound Host".into());
        }
        let prospective = self.prospective_body_play(request.wake, request.plan)?;
        queue
            .queue
            .verify_image_admission(self, &prospective, request.control)?;
        let mut custody = None;
        let report = self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            None,
            None,
            None,
            Some((&queue.queue, fore)),
            None,
            None,
            None,
            started,
            Some(&mut custody),
            Some(face),
        )?;
        Ok(ImageBodyRunReport {
            report,
            _memory: custody.ok_or("selected live result lost original memory custody")?,
        })
    }
}

#[cfg(all(test, target_os = "linux", target_pointer_width = "64"))]
mod tests;
