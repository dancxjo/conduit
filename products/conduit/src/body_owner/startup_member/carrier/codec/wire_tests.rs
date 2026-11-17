//! Actual sender encoding through receiver wire types and token-authenticated control.
use super::*;
#[test]
fn fixed_commit_wire_preserves_retained_scope_and_refuses_missing_fields() {
    let (host, session, receipt, mut receiver) =
        crate::durable_host_control::startup_member::prepared_wire_custody();
    let root = receiver.joined.root.parent().unwrap().join("wire-owner");
    std::fs::create_dir(&root).unwrap();
    let mut installation: crate::durable_host::Installation = serde_json::from_slice(
        &std::fs::read(receiver.joined.root.join("installation.json")).unwrap(),
    )
    .unwrap();
    installation.host_id = host.advertisement().host_id.as_str().into();
    installation.joined_body_state = None;
    installation.body_state = None;
    std::fs::write(
        root.join("installation.json"),
        serde_json::to_vec(&installation).unwrap(),
    )
    .unwrap();
    let resident = session.evidence().body.workset.plots()[0].clone();
    let mut owner = Owner::open(host, resident, None, "Actual wire custody").unwrap();
    owner.session = session;
    owner.persist(&root).unwrap();
    let retained = super::super::super::super::super::state::load(&root)
        .unwrap()
        .unwrap();
    let workspace = owner
        .host
        .prepare_startup_member_control_workspace()
        .unwrap()
        .into_shared();
    let session_id = receiver.wire_session_id();
    let length = encode_commit(&workspace, session_id, &receipt, &retained).unwrap();
    let bytes = workspace.lock().unwrap().write_buffer()[..length].to_vec();
    for field in ["biography", "preparation_id"] {
        let mut missing: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        missing["command"].as_object_mut().unwrap().remove(field);
        let invoked = std::cell::Cell::new(false);
        assert!(crate::host_rendezvous::wire_fixture_exchange(
            &serde_json::to_vec(&missing).unwrap(),
            |command| {
                invoked.set(true);
                receiver.dispatch(command)
            }
        )
        .is_err());
        assert!(!invoked.get());
        assert!(receiver.retains_ticket());
    }
    let invoked = std::cell::Cell::new(false);
    assert!(
        crate::host_rendezvous::wire_fixture_exchange(b"{", |command| {
            invoked.set(true);
            receiver.dispatch(command)
        })
        .is_err()
    );
    assert!(!invoked.get());
    let reply =
        crate::host_rendezvous::wire_fixture_exchange(&bytes, |command| receiver.dispatch(command))
            .unwrap();
    crate::durable_host_control::startup_member::preflight::reply(&reply).unwrap();
    let response: Response = serde_json::from_slice(&reply).unwrap();
    assert_eq!(
        (response.kind.as_str(), response.protocol),
        ("startup-member", 1)
    );
    assert!(matches!(response.reply,Reply::Committed {receipt:actual} if *actual==receipt));
    assert!(matches!(
        receiver
            .dispatch(Command::Cancel {
                session_id,
                preparation_id: receipt.preparation_id
            })
            .unwrap(),
        Reply::Progress {
            progress: Progress::Terminal,
            ..
        }
    ));
    receiver.release().unwrap();
}
