//! Shared bounded typed channel projection; all growth belongs to preparation.
use super::*;
use alloc::{string::String, vec::Vec};
use core::fmt::Write;

/// Project separately checked typed observations without synthesizing a larger
/// generic Data value. This still checks finite identity/order/gap/family laws.
pub fn project_reported_quantity_channel(
    basis: FaceContributionBasis,
    channel: ReportedQuantityChannel<'_>,
    samples: &[ReportedQuantityObservation<'_>],
    label: &str,
) -> Result<FaceFragment, QuantityObservationFaceRefusal> {
    project_iter(
        basis,
        channel,
        samples.iter().copied(),
        label,
        MAX_REPORTED_QUANTITY_OBSERVATIONS,
    )
}

pub(super) fn project_iter<'a>(
    basis: FaceContributionBasis,
    metadata: ReportedQuantityChannel<'a>,
    observations: impl ExactSizeIterator<Item = ReportedQuantityObservation<'a>>,
    label: &str,
    maximum: usize,
) -> Result<FaceFragment, QuantityObservationFaceRefusal> {
    use QuantityObservationFaceRefusal as R;
    let source = metadata.source_identity;
    let channel = metadata.channel_identity;
    let n = observations.len();
    validate_metadata(&basis, metadata, label, n, maximum)?;
    let recovery_properties = if metadata.recovery.is_some() { 3 } else { 0 };
    let mut fragment = FaceFragment {
        basis,
        subjects: Vec::with_capacity(n + 2),
        relationships: Vec::with_capacity(2 * n),
        properties: Vec::with_capacity(11 + 7 * n + recovery_properties),
        composition: Vec::new(),
        text: Vec::new(),
        actions: Vec::new(),
        disclosures: Vec::new(),
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
    fragment.subjects.push(FaceSubject {
        identity: source.into(),
        name: metadata.source_name.into(),
        role: FaceRole::Collection,
    });
    fragment.subjects.push(FaceSubject {
        identity: channel.into(),
        name: label.into(),
        role: FaceRole::Semantic(QUANTITY_SERIES_ROLE.into()),
    });
    let mut fact = |subject: &str, name: &str, value| {
        fragment.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        })
    };
    use FacePropertyValue as V;
    fact(source, "clock", V::Text("producer-boot-ms".into()));
    for (name, value) in [
        ("producer-host", metadata.producer[0]),
        ("producer-boot", metadata.producer[1]),
        ("producer-plan", metadata.producer[2]),
        ("producer-play", metadata.producer[3]),
    ] {
        fact(source, name, V::Identity(value.into()));
    }
    fact(source, "visible-samples", V::Count(n as u64));
    fact(
        source,
        "samples-outside-view",
        V::Count(metadata.samples_outside_view),
    );
    fact(
        source,
        "observation-origin",
        V::Text("reported-input".into()),
    );
    fact(channel, "series-source", V::Identity(source.into()));
    fact(channel, "series-value-property", V::Text("value".into()));
    fact(
        channel,
        "reported-channel-name",
        V::Text(metadata.channel_name.into()),
    );
    if let Some(recovery) = metadata.recovery {
        fact(
            source,
            "recovered-chunks",
            V::Count(u64::from(recovery.chunks)),
        );
        fact(
            source,
            "recovered-samples",
            V::Count(u64::from(recovery.samples)),
        );
        fact(
            source,
            "preceding-history-known",
            V::Flag(recovery.preceding_history_known),
        );
    }
    let mut previous = None;
    let mut quantity = None;
    for observation in observations {
        validate_sample(observation, &mut previous, &mut quantity)?;
        let mut identity = String::with_capacity(156);
        write!(&mut identity, "{source}/sample/{}", observation.sequence)
            .map_err(|_| R::Fragment)?;
        let mut name = String::with_capacity(32);
        write!(&mut name, "Observation {}", observation.sequence).map_err(|_| R::Fragment)?;
        fragment.subjects.push(FaceSubject {
            identity: identity.clone(),
            name,
            role: FaceRole::Item,
        });
        for parent in [source, channel] {
            fragment.relationships.push(FaceRelationship {
                source: parent.into(),
                target: identity.clone(),
                kind: FaceRelationshipKind::Contains,
            });
        }
        fact(&identity, "series-source", V::Identity(source.into()));
        fact(&identity, "sequence", V::Count(observation.sequence));
        fact(
            &identity,
            "sampled-at-boot-ms",
            V::Count(observation.sampled_at_boot_ms),
        );
        match observation.outcome {
            ReportedQuantityOutcome::Available(value) => {
                fact(&identity, "series-available", V::Flag(true));
                fact(
                    &identity,
                    "value",
                    V::TypedValue {
                        contract: conduit_core::CheckedValueContract::new(
                            conduit_core::QUANTITY_INFO_ID.into(),
                            conduit_core::QUANTITY_ENCODED_LEN as u32,
                            Vec::new(),
                        )
                        .map_err(|_| R::Fragment)?,
                        bytes: value.encode().to_vec(),
                    },
                );
            }
            ReportedQuantityOutcome::Unavailable(reason) => {
                fact(&identity, "series-available", V::Flag(false));
                fact(&identity, "series-unavailability", V::Text(reason.into()));
            }
        }
        if let Some(gap) = &observation.gap_before {
            fact(&identity, "missing-sequence-first", V::Count(gap.0));
            fact(&identity, "missing-sequence-last", V::Count(gap.1));
        }
    }
    fragment.validate_bounds().map_err(|_| R::Fragment)?;
    if inspection_fragment_backing_bytes(&fragment).ok_or(R::Backing)?
        > reported_quantity_fragment_backing_bound(maximum, metadata.recovery.is_some())
            .ok_or(R::Backing)?
    {
        return Err(R::Backing);
    }
    Ok(fragment)
}

