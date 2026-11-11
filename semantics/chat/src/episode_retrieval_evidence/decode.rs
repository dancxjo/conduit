use super::*;
use core::fmt;
use serde::de::{self, MapAccess, SeqAccess, Visitor};

impl<'de, const N: usize> Deserialize<'de> for FixedEvidenceText<N> {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Text<const N: usize>;
        impl<const N: usize> Visitor<'_> for Text<N> {
            type Value = FixedEvidenceText<N>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded UTF-8 text")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v.len() > N {
                    return Err(E::custom("text bound"));
                }
                let mut value = FixedEvidenceText::default();
                value.bytes[..v.len()].copy_from_slice(v.as_bytes());
                value.length = v.len();
                Ok(value)
            }
        }
        d.deserialize_str(Text::<N>)
    }
}
fn duplicate<E: de::Error>(seen: &mut u16, bit: u16) -> Result<(), E> {
    if *seen & bit != 0 {
        return Err(E::custom("duplicate metadata field"));
    }
    *seen |= bit;
    Ok(())
}
fn skip<'de, A: MapAccess<'de>>(map: &mut A) -> Result<(), A::Error> {
    map.next_value_seed(super::skip::Skip {
        depth: 1,
        nodes: &mut 0,
    })
}

pub(super) struct Root {
    pub revision: Option<u64>,
    pub provenance_revision: Option<FixedEvidenceText<256>>,
    pub items: Items,
    pub time_fields: Option<[FixedEvidenceText<32>; 5]>,
}
impl<'de> Deserialize<'de> for Root {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct RootVisitor;
        impl<'de> Visitor<'de> for RootVisitor {
            type Value = Root;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("compact retained evidence")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Root, A::Error> {
                let mut root = Root {
                    revision: None,
                    provenance_revision: None,
                    items: Items::default(),
                    time_fields: None,
                };
                let mut seen = 0;
                while let Some(key) = map.next_key::<FixedEvidenceText<256>>()? {
                    match key.as_str() {
                        "revision" => {
                            duplicate(&mut seen, 1)?;
                            root.revision = map.next_value()?;
                        }
                        "provenance_revision" => {
                            duplicate(&mut seen, 2)?;
                            root.provenance_revision = map.next_value()?;
                        }
                        "time_fields" => {
                            duplicate(&mut seen, 8)?;
                            root.time_fields = map.next_value()?;
                        }
                        "evidence" => {
                            duplicate(&mut seen, 4)?;
                            root.items = map.next_value()?;
                        }
                        _ => skip(&mut map)?,
                    }
                }
                Ok(root)
            }
        }
        d.deserialize_map(RootVisitor)
    }
}
#[derive(Default)]
pub(super) struct Items {
    pub items: [EpisodeEvidenceMetadata; 8],
    pub length: usize,
}
impl<'de> Deserialize<'de> for Items {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ItemsVisitor;
        impl<'de> Visitor<'de> for ItemsVisitor {
            type Value = Items;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("at most eight evidence items")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Items, A::Error> {
                let mut result = Items::default();
                while let Some(item) = seq.next_element()? {
                    if result.length == 8 {
                        return Err(de::Error::custom("item bound"));
                    }
                    result.items[result.length] = item;
                    result.length += 1;
                }
                Ok(result)
            }
        }
        d.deserialize_seq(ItemsVisitor)
    }
}
impl<'de> Deserialize<'de> for EpisodeEvidenceMetadata {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ItemVisitor;
        impl<'de> Visitor<'de> for ItemVisitor {
            type Value = EpisodeEvidenceMetadata;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("evidence metadata")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut item = EpisodeEvidenceMetadata::default();
                let mut seen = 0;
                while let Some(key) = map.next_key::<FixedEvidenceText<256>>()? {
                    match key.as_str() {
                        "identity" => {
                            duplicate(&mut seen, 1)?;
                            item.identity = map.next_value()?;
                        }
                        "origin" => {
                            duplicate(&mut seen, 2)?;
                            item.origin = map.next_value::<Option<Origin>>()?.and_then(|v| v.0);
                        }
                        "observed_at" => {
                            duplicate(&mut seen, 4)?;
                            item.observed_at = map.next_value()?;
                        }
                        "temporal_role" => {
                            duplicate(&mut seen, 8)?;
                            skip(&mut map)?;
                        }
                        "utf8_content" => {
                            duplicate(&mut seen, 16)?;
                            skip(&mut map)?;
                        }
                        "sources" => {
                            duplicate(&mut seen, 32)?;
                            skip(&mut map)?;
                        }
                        _ => skip(&mut map)?,
                    }
                }
                if item
                    .identity
                    .as_ref()
                    .is_some_and(|v| v.as_str().is_empty())
                {
                    return Err(de::Error::custom("empty identity"));
                }
                Ok(item)
            }
        }
        d.deserialize_map(ItemVisitor)
    }
}
struct Origin(Option<EpisodeEvidenceOrigin>);
impl<'de> Deserialize<'de> for Origin {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use EpisodeEvidenceOrigin::*;
        let value = FixedEvidenceText::<256>::deserialize(d)?;
        Ok(Self(match value.as_str() {
            "observation" => Some(Observation),
            "human_reported" => Some(HumanReported),
            "deterministic_derived" => Some(DeterministicDerived),
            "remembered" => Some(Remembered),
            "model_derived" => Some(ModelDerived),
            "imagined" => Some(Imagined),
            _ => None,
        }))
    }
}
impl<'de> Deserialize<'de> for EpisodeObservedInstant {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct InstantVisitor;
        impl<'de> Visitor<'de> for InstantVisitor {
            type Value = EpisodeObservedInstant;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("exact temporal instant")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let missing = || <A::Error as de::Error>::custom("missing instant column");
                let value = EpisodeObservedInstant {
                    clock_basis: seq.next_element()?.ok_or_else(missing)?,
                    scale: seq.next_element::<Scale>()?.ok_or_else(missing)?.0,
                    ticks: seq.next_element()?.ok_or_else(missing)?,
                    resolution_ticks: seq.next_element()?.ok_or_else(missing)?,
                    uncertainty_ticks: seq.next_element()?.ok_or_else(missing)?,
                    compact_array: true,
                };
                if seq.next_element::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("extra instant column"));
                }
                value
                    .borrowed()
                    .validate()
                    .map_err(|_| de::Error::custom("invalid instant"))?;
                Ok(value)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let (mut basis, mut scale, mut ticks, mut resolution, mut uncertainty) =
                    (None, None, None, None, None);
                let mut seen = 0;
                while let Some(key) = map.next_key::<FixedEvidenceText<256>>()? {
                    match key.as_str() {
                        "clock_basis" => {
                            duplicate(&mut seen, 1)?;
                            basis = Some(map.next_value()?);
                        }
                        "scale" => {
                            duplicate(&mut seen, 2)?;
                            scale = Some(map.next_value::<Scale>()?.0);
                        }
                        "ticks" => {
                            duplicate(&mut seen, 4)?;
                            ticks = Some(map.next_value()?);
                        }
                        "resolution_ticks" => {
                            duplicate(&mut seen, 8)?;
                            resolution = Some(map.next_value()?);
                        }
                        "uncertainty_ticks" => {
                            duplicate(&mut seen, 16)?;
                            uncertainty = Some(map.next_value()?);
                        }
                        _ => skip(&mut map)?,
                    }
                }
                let missing = || <A::Error as de::Error>::custom("missing instant field");
                let value = EpisodeObservedInstant {
                    clock_basis: basis.ok_or_else(missing)?,
                    scale: scale.ok_or_else(missing)?,
                    ticks: ticks.ok_or_else(missing)?,
                    resolution_ticks: resolution.ok_or_else(missing)?,
                    uncertainty_ticks: uncertainty.ok_or_else(missing)?,
                    compact_array: false,
                };
                value
                    .borrowed()
                    .validate()
                    .map_err(|_| de::Error::custom("invalid instant"))?;
                // Preserve uncertainty exactly; interval overflow is refused by correspondence, not rewritten here.
                Ok(value)
            }
        }
        d.deserialize_ignored_any(InstantVisitor)
    }
}
struct Scale(TemporalScale);
impl<'de> Deserialize<'de> for Scale {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = FixedEvidenceText::<16>::deserialize(d)?;
        Ok(Self(match text.as_str() {
            "seconds" => TemporalScale::Seconds,
            "milliseconds" => TemporalScale::Milliseconds,
            "microseconds" => TemporalScale::Microseconds,
            "nanoseconds" => TemporalScale::Nanoseconds,
            _ => return Err(de::Error::custom("unknown scale")),
        }))
    }
}
