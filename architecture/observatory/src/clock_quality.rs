//! Safe inspection of authoritative, already assessed clock quality.

use alloc::{boxed::Box, string::String};
use conduit_core::{
    BodyTimeEstimate, BodyTimeQuality, BodyTimeRefusal, ClockProvenance, TemporalScale,
};
use serde::{Deserialize, Serialize};

use crate::CausalExplanationVisibility;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockQualityExplanation {
    Ready {
        now: Option<Box<ClockEstimateExplanation>>,
        horizon: Option<Box<ClockEstimateExplanation>>,
    },
    Degrading {
        now: Option<Box<ClockEstimateExplanation>>,
        reason: BodyTimeRefusal,
    },
    Unsupported {
        reason: BodyTimeRefusal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockEstimateExplanation {
    pub body_basis: String,
    pub generation: u64,
    pub correlation_age_ticks: u64,
    pub correlation_age_scale: TemporalScale,
    pub earliest_ticks: u64,
    pub center_ticks: u64,
    pub latest_ticks: u64,
    pub scale: TemporalScale,
    pub local_host: String,
    pub local_boot: String,
    pub local_basis: String,
    pub local_ticks: u64,
    pub local_scale: TemporalScale,
    pub source: ClockQualitySourceExplanation,
}

/// References here identify policy; they never contain admission or bearer data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockQualitySourceExplanation {
    Peer {
        host: String,
        boot: String,
        policy: String,
    },
    External {
        provider: String,
        policy: String,
    },
}

/// The caller owns authorization and supplies current runtime truth. This
/// projection neither assesses a clock nor admits a source. Public inspection
/// retains quality loss and its exact reason without correlation identities.
pub fn explain_clock_quality(
    quality: &BodyTimeQuality,
    visibility: CausalExplanationVisibility,
) -> Result<ClockQualityExplanation, BodyTimeRefusal> {
    quality.validate()?;
    let estimate = |value: &BodyTimeEstimate| {
        (visibility == CausalExplanationVisibility::Operator)
            .then(|| Box::new(project_estimate(value)))
    };
    Ok(match quality {
        BodyTimeQuality::Ready { now, horizon } => ClockQualityExplanation::Ready {
            now: estimate(now),
            horizon: estimate(horizon),
        },
        BodyTimeQuality::Degrading { now, reason } => ClockQualityExplanation::Degrading {
            now: estimate(now),
            reason: *reason,
        },
        BodyTimeQuality::Unsupported { reason } => {
            ClockQualityExplanation::Unsupported { reason: *reason }
        }
    })
}

fn project_estimate(value: &BodyTimeEstimate) -> ClockEstimateExplanation {
    let local = value.local_sample.clock();
    ClockEstimateExplanation {
        body_basis: value.body_basis.clone(),
        generation: value.generation,
        correlation_age_ticks: value.correlation_age_ticks,
        correlation_age_scale: local.scale(),
        earliest_ticks: value.earliest_ticks,
        center_ticks: value.center_ticks,
        latest_ticks: value.latest_ticks,
        scale: value.scale,
        local_host: local.host_id().as_str().into(),
        local_boot: local.boot_id().as_str().into(),
        local_basis: local.basis_id().into(),
        local_ticks: value.local_sample.ticks(),
        local_scale: local.scale(),
        source: match &value.provenance {
            ClockProvenance::Peer {
                host_id,
                boot_id,
                policy_id,
                ..
            } => ClockQualitySourceExplanation::Peer {
                host: host_id.as_str().into(),
                boot: boot_id.as_str().into(),
                policy: policy_id.clone(),
            },
            ClockProvenance::External {
                provider_id,
                policy_id,
                ..
            } => ClockQualitySourceExplanation::External {
                provider: provider_id.clone(),
                policy: policy_id.clone(),
            },
        },
    }
}
