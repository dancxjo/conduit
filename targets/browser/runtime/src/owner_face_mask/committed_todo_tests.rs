//! Explicit producer/consumer proof over freshly executed owner checkpoints.
//! Run the owner producer first with CONDUIT_CROSS_MASK_TODO_EXCHANGE set.
use super::*;

#[test]
#[ignore = "requires a freshly executed owner checkpoint exchange"]
fn checked_browser_mask_consumes_actual_committed_todo_exchange() {
    let path = std::env::var_os("CONDUIT_CROSS_MASK_TODO_EXCHANGE").expect("owner exchange path");
    let recorded: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        recorded["schema"],
        "conduit.proof/committed-todo-mask-exchange@1"
    );
    let exchanges = recorded["exchanges"].as_array().unwrap();
    assert_eq!(exchanges.len(), 2);
    let mut body = None;
    for (index, exchange) in exchanges.iter().enumerate() {
        let face: Presentation = serde_json::from_value(exchange["face"].clone()).unwrap();
        face.validate().unwrap();
        let body_id = face.basis.body_id.clone().unwrap();
        if let Some(previous) = &body {
            assert_eq!(previous, &body_id);
        }
        body = Some(body_id.clone());
        let receipt = &exchange["verified_read_receipt"];
        assert_eq!(receipt["body_id"], body_id.as_str());
        assert_eq!(receipt["read_terminal"], "Completed");
        assert_eq!(receipt["write"]["terminal"], "Completed");
        for sign in [
            &receipt["read_terminal_sign"]["sign_id"],
            &receipt["write"]["terminal_sign"]["sign_id"],
        ] {
            let sign = sign.as_str().unwrap();
            assert!(face.basis.sign_ids.iter().any(|id| id.as_str() == sign));
        }
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                visual_constraints: None,
                body_id,
                host_id: HostId::from("host/browser-committed-todo-fixture"),
                boot_id: BootId::from("boot/browser-committed-todo-fixture"),
            },
            face.clone(),
            index as u64 + 1,
            true,
        )
        .unwrap();
        let view = mask.view();
        let items = view
            .subjects
            .iter()
            .filter(|subject| subject.role == "Item")
            .collect::<Vec<_>>();
        assert_eq!(items.len(), 3);
        for name in ["Milk", "Eggs", "Bread"] {
            assert!(items.iter().any(|item| item.name == name));
        }
        let remaining = items
            .iter()
            .filter(|item| {
                item.flags
                    .iter()
                    .any(|flag| flag.name == "complete" && !flag.value)
            })
            .count();
        assert_eq!(remaining, 3 - index);
        assert_eq!(exchange["remaining"], remaining);
        mask.acknowledge(Acknowledgement {
            show_id: view.show_id,
            face_id: view.face_id,
            face_revision: view.face_revision,
        })
        .unwrap();
        assert_eq!(mask.view().show_state, "available");
        assert_eq!(mask.presentation, face);
    }
}
