//! Exact borrowed configuration comparison for a selected assigned node.
use crate::selected_operation::SelectedCallRefusal as E;
use conduit_core as c;
use conduit_kernel::NodeId;

pub(super) fn check(
    input: super::AssignedOperationInput<'_>,
    node: NodeId,
    expected: &[c::ConfigurationEntry],
) -> Result<(), E> {
    let mut count = 0;
    for record in input.records().map_err(|_| E::WrongBinding)? {
        let record = record.map_err(|_| E::WrongBinding)?;
        if record.tag != c::ASSIGNED_CONFIGURATION {
            continue;
        }
        let p = record.payload;
        if p.get(..2) != Some(node.0.to_le_bytes().as_slice()) {
            continue;
        }
        let entry = expected.get(count).ok_or(E::WrongBinding)?;
        if expected[..count].iter().any(|prior| prior.key == entry.key)
            || p.get(2..18) != Some(c::AssignedIdentity::from_text(&entry.key).0.as_slice())
        {
            return Err(E::WrongBinding);
        }
        let matches = match &entry.value {
            c::ConfigurationValue::Bool(v) => {
                p.get(18) == Some(&0)
                    && p.get(19..) == Some([u8::from(*v), 0, 0, 0, 0, 0, 0, 0].as_slice())
            }
            c::ConfigurationValue::I64(v) => {
                p.get(18) == Some(&1) && p.get(19..) == Some(v.to_le_bytes().as_slice())
            }
            c::ConfigurationValue::U64(v) => {
                p.get(18) == Some(&2) && p.get(19..) == Some(v.to_le_bytes().as_slice())
            }
            c::ConfigurationValue::Text(v) => {
                p.get(18) == Some(&3) && p.get(19..) == Some(v.as_bytes())
            }
            // The current assigned schema carries only these four primitives.
            c::ConfigurationValue::Quantity(_)
            | c::ConfigurationValue::Structured(_)
            | c::ConfigurationValue::Unit(_)
            | c::ConfigurationValue::TemperatureDifference(_) => false,
        };
        if !matches {
            return Err(E::WrongBinding);
        }
        count += 1;
    }
    if count != expected.len() {
        return Err(E::WrongBinding);
    }
    Ok(())
}
