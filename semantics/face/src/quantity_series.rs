//! Prepared interpretation of a bounded quantity series in an immutable Face.
//! These facts grant no observation, storage, membership or disclosure authority.
use crate::{Face, FacePropertyValue as V, FaceRelationshipKind, FaceRole};
use alloc::vec::Vec;
use conduit_core::{quantity_info_dimension, Quantity, QUANTITY_INFO_ID};
pub const QUANTITY_SERIES_ROLE: &str = "data/quantity-observation-series@1";
pub const MAX_QUANTITY_SERIES_SAMPLES: usize = 64;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantitySeriesRefusal {
    InvalidFace,
    Unsupported,
    Missing,
    Duplicate,
    Capacity,
    Provenance,
    Clock,
    Order,
    Gap,
    Availability,
    Quantity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantitySeriesSample<'a> {
    pub subject: &'a str,
    pub sequence: u64,
    pub boot_ms: u64,
    pub value: Option<Quantity>,
    pub gap_before: Option<(u64, u64)>,
    pub unavailable_reason: Option<&'a str>,
}
/// Hosted preparation owns finite interpretation storage, not a new history store.
/// The source Face remains the complete reading and provenance fallback.
pub struct PreparedQuantitySeries<'a> {
    subject: &'a str,
    source: &'a str,
    producer: [&'a str; 4],
    omitted_samples: u64,
    samples: Vec<QuantitySeriesSample<'a>>,
    publication_evidence: Option<&'a str>,
    publication_reference: Option<&'a [u8]>,
}
impl<'a> PreparedQuantitySeries<'a> {
    pub fn subject(&self) -> &str {
        self.subject
    }
    pub fn source(&self) -> &str {
        self.source
    }
    pub fn producer(&self) -> [&str; 4] {
        self.producer
    }
    pub fn omitted_samples(&self) -> u64 {
        self.omitted_samples
    }
    pub fn samples(&self) -> &[QuantitySeriesSample<'a>] {
        &self.samples
    }
    pub fn publication_evidence(&self) -> Option<&str> {
        self.publication_evidence
    }
    pub fn publication_reference(&self) -> Option<&[u8]> {
        self.publication_reference
    }
    /// Actual producer time, never sample-index spacing or a cross-Boot axis.
    pub fn time_millionths(&self, index: usize) -> Option<u32> {
        let time = self.samples.get(index)?.boot_ms;
        let first = self.samples.first()?.boot_ms;
        let span = self.samples.last()?.boot_ms - first;
        Some(if span == 0 {
            0
        } else {
            (u128::from(time - first) * 1_000_000 / u128::from(span)) as u32
        })
    }
    pub fn prepare(
        face: &'a Face,
        subject: &str,
        maximum_samples: usize,
    ) -> Result<Self, QuantitySeriesRefusal> {
        use QuantitySeriesRefusal as R;
        face.validate().map_err(|_| R::InvalidFace)?;
        if maximum_samples == 0 || maximum_samples > MAX_QUANTITY_SERIES_SAMPLES {
            return Err(R::Capacity);
        }
        let channel = face
            .subjects
            .iter()
            .find(|item| item.identity == subject)
            .ok_or(R::Missing)?;
        if !matches!(&channel.role, FaceRole::Semantic(role) if role.as_str() == QUANTITY_SERIES_ROLE)
        {
            return Err(R::Unsupported);
        }
        let subject = channel.identity.as_str();
        let source = identity(face, subject, "series-source")?;
        if !face
            .subjects
            .iter()
            .any(|item| item.identity == source && item.role == FaceRole::Collection)
        {
            return Err(R::Provenance);
        }
        let field = text(face, subject, "series-value-property")?.ok_or(R::Missing)?;
        if field.is_empty() {
            return Err(R::Missing);
        }
        if text(face, source, "clock")? != Some("producer-boot-ms") {
            return Err(R::Clock);
        }
        let producer = [
            identity(face, source, "producer-host")?,
            identity(face, source, "producer-boot")?,
            identity(face, source, "producer-plan")?,
            identity(face, source, "producer-play")?,
        ];
        let mut samples = Vec::with_capacity(maximum_samples);
        for relation in face.relationships.iter().filter(|relation| {
            relation.source == subject && relation.kind == FaceRelationshipKind::Contains
        }) {
            if samples.len() == maximum_samples {
                return Err(R::Capacity);
            }
            let item = relation.target.as_str();
            if !face
                .subjects
                .iter()
                .any(|entry| entry.identity == item && entry.role == FaceRole::Item)
                || identity(face, item, "series-source")? != source
                || !face.relationships.iter().any(|entry| {
                    entry.source == source
                        && entry.target == item
                        && entry.kind == FaceRelationshipKind::Contains
                })
            {
                return Err(R::Provenance);
            }
            let sequence = count(face, item, "sequence")?;
            let boot_ms = count(face, item, "sampled-at-boot-ms")?;
            let available = match property(face, item, "series-available")? {
                Some(V::Flag(value)) => *value,
                _ => return Err(R::Availability),
            };
            let value = match property(face, item, field)? {
                Some(V::TypedValue { contract, bytes }) if available => {
                    contract.validate(bytes).map_err(|_| R::Quantity)?;
                    let value = Quantity::decode(bytes).map_err(|_| R::Quantity)?;
                    let kind = contract.value_kind.as_str();
                    if kind != QUANTITY_INFO_ID
                        && quantity_info_dimension(kind) != Some(value.dimension())
                    {
                        return Err(R::Quantity);
                    }
                    Some(value)
                }
                None if !available => None,
                _ => return Err(R::Availability),
            };
            let gap_before = match (
                property(face, item, "missing-sequence-first")?,
                property(face, item, "missing-sequence-last")?,
            ) {
                (None, None) => None,
                (Some(V::Count(first)), Some(V::Count(last)))
                    if *first > 0 && first <= last && last.checked_add(1) == Some(sequence) =>
                {
                    Some((*first, *last))
                }
                _ => return Err(R::Gap),
            };
            let unavailable_reason = text(face, item, "series-unavailability")?;
            if unavailable_reason.is_some_and(|reason| available || reason.is_empty()) {
                return Err(R::Availability);
            }
            samples.push(QuantitySeriesSample {
                subject: item,
                sequence,
                boot_ms,
                value,
                gap_before,
                unavailable_reason,
            });
        }
        samples.sort_unstable_by_key(|sample| sample.sequence);
        let mut prior = None;
        let mut quantity = None;
        for sample in &samples {
            if sample.sequence == 0 {
                return Err(R::Order);
            }
            if let Some((sequence, time)) = prior {
                if sample.sequence <= sequence || sample.boot_ms < time {
                    return Err(R::Order);
                }
                let expected = if sample.sequence - sequence > 1 {
                    Some((sequence + 1, sample.sequence - 1))
                } else {
                    None
                };
                if sample.gap_before != expected {
                    return Err(R::Gap);
                }
            }
            if let Some(value) = sample.value {
                if quantity.is_some_and(|old: Quantity| {
                    old.family() != value.family() || old.role() != value.role()
                }) {
                    return Err(R::Quantity);
                }
                quantity = Some(value);
            }
            prior = Some((sample.sequence, sample.boot_ms));
        }
        let source_items = face
            .relationships
            .iter()
            .filter(|relation| {
                relation.source == source && relation.kind == FaceRelationshipKind::Contains
            })
            .count() as u64;
        if count(face, source, "visible-samples")? != source_items {
            return Err(R::Provenance);
        }
        let omitted_samples = count(face, source, "samples-outside-view")?
            .checked_add(
                source_items
                    .checked_sub(samples.len() as u64)
                    .ok_or(R::Provenance)?,
            )
            .ok_or(R::Capacity)?;
        let publication_evidence = text(face, source, "publication-evidence")?;
        let publication_reference = match property(face, source, "last-publication-reference")? {
            Some(V::Content(bytes)) => {
                conduit_core::BoundedResourceRef::validate_encoded(bytes)
                    .map_err(|_| R::Provenance)?;
                Some(bytes.as_slice())
            }
            None => None,
            _ => return Err(R::Provenance),
        };
        if publication_evidence == Some("kernel-publication-commit")
            && publication_reference.is_none()
        {
            return Err(R::Provenance);
        }
        Ok(Self {
            subject,
            source,
            producer,
            omitted_samples,
            samples,
            publication_evidence,
            publication_reference,
        })
    }
}
fn property<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<Option<&'a V>, QuantitySeriesRefusal> {
    let mut found = face
        .properties
        .iter()
        .filter(|property| property.subject == subject && property.name == name);
    let value = found.next().map(|property| &property.value);
    if found.next().is_some() {
        return Err(QuantitySeriesRefusal::Duplicate);
    }
    Ok(value)
}
fn identity<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<&'a str, QuantitySeriesRefusal> {
    match property(face, subject, name)? {
        Some(V::Identity(value)) if !value.is_empty() => Ok(value),
        _ => Err(QuantitySeriesRefusal::Provenance),
    }
}
fn text<'a>(
    face: &'a Face,
    subject: &str,
    name: &str,
) -> Result<Option<&'a str>, QuantitySeriesRefusal> {
    match property(face, subject, name)? {
        Some(V::Text(value)) => Ok(Some(value)),
        None => Ok(None),
        _ => Err(QuantitySeriesRefusal::Missing),
    }
}
fn count(face: &Face, subject: &str, name: &str) -> Result<u64, QuantitySeriesRefusal> {
    match property(face, subject, name)? {
        Some(V::Count(value)) => Ok(*value),
        _ => Err(QuantitySeriesRefusal::Missing),
    }
}
