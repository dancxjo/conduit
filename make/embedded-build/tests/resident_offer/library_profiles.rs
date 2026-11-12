//! Emitted mixed-profile preflight retains only original finite owner loans.
pub fn run(library: &mut crate::library::Initialized, assignment: &crate::support::Assignment) {
    use conduit_core as c;
    use conduit_plan_lowering::{
        selected_assigned_expression::SelectedAssignedLineForeExpressionPlan,
        selected_assigned_operation::AssignedOperationInput,
    };
    let original = library.offers().map(|image| {
        let image = image.unwrap();
        (image.as_ptr(), image.len())
    });
    let active = assignment.active();
    // Schema 8 also supports a wholly local profile. This tests the new emitted
    // dispatch and exact checker without claiming a carrier or an admitted Line.
    let mut bytes = assignment.encoded[..c::ASSIGNED_PLAN_HEADER_BYTES].to_vec();
    bytes[8..10].copy_from_slice(&c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA.to_le_bytes());
    for record in c::AssignedPlanRecords::new(&assignment.encoded).unwrap() {
        let record = record.unwrap();
        bytes.push(record.tag);
        if record.tag == c::ASSIGNED_LOCAL_FORE {
            let pin = assignment.fore.iter().find(|pin| pin.payload() == record.payload).unwrap();
            let payload = c::assigned_line_fore_local_payload(pin);
            bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            bytes.extend_from_slice(&payload);
        } else {
            bytes.extend_from_slice(&(record.payload.len() as u16).to_le_bytes());
            bytes.extend_from_slice(record.payload);
        }
    }
    let length = u16::try_from(bytes.len()).unwrap();
    bytes[10..12].copy_from_slice(&length.to_le_bytes());
    let digest = c::assigned_plan_payload_digest(&bytes[c::ASSIGNED_PLAN_HEADER_BYTES..]);
    bytes[93..125].copy_from_slice(&digest);
    let select = |local| {
        let mut expression = assignment.selected(&active);
        expression.input = AssignedOperationInput::Contiguous(&bytes);
        expression.fore = None;
        crate::library::ResidentAssignment::LineFore(SelectedAssignedLineForeExpressionPlan {
            expression,
            fore: c::AssignedLineForeProfile { local, lines: &[] },
        })
    };
    let (prepared, cost) = crate::probe::observe(|| {
        library
            .prepare_selected_profiles_array([Some(select(&assignment.fore)), None])
            .unwrap()
    });
    assert_eq!(prepared.nodes(), [Some(0), None]);
    assert!(core::mem::size_of_val(&prepared) <= 3 * core::mem::size_of::<usize>());
    drop(prepared);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let (result, cost) = crate::probe::observe(|| {
        library.prepare_selected_profiles_array([Some(select(&[])), None])
    });
    assert!(result.is_err());
    drop(result);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(
        library.offers().map(|image| {
            let image = image.unwrap();
            (image.as_ptr(), image.len())
        }),
        original
    );
}