fn validate_metadata(
    basis: &FaceContributionBasis,
    channel: ReportedQuantityChannel<'_>,
    label: &str,
    count: usize,
    maximum: usize,
) -> Result<(), QuantityObservationFaceRefusal> {
    use QuantityObservationFaceRefusal as R;
    if basis.required_interaction_context.is_some()
        || [
            basis.checked_plot_id.as_str(),
            basis.plan_id.as_str(),
            basis.active_play_id.as_str(),
        ]
        .iter()
        .any(|id| id.is_empty() || id.len() > MAX_FACE_ID_BYTES)
    {
        return Err(R::Basis);
    }
    if !conduit_core::valid_data_label(label) {
        return Err(R::Label);
    }
    if maximum > MAX_REPORTED_QUANTITY_OBSERVATIONS
        || count > maximum
        || [
            channel.source_identity,
            channel.source_name,
            channel.channel_identity,
            channel.channel_name,
        ]
        .iter()
        .any(|text| text.is_empty() || text.len() > 128)
        || channel.source_identity == channel.channel_identity
        || channel
            .producer
            .iter()
            .any(|id| id.is_empty() || id.len() > 256)
    {
        return Err(R::Fragment);
    }
    if let Some(recovery) = channel.recovery {
        if u64::from(recovery.samples).checked_sub(count as u64)
            != Some(channel.samples_outside_view)
            || (recovery.chunks == 0) != (recovery.samples == 0)
            || u32::from(recovery.chunks) > recovery.samples
        {
            return Err(R::Fragment);
        }
    }
    Ok(())
}
fn validate_sample(
    sample: ReportedQuantityObservation<'_>,
    previous: &mut Option<(u64, u64)>,
    quantity: &mut Option<conduit_core::Quantity>,
) -> Result<(), QuantityObservationFaceRefusal> {
    use QuantityObservationFaceRefusal as R;
    if sample.sequence == 0 {
        return Err(R::Fragment);
    }
    if sample.gap_before.is_some_and(|(first, last)| {
        first == 0 || first > last || last.checked_add(1) != Some(sample.sequence)
    }) {
        return Err(R::Fragment);
    }
    if let Some((sequence, time)) = *previous {
        if sample.sequence <= sequence || sample.sampled_at_boot_ms < time {
            return Err(R::Fragment);
        }
        let expected =
            (sample.sequence - sequence > 1).then_some((sequence + 1, sample.sequence - 1));
        if sample.gap_before != expected {
            return Err(R::Fragment);
        }
    }
    match sample.outcome {
        ReportedQuantityOutcome::Available(value) => {
            if quantity.is_some_and(|prior| {
                prior.family() != value.family() || prior.role() != value.role()
            }) {
                return Err(R::Fragment);
            }
            *quantity = Some(*value);
        }
        ReportedQuantityOutcome::Unavailable(reason) if reason.is_empty() || reason.len() > 128 => {
            return Err(R::Fragment)
        }
        ReportedQuantityOutcome::Unavailable(_) => {}
    }
    *previous = Some((sample.sequence, sample.sampled_at_boot_ms));
    Ok(())
}

