//! Exact retained Body lifecycle and terminal report ownership.
use super::*;
use crate::body_execution::BodyRunReport;
use conduit_body::{BodyPlayIdentity, Wake, WakeLifecycleEvent};
use conduit_core::SignIdentity;

pub(crate) fn lifecycle_bytes(
    play: &BodyPlayIdentity,
    wake: &Wake,
    sign: &SignIdentity,
) -> Result<usize, String> {
    let mut bytes = wake
        .workset
        .retained_backing_bytes()
        .ok_or("Body immutable workset backing overflow")?;
    for count in [
        play.active_play_id.0.capacity(),
        play.body_id.retained_backing_bytes(),
        play.wake_id.retained_backing_bytes(),
        play.plan_id.0.capacity(),
        wake.wake_id.retained_backing_bytes(),
        wake.body_id.retained_backing_bytes(),
        sign.sign_id.0.capacity(),
        sign.host_id.0.capacity(),
        sign.boot_id.0.capacity(),
        sign.active_play_id.as_ref().map_or(0, |id| id.0.capacity()),
        vector(&wake.plans)?,
        vector(&wake.rejections)?,
        vector(&wake.sign_ids)?,
        vector(&wake.events)?,
    ] {
        add(&mut bytes, count)?;
    }
    for plan in &wake.plans {
        if plan.hold.is_some() {
            return Err("local IMAGE result does not support held-plan metadata".into());
        }
        add(&mut bytes, plan.plan_id.0.capacity())?;
        add(
            &mut bytes,
            plan.active_play_id.as_ref().map_or(0, |id| id.0.capacity()),
        )?;
    }
    for id in &wake.sign_ids {
        add(&mut bytes, id.0.capacity())?;
    }
    for rejection in &wake.rejections {
        for count in [
            rejection.reason_code.capacity(),
            rejection.category.capacity(),
            rejection.stage.capacity(),
            rejection.resource.capacity(),
            rejection.host_id.0.capacity(),
            rejection.boot_id.0.capacity(),
            rejection.plan_id.as_ref().map_or(0, |id| id.0.capacity()),
            vector(&rejection.checked_plot_ids)?,
        ] {
            add(&mut bytes, count)?;
        }
        for id in &rejection.checked_plot_ids {
            add(&mut bytes, id.0.capacity())?;
        }
    }
    for event in &wake.events {
        add(&mut bytes, event.sign_id().0.capacity())?;
        match event {
            WakeLifecycleEvent::Woke { .. }
            | WakeLifecycleEvent::Lulled { .. }
            | WakeLifecycleEvent::Failed { .. } => {}
            WakeLifecycleEvent::PlanReady { plan_id, .. }
            | WakeLifecycleEvent::BecameUnsatisfied { plan_id, .. }
            | WakeLifecycleEvent::SamePlanObserved { plan_id, .. } => {
                add(&mut bytes, plan_id.0.capacity())?
            }
            WakeLifecycleEvent::PlayStarted {
                plan_id,
                active_play_id,
                ..
            }
            | WakeLifecycleEvent::HeldPlanReleased {
                plan_id,
                active_play_id,
                ..
            } => {
                add(&mut bytes, plan_id.0.capacity())?;
                add(&mut bytes, active_play_id.0.capacity())?;
            }
            WakeLifecycleEvent::PlanHeld {
                prior_plan_id,
                plan_id,
                basis_sign_ids,
                ..
            } => {
                add(
                    &mut bytes,
                    prior_plan_id.as_ref().map_or(0, |id| id.0.capacity()),
                )?;
                add(&mut bytes, plan_id.0.capacity())?;
                add(&mut bytes, vector(basis_sign_ids)?)?;
                for id in basis_sign_ids {
                    add(&mut bytes, id.0.capacity())?;
                }
            }
            WakeLifecycleEvent::HeldPlanInvalidated {
                plan_id,
                current_basis_sign_ids,
                ..
            } => {
                add(&mut bytes, plan_id.0.capacity())?;
                add(&mut bytes, vector(current_basis_sign_ids)?)?;
                for id in current_basis_sign_ids {
                    add(&mut bytes, id.0.capacity())?;
                }
            }
            WakeLifecycleEvent::WorkloadChanged {
                prior_plan_id,
                prior_workset,
                replacement_workset,
                ..
            } => {
                add(&mut bytes, prior_plan_id.0.capacity())?;
                add(
                    &mut bytes,
                    prior_workset
                        .retained_backing_bytes()
                        .ok_or("prior workset backing overflow")?,
                )?;
                add(
                    &mut bytes,
                    replacement_workset
                        .retained_backing_bytes()
                        .ok_or("replacement workset backing overflow")?,
                )?;
            }
            WakeLifecycleEvent::Replanned {
                prior_plan_id,
                replacement_plan_id,
                ..
            } => {
                add(&mut bytes, prior_plan_id.0.capacity())?;
                add(&mut bytes, replacement_plan_id.0.capacity())?;
            }
        }
    }
    Ok(bytes)
}
pub(crate) fn result_bytes(report: &BodyRunReport) -> Result<usize, String> {
    let mut bytes = lifecycle_bytes(&report.play, &report.wake_at_start, &report.terminal_sign)?;
    for map in &report.partitions {
        add(&mut bytes, identities(map)?)?;
    }
    add(
        &mut bytes,
        crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes(),
    )?;
    add(&mut bytes, vector(&report.fore_deliveries)?)?;
    for delivery in &report.fore_deliveries {
        add(
            &mut bytes,
            delivery
                .retained_backing_bytes()
                .ok_or("Fore result backing overflow")?,
        )?;
    }
    if let Some(pending) = &report.fore_pending_output {
        add(
            &mut bytes,
            pending
                .delivery
                .retained_backing_bytes()
                .ok_or("pending Fore backing overflow")?,
        )?;
    }
    for count in [
        vector(&report.partitions)?,
        vector(&report.requests)?,
        vector(&report.kernel_events)?,
        vector(&report.clock_observations)?,
        report.failure.as_ref().map_or(0, String::capacity),
        report.cleanup_failure.as_ref().map_or(0, String::capacity),
    ] {
        add(&mut bytes, count)?;
    }
    for observation in &report.clock_observations {
        add(
            &mut bytes,
            observation
                .time
                .local()
                .clock()
                .retained_backing_bytes()
                .ok_or("result clock backing overflow")?,
        )?;
    }
    Ok(bytes)
}
