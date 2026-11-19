//! Exact portable Core record descendants, excluding unmodelled constructors.
use super::{json::*, semantic, Layout, Refusal};
use conduit_core::*;
fn texts(o: &Object<'_>, indices: &[usize], l: &mut Layout) -> R<()> {
    for i in indices {
        text(o.get(*i)?, l)?;
    }
    Ok(())
}
fn optional_text(raw: Option<&RawValue>, l: &mut Layout) -> R<()> {
    if let Some(raw) = raw {
        if !null(raw) {
            text(raw, l)?;
        }
    }
    Ok(())
}
fn absent_or_null(raw: Option<&RawValue>) -> R<()> {
    if raw.is_none_or(null) {
        Ok(())
    } else {
        Err(Refusal::Unsupported)
    }
}
pub(super) fn offer(raw: &RawValue, l: &mut Layout) -> R<()> {
    let o = object(
        raw,
        &[
            "startup_parameters",
            "shorthand",
            "capability_id",
            "kind_id",
            "kind_contract_revision",
            "inputs",
            "outputs",
            "semantic_contract",
            "execution_profile_id",
            "implementation_id",
            "artifact_id",
            "state_retention",
            "realization_properties",
            "host_calls",
            "resource_requirements",
            "authority_requirements",
            "limits",
        ],
        (1 << 2)
            | (1 << 3)
            | (1 << 4)
            | (1 << 5)
            | (1 << 6)
            | (1 << 7)
            | (1 << 8)
            | (1 << 9)
            | (1 << 10)
            | (1 << 13)
            | (1 << 14)
            | (1 << 15)
            | (1 << 16),
    )?;
    // Reject constructors even if later fields would also fail validation.
    absent_or_null(o.optional(11))?;
    if let Some(raw) = o.optional(12) {
        empty_array(raw)?;
    }
    if let Some(raw) = o.optional(0) {
        array::<FrontStartupParameter>(raw, l, startup)?;
    }
    if let Some(raw) = o.optional(1) {
        if !null(raw) {
            pair(raw, l)?;
        }
    }
    texts(&o, &[2, 3, 4, 8, 9, 10], l)?;
    array::<PortDescriptor>(o.get(5)?, l, port)?;
    array::<PortDescriptor>(o.get(6)?, l, port)?;
    semantic::contract(o.get(7)?, l)?;
    array::<HostCallRequirement>(o.get(13)?, l, host_call)?;
    array::<ResourceRequirement>(o.get(14)?, l, resource)?;
    array::<AuthorityRequirement>(o.get(15)?, l, authority)?;
    let limits = object(
        o.get(16)?,
        &["max_active_instances", "max_queue_items", "max_queue_bytes"],
        7,
    )?;
    unsigned(limits.get(0)?, u16::MAX.into(), false)?;
    unsigned(limits.get(1)?, u16::MAX.into(), false)?;
    unsigned(limits.get(2)?, u32::MAX.into(), false)
}
fn startup(raw: &RawValue, l: &mut Layout) -> R<()> {
    let o = object(raw, &["name", "value_type", "has_default"], 7)?;
    texts(&o, &[0, 1], l)?;
    boolean(o.get(2)?)
}
fn port(raw: &RawValue, l: &mut Layout) -> R<()> {
    Layout::add_fact(&mut l.validation.port_symbols, 1)?;
    let o = object(
        raw,
        &[
            "port_id",
            "value_kind",
            "direction",
            "temporal",
            "abnormal_kind",
        ],
        7,
    )?;
    texts(&o, &[0, 1], l)?;
    word(o.get(2)?, &["Input", "Output"])?;
    if let Some(raw) = o.optional(3) {
        if raw.get().starts_with('"') {
            word(raw, &["Value", "Current"])?;
        } else {
            let (_, flow) = variant(raw, &["Flow"])?;
            let f = object(flow, &["closes"], 1)?;
            boolean(f.get(0)?)?;
        }
    }
    optional_text(o.optional(4), l)
}
fn host_call(raw: &RawValue, l: &mut Layout) -> R<()> {
    let o = object(
        raw,
        &[
            "contract_id",
            "target_kind",
            "maximum_in_flight",
            "maximum_input_bytes",
            "maximum_output_bytes",
        ],
        29,
    )?;
    text(o.get(0)?, l)?;
    optional_text(o.optional(1), l)?;
    unsigned(o.get(2)?, u16::MAX.into(), false)?;
    unsigned(o.get(3)?, u32::MAX.into(), false)?;
    unsigned(o.get(4)?, u32::MAX.into(), false)
}
fn resource(raw: &RawValue, l: &mut Layout) -> R<()> {
    let o = object(
        raw,
        &["class_id", "units", "protected_role", "compute", "content"],
        3,
    )?;
    absent_or_null(o.optional(3))?;
    absent_or_null(o.optional(4))?;
    text(o.get(0)?, l)?;
    unsigned(o.get(1)?, u32::MAX.into(), false)?;
    optional_text(o.optional(2), l)
}
fn authority(raw: &RawValue, l: &mut Layout) -> R<()> {
    let o = object(
        raw,
        &["contract_id", "host_call_contract_id", "subject_kind"],
        7,
    )?;
    texts(&o, &[0, 1, 2], l)
}
fn pair(raw: &RawValue, l: &mut Layout) -> R<()> {
    use serde::de::{Error, SeqAccess, Visitor};
    struct Pair<'a>(&'a mut Layout);
    impl<'de> Visitor<'de> for Pair<'_> {
        type Value = ();
        fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str("an exact shorthand pair")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            for _ in 0..2 {
                let raw = seq
                    .next_element::<&RawValue>()?
                    .ok_or_else(|| A::Error::custom("short shorthand"))?;
                text(raw, self.0).map_err(|_| A::Error::custom("unsupported shorthand"))?;
            }
            if seq.next_element::<&RawValue>()?.is_some() {
                return Err(A::Error::custom("long shorthand"));
            }
            Ok(())
        }
    }
    shape(raw, b'[')?;
    let mut d = serde_json::Deserializer::from_str(raw.get());
    serde::Deserializer::deserialize_seq(&mut d, Pair(l)).map_err(|_| Refusal::Unsupported)?;
    d.end().map_err(|_| Refusal::Json)
}
