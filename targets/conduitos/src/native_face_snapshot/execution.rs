//! Fixed production kernel invocation; no native application dispatch.
use super::*;
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{PortDirection, bind_active_play};
use conduit_kernel::{
    Failure, FailureCode, FixedRoutes, FixedSignLog, FixedValueStore, PortId, SignSink,
    scheduler::{
        FixedScheduler, RemoteIngressOutcome, RemoteTerminalDisposition, SchedulerStatus, StepBack,
        StepInputBytes, StepIo, StepOutcome,
    },
};
use conduit_plan_lowering::lowering::{
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredForePort, LoweredPlanFragment,
};
use sha2::{Digest, Sha256};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const SIGNS: usize = 64;
const REMOTE_SIGNS: u16 = 16;
const WORK_STEPS: usize = 32;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
type Scheduler = FixedScheduler<
    SnapshotForward,
    FixedValueStore<2, MAX_SNAPSHOT_BYTES>,
    FixedSignLog<SIGNS>,
    1,
    2,
    PORTS,
    2,
    PORTS,
    2,
    1,
    1,
>;

pub struct PreparedFaceSnapshot<'a> {
    owner: core::marker::PhantomData<&'a mut NativeFaceSnapshotProducer>,
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    scheduler: Box<Scheduler>,
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    scheduler: crate::protected_runtime::snapshot::Snapshot,
    input: LoweredForePort,
    output: LoweredForePort,
    bytes: Vec<u8>,
    observed: Vec<u8>,
    receipt: FaceSnapshotReceipt,
    retired: bool,
}

impl NativeFaceSnapshotProducer {
    /// Values and output storage are bounded before the first kernel Step.
    /// This trusted local API does not authenticate arbitrary external snapshots.
    pub fn prepare_snapshot(
        &mut self,
        face: Face,
        observation_sequence: u64,
        play_sequence: u64,
    ) -> Result<PreparedFaceSnapshot<'_>, FaceSnapshotRefusal> {
        if self
            .last_play_sequence
            .is_some_and(|previous| play_sequence <= previous)
        {
            return Err(FaceSnapshotRefusal::PlaySequence);
        }
        face.validate().map_err(|_| FaceSnapshotRefusal::Face)?;
        let basis = &face.basis;
        // A zero-Body Birth Face is produced by this forwarding Plot. Once a
        // Body exists, its Face records the actual Body workload provenance;
        // forwarding must preserve that basis rather than claiming our relay
        // Plot made the Body's Plan or Play. This API has a trusted local caller.
        if basis.body_id.is_none()
            && (basis.source_document_id.as_ref() != Some(&self.plan.source_document_id)
                || basis.checked_plot_id.as_ref() != Some(&self.plan.checked_plot_id)
                || basis.expanded_plot_id.as_ref() != Some(&self.plan.expanded_plot_id)
                || basis.plan_id.as_ref() != Some(&self.plan.plan_id)
                || basis.active_play_id.is_some())
        {
            return Err(FaceSnapshotRefusal::ProducerBasis);
        }
        // The caller supplies facts, not a claim that any Body Play is active.
        // Actual Host-owned forwarding Play identity is retained in the receipt.
        let bytes = serde_json::to_vec(&face).map_err(|_| FaceSnapshotRefusal::Face)?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(FaceSnapshotRefusal::Capacity);
        }
        validate_shape(&self.plan, &self.lowered)?;
        let fragment = &self.plan.fragments[0];
        let active = bind_active_play(
            &self.plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let receipt = FaceSnapshotReceipt {
            observed_face_basis: basis.clone(),
            producer_plot: self.plot_identity(),
            producer_plan_id: self.plan.plan_id.clone(),
            producer_active_play_id: active.active_play_id,
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
            observation_sequence,
            face_id: face.identity.as_str().into(),
            face_revision: face.revision,
            encoded_bytes: bytes.len() as u32,
            value_sha256: Sha256::digest(&bytes).into(),
            kernel_signs: 0,
            fore_endpoints: self.lowered.fore_ports.len() as u16,
        };
        let boundary = |direction| {
            self.lowered
                .fore_ports
                .iter()
                .find(|port| port.direction == direction)
                .cloned()
                .ok_or(FaceSnapshotRefusal::Shape)
        };
        let input = boundary(PortDirection::Input)?;
        let output = boundary(PortDirection::Output)?;
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        let scheduler = native::prepare(&self.lowered, &input, &output)?;
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        let scheduler = {
            let mut routes = FixedRoutes::<PORTS, 2>::new(PORTS as u16);
            for route in &self.lowered.routes {
                routes
                    .install(
                        route.source_node,
                        route.source_port,
                        route.range,
                        &route.targets,
                    )
                    .map_err(|_| FaceSnapshotRefusal::Shape)?;
            }
            routes.seal().map_err(|_| FaceSnapshotRefusal::Shape)?;
            let values =
                FixedValueStore::<2, MAX_SNAPSHOT_BYTES>::new((2 * MAX_SNAPSHOT_BYTES) as u32)
                    .map_err(|_| FaceSnapshotRefusal::Capacity)?;
            let signs = FixedSignLog::<SIGNS>::new_with_remote_storage(
                (SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
                REMOTE_SIGNS,
                conduit_kernel::remote_sign_storage_bytes(REMOTE_SIGNS)
                    .ok_or(FaceSnapshotRefusal::Capacity)?,
            )
            .map_err(|_| FaceSnapshotRefusal::Capacity)?;
            let scheduler = FixedScheduler::new(
                [self.lowered.node_specs[0]],
                [self.lowered.cords[0].spec, self.lowered.cords[1].spec],
                routes,
                [SnapshotForward],
                values,
                signs,
            )
            .map_err(|_| FaceSnapshotRefusal::Kernel)?;
            Box::new(scheduler)
        };
        self.last_play_sequence = Some(play_sequence);
        Ok(PreparedFaceSnapshot {
            owner: core::marker::PhantomData,
            scheduler,
            input,
            output,
            bytes,
            observed: Vec::with_capacity(MAX_SNAPSHOT_BYTES),
            receipt,
            retired: false,
        })
    }
}

