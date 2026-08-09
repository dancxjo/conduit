
use super::*;

#[test]
fn malformed_and_oversized_presentation_info_fail_before_rendering() {
    assert!(matches!(
        decode_presentation(b"{"),
        Err(CrossHostRendererError::Presentation(_))
    ));
    assert!(matches!(
        decode_presentation(&vec![0; MAX_RENDERER_VALUE_BYTES as usize + 1]),
        Err(CrossHostRendererError::Presentation(_))
    ));
}

#[test]
fn absent_planned_line_fails_without_a_manifestation() {
    let identity = RendererAdapterIdentity {
        host_id: HostId::from("patchbay-html/host"),
        boot_id: BootId::from("patchbay-html/boot"),
        target_subject: "patchbay-html/document-0".into(),
    };
    let exact = cross_host_renderer_plan(
        HostId::from("patchbay-presentation/host"),
        BootId::from("patchbay-presentation/boot"),
        identity.clone(),
    )
    .unwrap();
    let sink_fragment = fragment_for(&exact.plan, &exact.renderer_advertisement.host_id).unwrap();
    let source_fragment = fragment_for(&exact.plan, &exact.source_advertisement.host_id).unwrap();
    let sink = Sink::prepare(exact.plan.clone(), sink_fragment, source_fragment).unwrap();
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    drop(socket);

    assert!(matches!(
        sink.run(&format!("ws://{address}"), identity),
        Err(CrossHostRendererError::Line(_))
    ));
}
