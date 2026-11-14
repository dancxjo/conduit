//! Checked resident images are eligibility; current offer selection is separate.
use super::*;

pub fn run(a: &crate::support::Assignment) {
    let (doc, program, temporal) =
        crate::support::context_for(a.model.fragments[0].placements[0].inputs[0].temporal);
    let image = b::PreparedResidentExpression::prepare(
        &doc,
        &program,
        temporal,
        crate::support::bounds(1000),
    )
    .unwrap();
    let fragment = &a.model.fragments[0].fragment_id;
    let bound =
        b::bind_current_resident_expressions(&a.model, fragment, &a.advertisement, &[&image])
            .unwrap();
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].computation_requirement(), a.pins[0]);
    for change in 0..6 {
        let mut actual = a.advertisement.clone();
        match change {
            0 => actual.host_id = "fixture/other-host".into(),
            1 => actual.boot_id = "fixture/other-boot".into(),
            2 => actual.offer_generation.0 += 1,
            3 => actual.capabilities.clear(),
            4 => actual.capabilities.push(actual.capabilities[0].clone()),
            _ => actual.capabilities[0].limits.max_queue_bytes += 1,
        }
        assert!(
            b::bind_current_resident_expressions(&a.model, fragment, &actual, &[&image],).is_err(),
            "substitution {change} accepted"
        );
    }
    for registry in [&[][..], &[&image, &image][..]] {
        assert!(b::bind_current_resident_expressions(
            &a.model,
            fragment,
            &a.advertisement,
            registry,
        )
        .is_err());
    }
    // Refused resolution does not consume or mutate either prepared source or
    // actual offer facts; a separately current request can bind normally.
    assert!(
        b::bind_current_resident_expressions(&a.model, fragment, &a.advertisement, &[&image],)
            .is_ok()
    );
}