impl PreparedFaceSnapshot<'_> {
    pub fn run(mut self) -> Result<PublishedFaceSnapshot, FaceSnapshotRefusal> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            let (observed, signs) = self
                .scheduler
                .forward(&self.bytes)
                .map_err(|_| FaceSnapshotRefusal::Kernel)?;
            if observed != self.bytes {
                return Err(FaceSnapshotRefusal::ForeOutput);
            }
            self.observed = observed;
            self.retired = true;
            self.receipt.kernel_signs = signs;
            let face =
                serde_json::from_slice(&self.observed).map_err(|_| FaceSnapshotRefusal::Face)?;
            Ok(PublishedFaceSnapshot {
                face,
                receipt: self.receipt.clone(),
            })
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        self.run_local()
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    fn run_local(mut self) -> Result<PublishedFaceSnapshot, FaceSnapshotRefusal> {
        if self
            .scheduler
            .admit_remote_input(self.input.endpoint, self.input.cord, 0, &self.bytes)
            .map_err(|_| FaceSnapshotRefusal::ForeInput)?
            != (RemoteIngressOutcome::Accepted { sequence: 0 })
        {
            return Err(FaceSnapshotRefusal::ForeInput);
        }
        self.scheduler
            .close_remote_input(self.input.endpoint, self.input.cord)
            .map_err(|_| FaceSnapshotRefusal::ForeInput)?;
        let mut delivered = false;
        for _ in 0..WORK_STEPS {
            if self.scheduler.next_host_request().is_some() {
                return Err(FaceSnapshotRefusal::UnexpectedEffect);
            }
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.output.endpoint, self.output.cord)
                .map_err(|_| FaceSnapshotRefusal::ForeOutput)?
            {
                let value = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(|_| FaceSnapshotRefusal::ForeOutput)?;
                if delivered || value != self.bytes {
                    return Err(FaceSnapshotRefusal::ForeOutput);
                }
                self.observed.extend_from_slice(value);
                self.scheduler
                    .remote_egress_accept(self.output.endpoint, self.output.cord, offer.sequence)
                    .and_then(|_| {
                        self.scheduler.remote_egress_delivered(
                            self.output.endpoint,
                            self.output.cord,
                            offer.sequence,
                        )
                    })
                    .map_err(|_| FaceSnapshotRefusal::ForeOutput)?;
                delivered = true;
            }
            match self
                .scheduler
                .step()
                .map_err(|_| FaceSnapshotRefusal::Kernel)?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Drained => {
                    if !delivered
                        || self
                            .scheduler
                            .remote_egress_terminal_disposition(
                                self.output.endpoint,
                                self.output.cord,
                            )
                            .map_err(|_| FaceSnapshotRefusal::ForeOutput)?
                            != Some(RemoteTerminalDisposition::NormalClose)
                    {
                        return Err(FaceSnapshotRefusal::ForeOutput);
                    }
                    self.retired = true;
                    self.receipt.kernel_signs = self.scheduler.signs().len();
                    // Decode the bytes actually delivered, after execution has ended.
                    let face = serde_json::from_slice(&self.observed)
                        .map_err(|_| FaceSnapshotRefusal::Face)?;
                    return Ok(PublishedFaceSnapshot {
                        face,
                        receipt: self.receipt.clone(),
                    });
                }
                _ => return Err(FaceSnapshotRefusal::Kernel),
            }
        }
        Err(FaceSnapshotRefusal::WorkBound)
    }

    /// Cancelling never publishes a Face or fabricates a normal Fore close.
    pub fn cancel(mut self) -> Result<(), FaceSnapshotRefusal> {
        self.scheduler
            .cancel()
            .map_err(|_| FaceSnapshotRefusal::Kernel)?;
        self.retired = true;
        Ok(())
    }
}

