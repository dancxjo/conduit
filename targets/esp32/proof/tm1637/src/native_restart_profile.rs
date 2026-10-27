//! Native durable profile: every canonical field is retained, but imported HOLD
//! custody is unsupported. No rich Core Plan deserializer is instantiated here.
use alloc::{string::String, vec::Vec};
use conduit_body::*;
use serde::{Deserialize, Deserializer};

#[derive(Default)]
struct NoHold;
impl<'de> Deserialize<'de> for NoHold {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct NullOnly;
        impl<'de> serde::de::Visitor<'de> for NullOnly {
            type Value = NoHold;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("null; native durable HOLD import is unsupported")
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<NoHold, E> {
                Ok(NoHold)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<NoHold, E> {
                Ok(NoHold)
            }
            fn visit_some<D: Deserializer<'de>>(self, _: D) -> Result<NoHold, D::Error> {
                Err(serde::de::Error::custom(
                    "native restart HOLD is unsupported",
                ))
            }
        }
        decoder.deserialize_option(NullOnly)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    schema: String,
    body_id: BodyId,
    friendly_name: String,
    body: Body,
    #[serde(default)]
    emergency: Option<DurableEmergencyConfiguration>,
    membership: BodyMembership,
    graduation: Option<BodyGraduationEvidence>,
    records: Vec<BodyBiographyRecord>,
    #[serde(default)]
    compaction: Option<BodyBiographyCompaction>,
    #[serde(default)]
    wakes: Vec<LocalWake>,
}
impl From<Evidence> for BodyBiographyEvidence {
    fn from(value: Evidence) -> Self {
        Self {
            schema: value.schema,
            body_id: value.body_id,
            friendly_name: value.friendly_name,
            body: value.body,
            emergency: value.emergency,
            membership: value.membership,
            graduation: value.graduation,
            records: value.records,
            compaction: value.compaction,
            wakes: value.wakes.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalWake {
    wake_id: WakeId,
    body_id: BodyId,
    workset: BodyWorkset,
    workload_revision: u64,
    wake_sequence: u64,
    lifecycle: WakeLifecycle,
    plans: Vec<LocalPlan>,
    #[serde(default)]
    rejections: Vec<WakeRejectionEvidence>,
    sign_ids: Vec<conduit_core::SignId>,
    events: Vec<WakeLifecycleEvent>,
}
impl From<LocalWake> for Wake {
    fn from(value: LocalWake) -> Self {
        Self {
            wake_id: value.wake_id,
            body_id: value.body_id,
            workset: value.workset,
            workload_revision: value.workload_revision,
            wake_sequence: value.wake_sequence,
            lifecycle: value.lifecycle,
            plans: value.plans.into_iter().map(Into::into).collect(),
            rejections: value.rejections,
            sign_ids: value.sign_ids,
            events: value.events,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalPlan {
    plan_id: conduit_core::PlanId,
    active_play_id: Option<conduit_core::ActivePlayId>,
    state: WakePlanState,
    #[serde(default)]
    #[serde(rename = "hold")]
    _hold: NoHold,
}
impl From<LocalPlan> for WakePlan {
    fn from(value: LocalPlan) -> Self {
        Self {
            plan_id: value.plan_id,
            active_play_id: value.active_play_id,
            state: value.state,
            hold: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    schema: String,
    body_id: BodyId,
    ordinal: u64,
    previous_digest: Option<[u8; 32]>,
    records: Vec<BodyBiographyRecord>,
    #[serde(default)]
    wakes: Vec<LocalWake>,
    #[serde(default)]
    body_events: Vec<BodyLifecycleEvent>,
    #[serde(default)]
    membership_events: Vec<MembershipEvent>,
    digest: [u8; 32],
}
impl From<Archive> for BodyBiographyArchiveSegment {
    fn from(value: Archive) -> Self {
        Self {
            schema: value.schema,
            body_id: value.body_id,
            ordinal: value.ordinal,
            previous_digest: value.previous_digest,
            records: value.records,
            wakes: value.wakes.into_iter().map(Into::into).collect(),
            body_events: value.body_events,
            membership_events: value.membership_events,
            digest: value.digest,
        }
    }
}

pub fn deserialize_evidence<'de, D: Deserializer<'de>>(
    decoder: D,
) -> Result<BodyBiographyEvidence, D::Error> {
    Evidence::deserialize(decoder).map(Into::into)
}
pub(crate) fn deserialize_archive<'de, D: Deserializer<'de>>(
    decoder: D,
) -> Result<BodyBiographyArchiveSegment, D::Error> {
    Archive::deserialize(decoder).map(Into::into)
}
pub(crate) fn ensure_archive_no_holds(
    segment: &BodyBiographyArchiveSegment,
) -> Result<(), &'static str> {
    if segment
        .wakes
        .iter()
        .flat_map(|wake| &wake.plans)
        .any(|plan| plan.hold.is_some())
    {
        Err("restart-hold-unsupported")
    } else {
        Ok(())
    }
}
pub fn deserialize_archives<'de, D: Deserializer<'de>>(
    decoder: D,
) -> Result<Vec<BodyBiographyArchiveSegment>, D::Error> {
    Vec::<Archive>::deserialize(decoder).map(|values| values.into_iter().map(Into::into).collect())
}
/// Must run before any journal erasure or write, including pending archives.
pub fn ensure_no_holds(
    evidence: &BodyBiographyEvidence,
    archives: &[BodyBiographyArchiveSegment],
) -> Result<(), &'static str> {
    if evidence
        .wakes
        .iter()
        .chain(archives.iter().flat_map(|archive| archive.wakes.iter()))
        .any(|wake| wake.plans.iter().any(|plan| plan.hold.is_some()))
    {
        return Err("restart-hold-unsupported");
    }
    Ok(())
}

use serde::Serialize;
#[derive(Serialize)]
pub struct EvidenceRef<'a> {
    schema: &'a String,
    body_id: &'a BodyId,
    friendly_name: &'a String,
    body: &'a Body,
    #[serde(skip_serializing_if = "Option::is_none")]
    emergency: &'a Option<DurableEmergencyConfiguration>,
    membership: &'a BodyMembership,
    graduation: &'a Option<BodyGraduationEvidence>,
    records: &'a Vec<BodyBiographyRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compaction: &'a Option<BodyBiographyCompaction>,
    #[serde(skip_serializing_if = "Wakes::is_empty")]
    wakes: Wakes<'a>,
}
impl<'a> From<&'a BodyBiographyEvidence> for EvidenceRef<'a> {
    fn from(value: &'a BodyBiographyEvidence) -> Self {
        Self {
            schema: &value.schema,
            body_id: &value.body_id,
            friendly_name: &value.friendly_name,
            body: &value.body,
            emergency: &value.emergency,
            membership: &value.membership,
            graduation: &value.graduation,
            records: &value.records,
            compaction: &value.compaction,
            wakes: Wakes(&value.wakes),
        }
    }
}
#[derive(Serialize)]
pub struct WakeRef<'a> {
    wake_id: &'a WakeId,
    body_id: &'a BodyId,
    workset: &'a BodyWorkset,
    workload_revision: &'a u64,
    wake_sequence: &'a u64,
    lifecycle: &'a WakeLifecycle,
    plans: Plans<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rejections: &'a Vec<WakeRejectionEvidence>,
    sign_ids: &'a Vec<conduit_core::SignId>,
    events: &'a Vec<WakeLifecycleEvent>,
}
impl<'a> From<&'a Wake> for WakeRef<'a> {
    fn from(value: &'a Wake) -> Self {
        Self {
            wake_id: &value.wake_id,
            body_id: &value.body_id,
            workset: &value.workset,
            workload_revision: &value.workload_revision,
            wake_sequence: &value.wake_sequence,
            lifecycle: &value.lifecycle,
            plans: Plans(&value.plans),
            rejections: &value.rejections,
            sign_ids: &value.sign_ids,
            events: &value.events,
        }
    }
}
#[derive(Serialize)]
pub struct PlanRef<'a> {
    plan_id: &'a conduit_core::PlanId,
    active_play_id: &'a Option<conduit_core::ActivePlayId>,
    state: &'a WakePlanState,
    hold: Option<()>,
}
impl<'a> From<&'a WakePlan> for PlanRef<'a> {
    fn from(value: &'a WakePlan) -> Self {
        Self {
            plan_id: &value.plan_id,
            active_play_id: &value.active_play_id,
            state: &value.state,
            hold: None,
        }
    }
}
#[derive(Serialize)]
pub struct ArchiveRef<'a> {
    schema: &'a String,
    body_id: &'a BodyId,
    ordinal: &'a u64,
    previous_digest: &'a Option<[u8; 32]>,
    records: &'a Vec<BodyBiographyRecord>,
    wakes: Wakes<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    body_events: &'a Vec<BodyLifecycleEvent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    membership_events: &'a Vec<MembershipEvent>,
    digest: &'a [u8; 32],
}
impl<'a> From<&'a BodyBiographyArchiveSegment> for ArchiveRef<'a> {
    fn from(value: &'a BodyBiographyArchiveSegment) -> Self {
        Self {
            schema: &value.schema,
            body_id: &value.body_id,
            ordinal: &value.ordinal,
            previous_digest: &value.previous_digest,
            records: &value.records,
            wakes: Wakes(&value.wakes),
            body_events: &value.body_events,
            membership_events: &value.membership_events,
            digest: &value.digest,
        }
    }
}
struct Wakes<'a>(&'a [Wake]);
impl Serialize for Wakes<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for item in self.0 {
            seq.serialize_element(&WakeRef::from(item))?;
        }
        seq.end()
    }
}
struct Plans<'a>(&'a [WakePlan]);
impl Serialize for Plans<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for item in self.0 {
            if item.hold.is_some() {
                return Err(serde::ser::Error::custom("restart-hold-unsupported"));
            }
            seq.serialize_element(&PlanRef::from(item))?;
        }
        seq.end()
    }
}
impl Wakes<'_> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
