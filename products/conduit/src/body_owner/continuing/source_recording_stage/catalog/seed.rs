//! Extract the canonical authored keep, using the planner's exact Gear State ID.
use super::*;
pub(super) struct Seed {
    pub(super) request: BodyStorageStateRequest,
}
pub(super) fn extract(
    plot: &conduit_plot::ExpandedAuthoringPlot,
    body: &str,
) -> Result<Seed, String> {
    let mut keeps = plot
        .expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == c::STATE_VALUE_KIND);
    let keep = keeps.next().ok_or("catalog has no authored seed keep")?;
    if keeps.next().is_some() {
        return Err("catalog has multiple seed keeps".into());
    }
    let initial = keep
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("initial", c::ConfigurationValue::Structured(value)) => Some(value),
            _ => None,
        })
        .ok_or("catalog keep lacks typed initializer")?;
    let default_value =
        c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).map_err(debug)?;
    if !matches!(default_value.shape(), c::StructuredInfoValueShape::Variant { tag, .. } if tag == "none")
    {
        return Err("catalog seed initializer is not canonical None".into());
    }
    let maximum = keep
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-bytes", c::ConfigurationValue::U64(value)) => u32::try_from(*value).ok(),
            _ => None,
        })
        .ok_or("catalog keep lacks bounded storage")?;
    let lifetime = keep
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("retained-duration", c::ConfigurationValue::Text(value)) => Some(value.as_str()),
            _ => None,
        });
    let [output] = keep.outputs.as_slice() else {
        return Err("catalog keep output differs".into());
    };
    if maximum != 1024 || lifetime != Some("body") || output.temporal != c::PortTemporal::Current {
        return Err("catalog seed differs from exact authored Body keep profile".into());
    }
    let type_name = keep
        .startup_parameters
        .first()
        .ok_or("catalog keep type absent")?
        .value_type
        .as_str()
        .to_owned();
    Ok(Seed {
        request: BodyStorageStateRequest {
            binding: conduit_std_host::state_value::DurableStateBinding {
                body: body.into(),
                state: keep.gear_id.as_str().into(),
                value_kind: output.value_kind.clone(),
                maximum_value_bytes: maximum,
            },
            type_name,
            value_type: default_value.value_type().clone(),
            default_value,
        },
    })
}
