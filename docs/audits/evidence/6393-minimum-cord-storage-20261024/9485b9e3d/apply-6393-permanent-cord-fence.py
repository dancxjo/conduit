from pathlib import Path
root=Path('work/6393-permanent-cord-bound')
p=root/'architecture/planner/src/lib.rs';s=p.read_text();assert 'mod value_storage;' not in s;s=s.replace('mod validated_planning;', 'mod validated_planning;\nmod value_storage;');assert 'mod value_storage;' in s;p.write_text(s)
p=root/'architecture/planner/src/validated_planning.rs';s=p.read_text();needle='        let resource = resource_port::plan_resource_connection(';assert s.count(needle)==1;s=s.replace(needle,'''        crate::value_storage::validate_capacity(
            connection,
            source_gear,
            sink_gear,
            limits.byte_capacity,
        )?;
'''+needle);p.write_text(s)
(root/'architecture/planner/src/value_storage.rs').write_text('''//! Guaranteed minimum value storage, distinct from permitted maximum size.
use crate::PlannerError;
use conduit_core::{CheckedFront, ConnectionTrack, FrontValueLocation, ValueConstraint};
use conduit_plot::{CheckedConnection, CheckedGear};

fn minimum_at(front: &CheckedFront, location: FrontValueLocation) -> u32 {
    front.value_contracts().iter()
        .filter(|bound| bound.location == location)
        .flat_map(|bound| &bound.contract.constraints)
        .filter_map(|constraint| match constraint {
            ValueConstraint::ByteLength { minimum, .. } => Some(*minimum),
            _ => None,
        })
        .max().unwrap_or_default()
}

fn required_minimum(
    source: &CheckedFront, source_port: &conduit_core::PortId,
    sink: &CheckedFront, sink_port: &conduit_core::PortId,
    track: ConnectionTrack,
) -> u32 {
    let source_location = match track {
        ConnectionTrack::Payload => FrontValueLocation::Output(source_port.clone()),
        ConnectionTrack::AbnormalTerminal => FrontValueLocation::OutputAbnormal(source_port.clone()),
        ConnectionTrack::NormalClose | ConnectionTrack::Quiescence => return 0,
    };
    minimum_at(source, source_location)
        .max(minimum_at(sink, FrontValueLocation::Input(sink_port.clone())))
}

pub(crate) fn validate_capacity(
    connection: &CheckedConnection,
    source: &CheckedGear,
    sink: &CheckedGear,
    byte_capacity: u32,
) -> Result<(), PlannerError> {
    let required = required_minimum(
        &source.checked_front(), &connection.source_port_id,
        &sink.checked_front(), &connection.sink_port_id, connection.track,
    );
    if byte_capacity < required {
        return Err(PlannerError::InvalidConnectionBudget(alloc::format!(
            "value/storage-bound@1: '{}:{}' to '{}:{}' requires at least {} bytes before Play, admitted {}",
            connection.source_gear_id.as_str(), connection.source_port_id.as_str(),
            connection.sink_gear_id.as_str(), connection.sink_port_id.as_str(),
            required, byte_capacity,
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, port_id, CheckedValueContract, FrontValueContract};

    fn contract(location: FrontValueLocation, minimum: Option<u32>, maximum: u32) -> FrontValueContract {
        FrontValueContract {
            location,
            contract: CheckedValueContract::new(kind_id("value/bytes"), maximum,
                minimum.map(|minimum| ValueConstraint::ByteLength { minimum, maximum }).into_iter().collect()).unwrap(),
        }
    }

    #[test]
    fn variable_upper_bound_does_not_require_a_full_size_cord() {
        let source = CheckedFront::new(vec![], vec![], vec![], None)
            .with_value_contracts(vec![contract(FrontValueLocation::Output(port_id("out")), None, 256)]);
        let sink = CheckedFront::new(vec![], vec![], vec![], None);
        assert_eq!(required_minimum(&source, &port_id("out"), &sink, &port_id("in"), ConnectionTrack::Payload), 0);
    }

    #[test]
    fn terminal_tracks_use_their_own_minimum_and_nonvalue_tracks_need_none() {
        let source = CheckedFront::new(vec![], vec![], vec![], None).with_value_contracts(vec![
            contract(FrontValueLocation::Output(port_id("out")), Some(8192), 8192),
            contract(FrontValueLocation::OutputAbnormal(port_id("out")), Some(73), 73),
        ]);
        let sink = CheckedFront::new(vec![], vec![], vec![], None);
        for (track, expected) in [(ConnectionTrack::Payload, 8192), (ConnectionTrack::AbnormalTerminal, 73),
            (ConnectionTrack::NormalClose, 0), (ConnectionTrack::Quiescence, 0)] {
            assert_eq!(required_minimum(&source, &port_id("out"), &sink, &port_id("in"), track), expected);
        }
    }
}
''')
p=root/'semantics/catalog/src/contract.rs';s=p.read_text();s=s.replace('portable_value_maximum_bytes(self.kind_id.as_str(), port.value_kind.as_str()).map(', 'portable_value_bounds(self.kind_id.as_str(), port.value_kind.as_str()).map(').replace('|maximum_bytes| FrontValueContract {','|(minimum_bytes, maximum_bytes)| FrontValueContract {').replace('                            maximum_bytes as u32,\n                            Vec::new(),','''                            maximum_bytes as u32,
                            if minimum_bytes == 0 { Vec::new() } else {
                                alloc::vec![conduit_core::ValueConstraint::ByteLength {
                                    minimum: minimum_bytes as u32, maximum: maximum_bytes as u32,
                                }]
                            },''')
s=s.replace('fn portable_value_maximum_bytes(owner: &str, kind: &str) -> Option<u64> {','fn portable_value_bounds(owner: &str, kind: &str) -> Option<(u64, u64)> {').replace('return Some(conduit_core::QUANTITY_ENCODED_LEN as u64);','let bytes = conduit_core::QUANTITY_ENCODED_LEN as u64;\n        return Some((bytes, bytes));')
s=s.replace('''        return Some(
            (crate::quantity_info_prefix().len() + conduit_core::QUANTITY_ENCODED_LEN) as u64,
        );''','''        let bytes = (crate::quantity_info_prefix().len() + conduit_core::QUANTITY_ENCODED_LEN) as u64;
        return Some((bytes, bytes));''')
for c in ['MAX_FACE_COMPOSITION_BYTES','MAX_LAYOUT_FRAME_BYTES','MAX_GRAPHICS_SCENE_BYTES','MAX_GRAY8_BITMAP_BYTES']:s=s.replace('Some(conduit_face::'+c+' as u64)', 'Some((0, conduit_face::'+c+' as u64))')
assert 'portable_value_maximum_bytes' not in s;p.write_text(s)
p=root/'semantics/catalog/src/quantity_info.rs';s=p.read_text();needle='''            (quantity_info_prefix().len() + QUANTITY_ENCODED_LEN) as u32,
            Vec::new(),''';assert s.count(needle)==1;s=s.replace(needle,'''            (quantity_info_prefix().len() + QUANTITY_ENCODED_LEN) as u32,
            vec![conduit_core::ValueConstraint::ByteLength {
                minimum: (quantity_info_prefix().len() + QUANTITY_ENCODED_LEN) as u32,
                maximum: (quantity_info_prefix().len() + QUANTITY_ENCODED_LEN) as u32,
            }],''');p.write_text(s)
