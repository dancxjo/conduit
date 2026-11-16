//! Repeated descriptor requests reuse one admitted pass, including refusals.
use super::*;
use conduitos::protocol_source::*;

#[test]
fn reusable_descriptor_source_preserves_results_pressure_and_normal_eof_without_play_allocations() {
    let mut oracle = Parser::new();
    let bytes = serde_json::to_vec(&usb_hid_absolute_descriptor_stages_package().unwrap()).unwrap();
    let pass = PreparedProtocolEntry::prepare(&bytes, "usb-hid-absolute-layout-pass").unwrap();
    assert_eq!(
        pass.expanded().expanded.gears.len(),
        14,
        "one four-item pass"
    );
    drop(pass);
    let entry = PreparedProtocolEntry::prepare(&bytes, "usb-hid-absolute-layout-driver").unwrap();
    assert!(
        entry.expanded().expanded.gears.len() <= 10,
        "small Source driver"
    );
    let incoming = entry
        .expanded()
        .front
        .inputs()
        .iter()
        .find(|p| p.port_id.as_str() == "input")
        .unwrap()
        .value_kind
        .clone();
    let mut changed = PREFIX.to_vec();
    changed[40] = 0x03;
    changed[57] = 0x40;
    changed[58] = 0x01;
    let cases: Vec<_> = [
        PREFIX.to_vec(),
        vec![0; 64],
        PREFIX[..65].to_vec(),
        vec![0xc0],
        vec![0x05, 0x0d, 0xa4],
        changed,
        PREFIX.to_vec(),
    ]
    .into_iter()
    .map(|wire| {
        let expected = oracle.run(&wire);
        (
            ValuePayload {
                value_kind: incoming.clone(),
                encoded: input(&oracle.programs[0].input_type, &wire),
            },
            expected,
        )
    })
    .collect();
    drop(entry);
    let mut pipe = PreparedHidDescriptorPipe::prepare(
        "fixture/reusable-hid".into(),
        "fixture/reusable-boot".into(),
    )
    .unwrap();
    let mut complete = false;
    let allocations = super::super::allocation::allocations(|| {
        pipe.start().unwrap();
        for (value, expected) in &cases {
            pipe.submit(value).unwrap();
            assert!(pipe.submit(value).is_err(), "one descriptor in flight");
            assert!(pipe.close().is_err(), "pending work retains custody");
            let mut seen = false;
            for _ in 0..10_000 {
                pipe.step().unwrap();
                if let Some(actual) = pipe.result() {
                    assert_eq!(&actual.encoded, expected, "same existing Source policy");
                    seen = true;
                    break;
                }
            }
            assert!(seen, "bounded parser must finish each request");
            assert!(
                pipe.submit(value).is_err(),
                "unacknowledged output applies pressure"
            );
            assert!(pipe.close().is_err());
            pipe.acknowledge();
        }
        pipe.close().unwrap();
        assert!(pipe.submit(&cases[0].0).is_err(), "EOF cannot reopen");
        for _ in 0..4096 {
            pipe.step().unwrap();
            if pipe.complete() {
                complete = true;
                break;
            }
        }
    });
    assert!(complete, "driver and pass both close normally");
    assert_eq!(allocations, 0);
}
