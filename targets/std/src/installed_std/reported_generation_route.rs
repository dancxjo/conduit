//! Exact domain Info → complete typed quantity Face over the original route shell.
//! Domain interpretation belongs to the selected DHT recording codec, not a raw
//! JSON fallback or an enlargement of the independent generic Data16 profile.
use super::prepared_face_route::{PreparedFaceProjector, PreparedFaceRoute};
use conduit_core::{validate_canonical_structured_value, Quantity};
use conduit_dht11_observation::{
    recording::RecordingDisposition as D,
    reported_generation::PreparedReportedRecordingGenerationInput, ObservationContract,
};
use conduit_face::{
    inspection_fragment_backing_bytes, project_reported_quantity_channels,
    reported_quantity_fragment_backing_bound, reported_quantity_projection_workspace_bytes,
    FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue as V,
    ReportedQuantityChannel, ReportedQuantityObservation, ReportedQuantityOutcome,
    ReportedQuantityRecovery, MAX_FACE_ID_BYTES, MAX_REPORTED_QUANTITY_OBSERVATIONS,
};
use core::mem::size_of;

pub(super) struct ReportedGenerationProjector {
    input: PreparedReportedRecordingGenerationInput,
    contract: ObservationContract,
}
pub(super) type ReportedGenerationRoute = PreparedFaceRoute<ReportedGenerationProjector>;
struct OwnedObservation {
    sequence: u64,
    boot_ms: u64,
    gap_before: Option<(u64, u64)>,
    quantities: Option<[Quantity; 2]>,
    unavailable: Option<&'static str>,
}
const CHANNELS: usize = 2;
const RECOVERY_PROPERTIES: usize = 2 * CHANNELS;
fn extra_fragment_bytes() -> usize {
    RECOVERY_PROPERTIES * (size_of::<FaceProperty>() + 128 + 32)
}
pub(super) fn fragment_backing_bound() -> usize {
    // Same two-channel aggregate construction plus two reported recovery range
    // facts per source. The generic Data16 escaped-fragment bound is unchanged.
    CHANNELS
        * reported_quantity_fragment_backing_bound(MAX_REPORTED_QUANTITY_OBSERVATIONS, true)
            .expect("fixed reported64 profile")
        + CHANNELS * (size_of::<FaceProperty>() + 128 + "inspection-label".len() + 128)
        + extra_fragment_bytes()
}
impl ReportedGenerationProjector {
    pub(super) fn prepare() -> Result<Self, String> {
        Ok(Self {
            input: PreparedReportedRecordingGenerationInput::prepare().map_err(debug)?,
            contract: ObservationContract::prepare().map_err(debug)?,
        })
    }
    /// Retained plus transient heap payload; route charges inline Self, input
    /// custody and its two escaped contribution slots independently.
    pub(super) fn workspace_bytes() -> Result<usize, String> {
        let prepared = Self::prepare()?;
        prepared
            .retained_backing_bytes()
            .and_then(|retained| retained.checked_add(prepared.projection_workspace_bytes()?))
            .ok_or_else(|| "reported generation projection workspace overflow".into())
    }
}
impl PreparedFaceProjector for ReportedGenerationProjector {
    const NAME: &'static str = "Reported recording generation";
    fn project(
        &self,
        basis: FaceContributionBasis,
        bytes: &[u8],
        label: &str,
    ) -> Result<FaceFragment, String> {
        if basis.required_interaction_context.is_some()
            || [
                basis.checked_plot_id.as_str(),
                basis.plan_id.as_str(),
                basis.active_play_id.as_str(),
            ]
            .iter()
            .any(|id| id.is_empty() || id.len() > MAX_FACE_ID_BYTES)
            || !conduit_core::valid_data_label(label)
        {
            return Err("reported generation requires exact inspecting basis and label".into());
        }
        let mut generation = self.input.decode(bytes, &self.contract).map_err(debug)?;
        let recovery = generation.recovery();
        let mut owned = Vec::with_capacity(generation.len());
        for index in 0..generation.len() {
            let sample = generation
                .sample(index)
                .ok_or("reported generation sample absent")?;
            let (quantities, unavailable) = match (sample.disposition, sample.observation) {
                (D::Available, Some(bytes)) => {
                    let value = validate_canonical_structured_value(bytes).map_err(debug)?;
                    let quantity = |name| {
                        let value = value
                            .record_field(name)
                            .map_err(debug)?
                            .ok_or("reported observation field absent")?;
                        Quantity::decode(
                            value
                                .value_node()
                                .get(5..)
                                .ok_or("reported Quantity atom absent")?,
                        )
                        .map_err(debug)
                    };
                    (
                        Some([quantity("temperature")?, quantity("humidity")?]),
                        None,
                    )
                }
                (D::Available, None) | (_, Some(_)) => {
                    return Err("reported observation disposition mismatch".into())
                }
                (failure, None) => (
                    None,
                    Some(reason(failure).ok_or("reported unavailable reason absent")?),
                ),
            };
            owned.push(OwnedObservation {
                sequence: sample.sequence,
                boot_ms: sample.sampled_at_boot_ms,
                gap_before: sample.gap_before,
                quantities,
                unavailable,
            });
        }
        // Both immutable quantity owners and borrowed row tables coexist during
        // allocating projection. Their exact finite backing is charged below.
        let rows: [Vec<ReportedQuantityObservation<'_>>; CHANNELS] =
            core::array::from_fn(|channel| {
                owned
                    .iter()
                    .map(|sample| ReportedQuantityObservation {
                        sequence: sample.sequence,
                        sampled_at_boot_ms: sample.boot_ms,
                        gap_before: sample.gap_before,
                        outcome: match &sample.quantities {
                            Some(quantities) => {
                                ReportedQuantityOutcome::Available(&quantities[channel])
                            }
                            None => ReportedQuantityOutcome::Unavailable(
                                sample.unavailable.expect("checked failure reason"),
                            ),
                        },
                    })
                    .collect()
            });
        let channel = |index| {
            let names = generation.channel(index).ok_or("reported channel absent")?;
            Ok::<_, String>(ReportedQuantityChannel {
                source_identity: names.source_identity,
                source_name: names.source_name,
                channel_identity: names.channel_identity,
                channel_name: names.channel_name,
                producer: generation.producer(),
                samples_outside_view: u64::from(recovery.samples_outside_view),
                recovery: Some(ReportedQuantityRecovery {
                    chunks: recovery.chunks,
                    samples: recovery.samples,
                    preceding_history_known: recovery.preceding_history_known,
                }),
            })
        };
        let channels = [channel(0)?, channel(1)?];
        let mut fragment = project_reported_quantity_channels(
            basis,
            &[(channels[0], &rows[0]), (channels[1], &rows[1])],
            label,
        )
        .map_err(debug)?;
        fragment.properties.reserve_exact(RECOVERY_PROPERTIES);
        for channel in channels {
            for (name, value) in [
                ("recovered-first-sequence", recovery.first_sequence),
                ("recovered-last-sequence", recovery.last_sequence),
            ] {
                fragment.properties.push(FaceProperty {
                    subject: channel.source_identity.into(),
                    name: name.into(),
                    value: V::Count(value),
                });
            }
        }
        fragment.validate_bounds().map_err(debug)?;
        if inspection_fragment_backing_bytes(&fragment)
            .ok_or("reported fragment backing unsupported")?
            > fragment_backing_bound()
        {
            return Err("reported fragment exceeds its admitted64 profile".into());
        }
        Ok(fragment)
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        self.input
            .retained_backing_bytes()?
            .checked_add(self.contract.retained_backing_bytes()?)
    }
    fn projection_workspace_bytes(&self) -> Option<usize> {
        PreparedReportedRecordingGenerationInput::decoding_backing_bound(&self.contract)?
            .checked_add(MAX_REPORTED_QUANTITY_OBSERVATIONS.checked_mul(
                size_of::<OwnedObservation>()
                    + CHANNELS * size_of::<ReportedQuantityObservation<'_>>(),
            )?)?
            .checked_add(reported_quantity_projection_workspace_bytes(
                MAX_REPORTED_QUANTITY_OBSERVATIONS,
                CHANNELS,
                true,
            )?)?
            // Recovery append may coexist with the old/new aggregate property
            // tables; generic three-envelope allowance covers existing fields.
            .checked_add(3 * extra_fragment_bytes())
    }
    fn fragment_backing_bound(&self) -> usize {
        fragment_backing_bound()
    }
    fn fragment_backing_bytes(&self, fragment: &FaceFragment) -> Option<usize> {
        inspection_fragment_backing_bytes(fragment)
    }
}
fn reason(disposition: D) -> Option<&'static str> {
    Some(match disposition {
        D::Available => return None,
        D::CaptureTimeout => "capture-timeout",
        D::ProviderFailed => "provider-failed",
        D::GeometryRefused => "geometry-refused",
        D::ShortFrame => "short-frame",
        D::MalformedLength => "malformed-length",
        D::Checksum => "checksum",
        D::InvalidPayload => "invalid-payload",
    })
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("reported recording generation Face: {error:?}")
}
#[cfg(test)]
mod tests;
