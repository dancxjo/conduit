//! Bounded immutable Mask projection. No measurement storage or clock is owned here.
use alloc::vec::Vec;
use conduit_core::{Quantity, QuantityRole, RATIO_INFO_ID, TEMPERATURE_INFO_ID, Unit, kind_id};
use conduit_face::{Face, FacePropertyValue as V, FaceRelationshipKind, FaceRole};

pub const MAX_GRAPH_SAMPLES: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphRefusal {
    InvalidFace,
    Missing,
    Duplicate,
    InvalidProperty,
    InvalidOrder,
    TooLarge,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphSample {
    pub(crate) sequence: u64,
    pub(crate) boot_ms: u64,
    pub(crate) temperature_milli_c: Option<i64>,
    pub(crate) humidity_milli_percent: Option<i64>,
    pub(crate) gap_before: Option<(u64, u64)>,
    pub(crate) failure: bool,
    pub(crate) failure_reason: Option<&'static str>,
}
/// Coordinates are shared by both realizations; producer time is never wall time.
pub struct HistoryGraph<'a> {
    pub(crate) content_id: &'a str,
    pub(crate) revision: u64,
    pub(crate) subject: &'a str,
    pub(crate) producer_boot: &'a str,
    pub(crate) producer_context: [&'a str; 4],
    pub(crate) omitted: u64,
    pub(crate) samples: Vec<GraphSample>,
}
impl GraphSample {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn boot_ms(&self) -> u64 {
        self.boot_ms
    }
    pub fn temperature_milli_c(&self) -> Option<i64> {
        self.temperature_milli_c
    }
    pub fn humidity_milli_percent(&self) -> Option<i64> {
        self.humidity_milli_percent
    }
    pub fn gap_before(&self) -> Option<(u64, u64)> {
        self.gap_before
    }
    pub fn failure_reason(&self) -> Option<&'static str> {
        self.failure_reason
    }
    pub fn failure(&self) -> bool {
        self.failure
    }
}
impl<'a> HistoryGraph<'a> {
    pub fn content_id(&self) -> &str {
        self.content_id
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn subject(&self) -> &str {
        self.subject
    }
    pub fn producer_boot(&self) -> &str {
        self.producer_boot
    }
    pub fn producer_context(&self) -> [&str; 4] {
        self.producer_context
    }
    pub fn omitted(&self) -> u64 {
        self.omitted
    }
    pub fn samples(&self) -> &[GraphSample] {
        &self.samples
    }

    pub fn prepare(face: &'a Face, subject: &'a str) -> Result<Self, GraphRefusal> {
        use GraphRefusal as R;
        face.validate().map_err(|_| R::InvalidFace)?;
        if !face
            .subjects
            .iter()
            .any(|s| s.identity == subject && s.role == FaceRole::Collection)
        {
            return Err(R::Missing);
        }
        let p = |s: &str, n: &str| property(face, s, n);
        if p(subject, "schema")? != Some(&V::Text("environment/history@1".into()))
            || p(subject, "clock")? != Some(&V::Text("producer-boot-ms".into()))
        {
            return Err(R::InvalidProperty);
        }
        let mut producer_context = [""; 4];
        for (slot, name) in producer_context.iter_mut().zip([
            "producer-host",
            "producer-boot",
            "producer-plan",
            "producer-play",
        ]) {
            *slot = match p(subject, name)? {
                Some(V::Identity(value)) if !value.is_empty() => value.as_str(),
                _ => return Err(R::Missing),
            };
        }
        let producer_boot = producer_context[1];
        let outside = count(face, subject, "samples-outside-view")?;
        let visible = count(face, subject, "visible-samples")?;
        if count(face, subject, "recovered-samples")?
            != outside.checked_add(visible).ok_or(R::TooLarge)?
        {
            return Err(R::InvalidProperty);
        }
        let chunks = count(face, subject, "recovered-chunks")?;
        let recovered = count(face, subject, "recovered-samples")?;
        if chunks > u64::from(u16::MAX)
            || (chunks == 0 && (recovered != 0 || visible != 0))
            || (chunks != 0
                && (visible == 0
                    || recovered < chunks
                    || recovered > chunks * 64
                    || outside < chunks - 1
                    || visible > 64
                    || outside > recovered - 1))
        {
            return Err(R::InvalidProperty);
        }
        if p(subject, "preceding-history-known")? != Some(&V::Flag(false)) {
            return Err(R::InvalidProperty);
        }
        let mut children: Vec<_> = face
            .relationships
            .iter()
            .filter(|r| r.source == subject && r.kind == FaceRelationshipKind::Contains)
            .collect();
        if children.len() > 64 {
            return Err(R::TooLarge);
        }
        // Contains is a semantic relation, not an ordering instruction. Decode
        // keys before sorting so malformed/duplicate properties cannot hide.
        let mut ordered = Vec::with_capacity(children.len());
        for child in children.drain(..) {
            ordered.push((count(face, &child.target, "sequence")?, child));
        }
        ordered.sort_unstable_by_key(|(sequence, _)| *sequence);
        children.extend(ordered.into_iter().map(|(_, child)| child));
        if visible != children.len() as u64 {
            return Err(R::InvalidProperty);
        }
        let skip = children.len().saturating_sub(MAX_GRAPH_SAMPLES);
        let mut samples = Vec::with_capacity(MAX_GRAPH_SAMPLES);
        let mut previous = None;
        let mut seen = Vec::with_capacity(64);
        for (i, child) in children.iter().enumerate() {
            if seen.contains(&child.target.as_str()) {
                return Err(R::Duplicate);
            }
            seen.push(child.target.as_str());
            let s = &child.target;
            if !face
                .subjects
                .iter()
                .any(|item| item.identity == *s && item.role == FaceRole::Item)
            {
                return Err(R::InvalidProperty);
            }
            let sequence = count(face, s, "sequence")?;
            if sequence == 0 {
                return Err(R::InvalidOrder);
            }
            let boot_ms = count(face, s, "sampled-at-boot-ms")?;
            let gap_before = match (
                p(s, "missing-sequence-first")?,
                p(s, "missing-sequence-last")?,
            ) {
                (None, None) => None,
                (Some(V::Count(first)), Some(V::Count(last)))
                    if *first > 0 && first <= last && *last == sequence - 1 =>
                {
                    Some((*first, *last))
                }
                _ => return Err(R::InvalidProperty),
            };
            if let Some((seq, time)) = previous {
                if sequence <= seq || boot_ms < time {
                    return Err(R::InvalidOrder);
                }
                if sequence != seq + 1 && gap_before != Some((seq + 1, sequence - 1)) {
                    return Err(R::InvalidOrder);
                }
                if sequence == seq + 1 && gap_before.is_some() {
                    return Err(R::InvalidOrder);
                }
            }
            if previous.is_none()
                && (sequence <= outside
                    || (outside == 0 && gap_before.is_some())
                    || gap_before.is_some_and(|(first, _)| first <= outside))
            {
                return Err(R::InvalidOrder);
            }
            previous = Some((sequence, boot_ms));
            let failure = match p(s, "sensor-status")? {
                Some(V::Text(v)) if v == "available" => false,
                Some(V::Text(v)) if v == "unavailable" => {
                    match p(s, "sensor-failure")? {
                        Some(V::Text(v))
                            if matches!(
                                v.as_str(),
                                "capture-timeout"
                                    | "provider-failed"
                                    | "geometry-refused"
                                    | "short-frame"
                                    | "malformed-length"
                                    | "checksum"
                                    | "invalid-payload"
                            ) => {}
                        _ => return Err(R::Missing),
                    };
                    true
                }
                _ => return Err(R::InvalidProperty),
            };
            let failure_reason = if failure {
                let Some(V::Text(label)) = p(s, "sensor-failure")? else {
                    return Err(R::Missing);
                };
                Some(match label.as_str() {
                    "capture-timeout" => "capture-timeout",
                    "provider-failed" => "provider-failed",
                    "geometry-refused" => "geometry-refused",
                    "short-frame" => "short-frame",
                    "malformed-length" => "malformed-length",
                    "checksum" => "checksum",
                    "invalid-payload" => "invalid-payload",
                    _ => return Err(R::InvalidProperty),
                })
            } else {
                None
            };
            if !failure && p(s, "sensor-failure")?.is_some() {
                return Err(R::InvalidProperty);
            }
            let temperature = quantity(face, s, "temperature", Unit::Celsius, QuantityRole::Point)?;
            let humidity = quantity(
                face,
                s,
                "relative-humidity",
                Unit::Percent,
                QuantityRole::Linear,
            )?;
            if (failure && (temperature.is_some() || humidity.is_some()))
                || humidity.is_some_and(|h| !(0..=100_000).contains(&h))
                || (!failure && (temperature.is_none() || humidity.is_none()))
            {
                return Err(R::InvalidProperty);
            }
            if i >= skip {
                samples.push(GraphSample {
                    sequence,
                    boot_ms,
                    temperature_milli_c: if failure { None } else { temperature },
                    humidity_milli_percent: if failure { None } else { humidity },
                    gap_before,
                    failure,
                    failure_reason,
                });
            }
        }
        Ok(Self {
            content_id: face.identity.as_str(),
            revision: face.revision,
            subject,
            producer_boot,
            producer_context,
            omitted: outside.checked_add(skip as u64).ok_or(R::TooLarge)?,
            samples,
        })
    }
    pub fn x(&self, sample: &GraphSample) -> u16 {
        ChartGeometry {
            samples: &self.samples,
        }
        .x(sample)
    }
    pub fn temperature_range(&self) -> (i64, i64) {
        ChartGeometry {
            samples: &self.samples,
        }
        .temperature_range()
    }
    pub fn y(&self, value: i64, humidity: bool) -> u16 {
        ChartGeometry {
            samples: &self.samples,
        }
        .y(value, humidity)
    }
}
pub(crate) struct ChartGeometry<'a> {
    pub(crate) samples: &'a [GraphSample],
}
impl ChartGeometry<'_> {
    pub fn x(&self, sample: &GraphSample) -> u16 {
        let first = self.samples.first().map_or(0, |s| s.boot_ms);
        let span = self.samples.last().map_or(0, |s| s.boot_ms - first);
        if span == 0 {
            32
        } else {
            32 + ((u128::from(sample.boot_ms.saturating_sub(first).min(span)) * 560)
                / u128::from(span)) as u16
        }
    }
    pub fn temperature_range(&self) -> (i64, i64) {
        let mut values = self
            .samples
            .iter()
            .filter_map(|sample| sample.temperature_milli_c);
        let Some(first) = values.next() else {
            return (0, 1000);
        };
        let (min, max) = values.fold((first, first), |(min, max), value| {
            (min.min(value), max.max(value))
        });
        if min == max {
            (min.saturating_sub(1000), max.saturating_add(1000))
        } else {
            (min, max)
        }
    }
    pub fn y(&self, value: i64, humidity: bool) -> u16 {
        let (min, max) = if humidity {
            (0, 100_000)
        } else {
            self.temperature_range()
        };
        let value = value.clamp(min, max);
        let offset = ((i128::from(value) - i128::from(min)) * 100
            / (i128::from(max) - i128::from(min))) as u16;
        (if humidity { 292 } else { 140 }) - offset
    }
}