impl Drop for PreparedFaceSnapshot<'_> {
    fn drop(&mut self) {
        if !self.retired {
            let _ = self.scheduler.cancel();
        }
    }
}

pub(super) fn validate_shape(
    plan: &Plan,
    lowered: &LoweredPlanFragment,
) -> Result<(), FaceSnapshotRefusal> {
    if !conduit_core::verify_plan(plan) || plan.fragments.len() != 1 {
        return Err(FaceSnapshotRefusal::Plan);
    }
    let fragment = &plan.fragments[0];
    if fragment.placements.len() != 1
        || lowered.nodes.len() != 1
        || lowered.cords.len() != 2
        || lowered.fore_ports.len() != 2
        || !lowered.host_calls.is_empty()
        || lowered.sign_items > SIGNS as u16
        || lowered.cord_value_slots > 2
        || lowered.cord_value_bytes > (2 * MAX_SNAPSHOT_BYTES) as u32
        || fragment.placements[0].kind_id.as_str() != conduit_face::FACE_TEE_KIND
        || fragment.placements[0].implementation_id.as_str() != planning::IMPLEMENTATION
    {
        return Err(FaceSnapshotRefusal::Shape);
    }
    for (direction, name) in [
        (PortDirection::Input, "snapshot"),
        (PortDirection::Output, "face"),
    ] {
        if lowered
            .fore_ports
            .iter()
            .filter(|port| {
                port.direction == direction
                    && port.front_port_id.as_str() == name
                    && port.item_capacity == 1
                    && port.byte_capacity == MAX_SNAPSHOT_BYTES as u32
            })
            .count()
            != 1
        {
            return Err(FaceSnapshotRefusal::Shape);
        }
    }
    Ok(())
}

use conduitos_runtime_core::snapshot::SnapshotForward;
const _: () = assert!(PORTS == conduitos_runtime_core::PORTS);
const _: () = assert!(MAX_SNAPSHOT_BYTES == conduitos_runtime_core::snapshot::VALUE_BYTES);
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
mod native;
