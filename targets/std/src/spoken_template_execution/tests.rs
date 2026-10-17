use super::*;

fn delivery(port: &str, sequence: u64) -> ExternalForeDelivery {
    ExternalForeDelivery {
        front_port_id: port_id(port),
        track: ConnectionTrack::Payload,
        value_kind: conduit_face::SHOW_VALUE_KIND.into(),
        sequence,
        bytes: vec![],
    }
}

#[test]
fn opening_collector_refuses_wrong_fore_and_duplicate_without_replacing_actual_delivery() {
    let mut capture = OneShow(None);
    assert!(capture.deliver(delivery("interaction", 0)).is_err());
    assert!(capture.0.is_none());
    assert!(capture.deliver(delivery("show", 1)).is_err());
    assert!(capture.0.is_none());
    let mut wrong_kind = delivery("show", 0);
    wrong_kind.value_kind = "value/text".into();
    assert!(capture.deliver(wrong_kind).is_err());
    assert!(capture.0.is_none());
    capture.deliver(delivery("show", 0)).unwrap();
    assert!(capture.deliver(delivery("show", 0)).is_err());
    assert_eq!(capture.0.as_ref().unwrap().front_port_id, port_id("show"));
    assert_eq!(capture.0.as_ref().unwrap().sequence, 0);
}
