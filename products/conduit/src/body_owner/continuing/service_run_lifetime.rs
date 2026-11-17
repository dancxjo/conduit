//! Exact service entrance selection; resident Wake keeps the original worker.
use super::*;
use conduit_core::{CapabilityOffer, PlannedGear};
use conduit_std_host::{ThreadTimer, TimerAdapter};

fn face_implementation(id: &str) -> bool {
    conduit_std_offers::ambient_aspect_for_implementation(id).is_some()
        || matches!(
            id,
            conduit_std_host::measurement_chart_offer::MEASUREMENT_CHART_FACE_IMPLEMENTATION
                | conduit_std_offers::MEASUREMENT_CHART_FLOW_FACE_IMPLEMENTATION
                | conduit_std_offers::AMBIENT_OBSERVE_IMPLEMENTATION
                | conduit_std_offers::AMBIENT_RESIDENT_OBSERVE_IMPLEMENTATION
                | conduit_std_offers::AMBIENT_SOURCE_IMPLEMENTATION
                | conduit_std_offers::AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION
        )
}
pub(super) fn has_runtime_face(plan: &conduit_body::BodyPlan) -> bool {
    plan.plots
        .iter()
        .flat_map(|p| &p.plan.fragments)
        .flat_map(|f| &f.placements)
        .any(|p| face_implementation(p.implementation_id.as_str()))
}
fn exact(placement: &PlannedGear, offer: &CapabilityOffer) -> bool {
    placement.kind_id == offer.kind_id
        && placement.kind_contract_revision == offer.kind_contract_revision
        && placement.semantic_contract == offer.semantic_contract
        && placement.capability_id == offer.capability_id
        && placement.execution_profile_id == offer.implementation.execution_profile_id
        && placement.implementation_id == offer.implementation.implementation_id
        && placement.artifact_id == offer.implementation.artifact_id
        && placement.inputs == offer.inputs
        && placement.outputs == offer.outputs
        && placement.host_calls == offer.host_calls
        && placement.limits == offer.limits
}
/// A duration-free Wake is only for the exact original resident native source,
/// admitted timer, and two ambient facets. General Wake retains its old deadline.
pub(super) fn resident_until_lull(
    plan: &conduit_body::BodyPlan,
    wake: &Wake,
    host: &StdHost,
) -> Result<bool, String> {
    use conduit_std_offers::*;
    let placements = plan
        .plots
        .iter()
        .flat_map(|p| &p.plan.fragments)
        .flat_map(|f| &f.placements)
        .collect::<Vec<_>>();
    let attempted = placements.iter().any(|p| {
        p.implementation_id.as_str() == AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION
            || (p.kind_id.as_str() == conduit_semantic_catalog::AMBIENT_RESIDENT_OBSERVE_KIND
                && p.implementation_id.as_str() != AMBIENT_RESIDENT_OBSERVE_IMPLEMENTATION)
    });
    if !attempted {
        return Ok(false);
    }
    plan.validate_for(wake)
        .map_err(|e| format!("resident Wake Plan: {e:?}"))?;
    if host.image_identity().is_none()
        || plan.plots.len() != 1
        || plan.plots[0].plan.fragments.len() != 1
        || placements.len() != 4
        || !plan.plots[0].plan.fragments[0].fore_ports.is_empty()
    {
        return Err("resident ambient Wake requires one original closed IMAGE workset".into());
    }
    let expected = [
        time_every_offer(),
        ambient_resident_source_offer()?,
        ambient_resident_aspect_cpu_offer(AmbientDisplayAspect::Wallpaper)?,
        ambient_resident_aspect_cpu_offer(AmbientDisplayAspect::ClockWeather)?,
    ];
    for offer in &expected {
        if placements.iter().filter(|p| exact(p, offer)).count() != 1 {
            return Err(
                "resident ambient Wake differs from its exact source/timer/facet offers".into(),
            );
        }
    }
    let offer = &expected[1];
    let ad = host.advertisement();
    let found = host.native_source_authority_grants().any(|grant| {
        grant.host_id == ad.host_id
            && grant.boot_id == ad.boot_id
            && grant.capability_id == offer.capability_id
            && grant.subject_kind == offer.kind_id
            && grant.contract_id.as_str() == AMBIENT_RESIDENT_SOURCE_AUTHORITY
            && grant.host_call_contract_id.as_str() == AMBIENT_RESIDENT_SOURCE_CALL
    });
    if !found {
        return Err("resident ambient Wake lacks the exact current native source grant".into());
    }
    Ok(true)
}

pub(super) enum ServiceTimer {
    Deadline(DeadlineTimer),
    UntilLull { control: RunControl },
}
impl ServiceTimer {
    pub(super) fn new(deadline: Option<Duration>, control: RunControl) -> Self {
        match deadline {
            Some(duration) => Self::Deadline(DeadlineTimer {
                until: Instant::now() + duration,
                control,
            }),
            None => Self::UntilLull { control },
        }
    }
}
impl TimerAdapter for ServiceTimer {
    fn wait(&mut self, duration: Duration) {
        match self {
            Self::Deadline(timer) => timer.wait(duration),
            Self::UntilLull { control } => {
                let began = Instant::now();
                while began.elapsed() < duration && !control.stop_requested() {
                    thread::sleep(
                        Duration::from_millis(10).min(duration.saturating_sub(began.elapsed())),
                    );
                }
            }
        }
    }
    fn monotonic_ticks_on(&mut self, basis: &conduit_core::MonotonicClockIdentity) -> Option<u64> {
        ThreadTimer.monotonic_ticks_on(basis)
    }
    fn monotonic_observation(
        &mut self,
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
    ) -> Option<conduit_core::MonotonicInstant> {
        ThreadTimer.monotonic_observation(host, boot)
    }
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        ThreadTimer.monotonic_now_ms()
    }
    fn monotonic_now_micros(&mut self) -> Option<u64> {
        ThreadTimer.monotonic_now_micros()
    }
}

#[cfg(test)]
#[path = "service_run_lifetime_tests.rs"]
mod tests;