/// Two independently reported channels in one ordinary contribution. Full typed
/// readings remain present; optional Mask plotting retains its separate budget.
pub fn project_reported_quantity_channels(
    basis: FaceContributionBasis,
    channels: &[(
        ReportedQuantityChannel<'_>,
        &[ReportedQuantityObservation<'_>],
    )],
    label: &str,
) -> Result<FaceFragment, QuantityObservationFaceRefusal> {
    use QuantityObservationFaceRefusal as R;
    if !(1..=2).contains(&channels.len()) {
        return Err(R::Fragment);
    }
    if channels
        .iter()
        .any(|(_, samples)| samples.len() > MAX_REPORTED_QUANTITY_OBSERVATIONS)
    {
        return Err(R::Fragment);
    }
    let samples = channels.iter().map(|(_, samples)| samples.len()).sum();
    let recovery = channels
        .iter()
        .filter(|(channel, _)| channel.recovery.is_some())
        .count();
    let mut bound = 0usize;
    for (channel, _) in channels {
        bound = bound
            .checked_add(
                reported_quantity_fragment_backing_bound(
                    MAX_REPORTED_QUANTITY_OBSERVATIONS,
                    channel.recovery.is_some(),
                )
                .ok_or(R::Backing)?,
            )
            .and_then(|n| {
                n.checked_add(
                    core::mem::size_of::<FaceProperty>() + 128 + "inspection-label".len() + 128,
                )
            })
            .ok_or(R::Backing)?;
    }
    aggregate_fragments(
        basis.clone(),
        channels.iter().map(|(channel, samples)| {
            Ok((
                project_reported_quantity_channel(basis.clone(), *channel, samples, label)?,
                channel.source_identity,
                channel.channel_identity,
                channel.channel_name,
            ))
        }),
        samples,
        recovery,
        label,
        bound,
    )
}

pub(crate) fn aggregate_fragments<'a>(
    basis: FaceContributionBasis,
    fragments: impl ExactSizeIterator<
        Item = Result<(FaceFragment, &'a str, &'a str, &'a str), QuantityObservationFaceRefusal>,
    >,
    samples: usize,
    recovery: usize,
    label: &str,
    bound: usize,
) -> Result<FaceFragment, QuantityObservationFaceRefusal> {
    use QuantityObservationFaceRefusal as R;
    let count = fragments.len();
    let mut aggregate = FaceFragment {
        basis,
        subjects: Vec::with_capacity(samples + 2 * count),
        relationships: Vec::with_capacity(2 * samples),
        properties: Vec::with_capacity(12 * count + 7 * samples + 3 * recovery),
        composition: Vec::new(),
        text: Vec::new(),
        actions: Vec::new(),
        disclosures: Vec::new(),
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
    for fragment in fragments {
        let (mut fragment, source, channel, name) = fragment?;
        fragment
            .subjects
            .iter_mut()
            .find(|s| s.identity == channel)
            .ok_or(R::Fragment)?
            .name = name.into();
        aggregate.properties.push(FaceProperty {
            subject: source.into(),
            name: "inspection-label".into(),
            value: FacePropertyValue::Text(label.into()),
        });
        aggregate.subjects.extend(fragment.subjects);
        aggregate.relationships.extend(fragment.relationships);
        aggregate.properties.extend(fragment.properties);
    }
    aggregate.validate_bounds().map_err(|_| R::Fragment)?;
    if inspection_fragment_backing_bytes(&aggregate).ok_or(R::Backing)? > bound {
        return Err(R::Backing);
    }
    Ok(aggregate)
}