fn property<'a>(face: &'a Face, subject: &str, name: &str) -> Result<Option<&'a V>, GraphRefusal> {
    let mut matches = face
        .properties
        .iter()
        .filter(|p| p.subject == subject && p.name == name);
    let value = matches.next().map(|p| &p.value);
    if matches.next().is_some() {
        return Err(GraphRefusal::Duplicate);
    }
    Ok(value)
}
fn count(face: &Face, subject: &str, name: &str) -> Result<u64, GraphRefusal> {
    match property(face, subject, name)? {
        Some(V::Count(v)) => Ok(*v),
        _ => Err(GraphRefusal::Missing),
    }
}
fn quantity(
    face: &Face,
    subject: &str,
    name: &str,
    target: Unit,
    role: QuantityRole,
) -> Result<Option<i64>, GraphRefusal> {
    let Some(V::TypedValue { contract, bytes }) = property(face, subject, name)? else {
        return if property(face, subject, name)?.is_none() {
            Ok(None)
        } else {
            Err(GraphRefusal::InvalidProperty)
        };
    };
    contract
        .validate(bytes)
        .map_err(|_| GraphRefusal::InvalidProperty)?;
    let q = Quantity::decode(bytes).map_err(|_| GraphRefusal::InvalidProperty)?;
    if q.role() != role || q.family() != target.family() {
        return Err(GraphRefusal::InvalidProperty);
    }
    let expected = if target == Unit::Celsius {
        TEMPERATURE_INFO_ID
    } else {
        RATIO_INFO_ID
    };
    if contract.value_kind != kind_id(expected) {
        return Err(GraphRefusal::InvalidProperty);
    }
    quantity_milli(q, target).map(Some)
}
pub(crate) fn quantity_milli(q: Quantity, target: Unit) -> Result<i64, GraphRefusal> {
    let (coefficient, source_exponent) = q
        .convert_to_unit(target)
        .map_err(|_| GraphRefusal::InvalidProperty)?;
    let exponent = i32::from(source_exponent) + 3;
    let value = if exponent >= 0 {
        coefficient.checked_mul(
            10_i128
                .checked_pow(exponent as u32)
                .ok_or(GraphRefusal::InvalidProperty)?,
        )
    } else {
        let divisor = 10_i128
            .checked_pow((-exponent) as u32)
            .ok_or(GraphRefusal::InvalidProperty)?;
        if coefficient % divisor != 0 {
            return Err(GraphRefusal::InvalidProperty);
        }
        coefficient.checked_div(divisor)
    };
    i64::try_from(value.ok_or(GraphRefusal::InvalidProperty)?)
        .map_err(|_| GraphRefusal::InvalidProperty)
}

pub(crate) fn milli_label(value: i64) -> alloc::string::String {
    let magnitude = i128::from(value).abs();
    alloc::format!(
        "{}{}.{:03}",
        if value < 0 { "-" } else { "" },
        magnitude / 1000,
        magnitude % 1000
    )
}
