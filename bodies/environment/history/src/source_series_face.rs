//! Shared Face projection and reader for the collector-observation profile.
use crate::source_series::*;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::CheckedValueContract;
use conduit_environment_measurement::source_observation::{ObservationContract, MAXIMUM_BYTES};
use conduit_face::{
    Face, FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue as V,
    FaceRelationship, FaceRelationshipKind as Relation, FaceRole, FaceSubject,
};

fn property(properties: &mut Vec<FaceProperty>, subject: &str, name: &str, value: V) {
    properties.push(FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    });
}
fn value_contract(
    codec: &ObservationContract,
) -> Result<CheckedValueContract, SourceSeriesRefusal> {
    let profile = codec
        .value_type()
        .profile()
        .map_err(|_| SourceSeriesRefusal::Contract)?;
    CheckedValueContract::new(profile.value_kind().clone(), MAXIMUM_BYTES as u32, vec![])
        .map_err(|_| SourceSeriesRefusal::Contract)
}
impl SourceSeriesCollection<'_> {
    /// Retains original canonical observation bytes. The caller owns authority,
    /// sampling/history retention and separately admitted complete Face capacity.
    pub fn contribution(
        &self,
        basis: FaceContributionBasis,
    ) -> Result<FaceFragment, SourceSeriesRefusal> {
        self.validate()?;
        let codec = ObservationContract::prepare().map_err(|_| SourceSeriesRefusal::Contract)?;
        let contract = value_contract(&codec)?;
        let mut fragment = FaceFragment {
            basis,
            subjects: vec![FaceSubject {
                identity: self.subject.into(),
                role: FaceRole::Collection,
                name: "Environmental observations".into(),
            }],
            relationships: vec![],
            composition: vec![],
            properties: vec![],
            text: vec![],
            actions: vec![],
            disclosures: vec![],
            temporal_references: vec![],
            temporal_facts: vec![],
        };
        for (name, value) in [
            ("schema", V::Text(SOURCE_SERIES_SCHEMA.into())),
            ("clock", V::Text(SOURCE_SERIES_CLOCK.into())),
            ("collector-host", V::Identity(self.collector_host.into())),
            ("collector-boot", V::Identity(self.collector_boot.into())),
            (
                "observed-through-boot-ms",
                V::Count(self.observed_through_boot_ms),
            ),
            ("visible-series", V::Count(self.series.len() as u64)),
        ] {
            property(&mut fragment.properties, self.subject, name, value);
        }
        for series in self.series {
            fragment.subjects.push(FaceSubject {
                identity: series.subject.into(),
                role: FaceRole::Collection,
                name: series.name.into(),
            });
            fragment.relationships.push(FaceRelationship {
                source: self.subject.into(),
                target: series.subject.into(),
                kind: Relation::Contains,
            });
            for (name, value) in [
                ("source", V::Identity(series.source.into())),
                ("provenance", V::Text("collector-observed-device".into())),
                ("visible-samples", V::Count(series.records.len() as u64)),
                (
                    "samples-outside-view",
                    V::Count(series.samples_outside_view),
                ),
                ("stale-after-ms", V::Count(series.stale_after_ms)),
            ] {
                property(&mut fragment.properties, series.subject, name, value);
            }
            for record in series.records {
                let subject = format!("{}/sample/{}", series.subject, record.sequence);
                fragment.subjects.push(FaceSubject {
                    identity: subject.clone(),
                    role: FaceRole::Item,
                    name: format!("Observation {}", record.sequence),
                });
                fragment.relationships.push(FaceRelationship {
                    source: series.subject.into(),
                    target: subject.clone(),
                    kind: Relation::Contains,
                });
                property(
                    &mut fragment.properties,
                    &subject,
                    "sequence",
                    V::Count(record.sequence),
                );
                property(
                    &mut fragment.properties,
                    &subject,
                    "observation",
                    V::TypedValue {
                        contract: contract.clone(),
                        bytes: record.observation.to_vec(),
                    },
                );
            }
        }
        fragment
            .validate_bounds()
            .map_err(|_| SourceSeriesRefusal::Face)?;
        Ok(fragment)
    }
}
fn get<'a>(face: &'a Face, subject: &str, name: &str) -> Result<&'a V, SourceSeriesRefusal> {
    let mut matches = face
        .properties
        .iter()
        .filter(|p| p.subject == subject && p.name == name);
    let value = &matches.next().ok_or(SourceSeriesRefusal::Face)?.value;
    if matches.next().is_some() {
        return Err(SourceSeriesRefusal::Face);
    }
    Ok(value)
}
fn count(face: &Face, subject: &str, name: &str) -> Result<u64, SourceSeriesRefusal> {
    match get(face, subject, name)? {
        V::Count(n) => Ok(*n),
        _ => Err(SourceSeriesRefusal::Face),
    }
}
fn ident<'a>(face: &'a Face, subject: &str, name: &str) -> Result<&'a str, SourceSeriesRefusal> {
    match get(face, subject, name)? {
        V::Identity(s) if identity(s) => Ok(s),
        _ => Err(SourceSeriesRefusal::Identity),
    }
}
fn contains<'a>(
    face: &'a Face,
    subject: &str,
    max: usize,
) -> Result<Vec<&'a FaceSubject>, SourceSeriesRefusal> {
    let mut result = Vec::with_capacity(max);
    for link in face
        .relationships
        .iter()
        .filter(|r| r.source == subject && r.kind == Relation::Contains)
    {
        if result.len() == max
            || result
                .iter()
                .any(|s: &&FaceSubject| s.identity == link.target)
        {
            return Err(SourceSeriesRefusal::Extent);
        }
        result.push(
            face.subjects
                .iter()
                .find(|s| s.identity == link.target)
                .ok_or(SourceSeriesRefusal::Face)?,
        );
    }
    Ok(result)
}
impl SourceSeriesGraph {
    pub fn prepare(face: &Face, subject: &str) -> Result<Self, SourceSeriesRefusal> {
        use SourceSeriesRefusal as R;
        face.validate().map_err(|_| R::Face)?;
        if !face
            .subjects
            .iter()
            .any(|s| s.identity == subject && s.role == FaceRole::Collection)
            || get(face, subject, "schema")? != &V::Text(SOURCE_SERIES_SCHEMA.into())
            || get(face, subject, "clock")? != &V::Text(SOURCE_SERIES_CLOCK.into())
        {
            return Err(R::Clock);
        }
        let collector_host = ident(face, subject, "collector-host")?;
        let collector_boot = ident(face, subject, "collector-boot")?;
        let cutoff = count(face, subject, "observed-through-boot-ms")?;
        let children = contains(face, subject, MAX_SOURCE_SERIES)?;
        if count(face, subject, "visible-series")? != children.len() as u64 {
            return Err(R::Extent);
        }
        let codec = ObservationContract::prepare().map_err(|_| R::Contract)?;
        let contract = value_contract(&codec)?;
        let mut series: Vec<PreparedSourceSeries> = Vec::with_capacity(MAX_SOURCE_SERIES);
        for child in children {
            if child.role != FaceRole::Collection
                || !name(&child.name)
                || get(face, &child.identity, "provenance")?
                    != &V::Text("collector-observed-device".into())
            {
                return Err(R::Identity);
            }
            let source = ident(face, &child.identity, "source")?;
            if series.iter().any(|s| s.source == source) {
                return Err(R::Identity);
            }
            let samples = contains(face, &child.identity, MAX_SOURCE_SERIES_SAMPLES)?;
            if count(face, &child.identity, "visible-samples")? != samples.len() as u64 {
                return Err(R::Extent);
            }
            let stale_after_ms = count(face, &child.identity, "stale-after-ms")?;
            if stale_after_ms == 0 {
                return Err(R::Extent);
            }
            let mut points = Vec::with_capacity(MAX_SOURCE_SERIES_SAMPLES);
            for sample in samples {
                if sample.role != FaceRole::Item {
                    return Err(R::Face);
                }
                let observation = match get(face, &sample.identity, "observation")? {
                    V::TypedValue {
                        contract: actual,
                        bytes,
                    } if actual == &contract => codec.decode(bytes).map_err(|_| R::Observation)?,
                    _ => return Err(R::Observation),
                };
                if observation.observed_at_host_boot_ms > cutoff {
                    return Err(R::Clock);
                }
                let sequence = count(face, &sample.identity, "sequence")?;
                if sequence == 0 {
                    return Err(R::Sequence);
                }
                points.push(SourceSeriesPoint {
                    sequence,
                    observation,
                });
            }
            points.sort_unstable_by_key(|p| p.sequence);
            if points.windows(2).any(|p| {
                p[0].sequence == p[1].sequence
                    || p[0].observation.observed_at_host_boot_ms
                        > p[1].observation.observed_at_host_boot_ms
            }) {
                return Err(R::Sequence);
            }
            series.push(PreparedSourceSeries {
                subject: child.identity.clone(),
                source: source.into(),
                name: child.name.clone(),
                samples_outside_view: count(face, &child.identity, "samples-outside-view")?,
                stale_after_ms,
                points,
            });
        }
        Ok(Self {
            subject: String::from(subject),
            collector_host: collector_host.into(),
            collector_boot: collector_boot.into(),
            observed_through_boot_ms: cutoff,
            series,
        })
    }
}
