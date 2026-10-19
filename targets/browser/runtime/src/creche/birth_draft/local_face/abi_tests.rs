//! ABI proof over the real CURRENT owner and trusted local Mask executor.
use super::*;
use serde_json::{json, Value};

fn input(value: &Value) -> usize {
    let bytes = serde_json::to_vec(value).unwrap();
    assert!(bytes.len() <= abi::INPUT_BYTES);
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr(),
            abi::conduit_creche_input_ptr() as *mut u8,
            bytes.len(),
        );
    }
    bytes.len()
}
fn output() -> Value {
    let bytes = unsafe {
        std::slice::from_raw_parts(
            transport::conduit_creche_local_face_output_ptr() as *const u8,
            transport::conduit_creche_local_face_output_len(),
        )
    };
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn canonical_abi_prepares_acknowledges_delivers_and_closes_the_actual_current_draft() {
    assert_eq!(
        transport::conduit_creche_local_face_component_descriptor(),
        0
    );
    assert_eq!(
        output()["bundle_sha256"],
        crate::owner_face_mask::local_component::expected_admission().bundle_sha256
    );
    CURRENT.with(|state| {
        let mut state = state.borrow_mut();
        state.close_local_face().unwrap();
        state
            .open(OpenRequest {
                persona_uuid: "11111111-2222-4333-8444-555555555555".into(),
                choices: vec![Choice {
                    title: "Clock".into(),
                    search_text: "clock".into(),
                    plot: ResidentPlot {
                        source_document_id: "source/clock".into(),
                        checked_plot_id: "checked/clock".into(),
                    },
                    selected: false,
                }],
            })
            .unwrap();
    });
    let generation = CURRENT.with(|state| state.borrow().generation);
    let length = input(
        &json!({"generation":generation,"host_id":"host/local-abi","boot_id":"boot/local-abi","component":true,"component_admission":crate::owner_face_mask::local_component::expected_admission()}),
    );
    assert_eq!(transport::conduit_creche_local_face_prepare(length), 0);
    let prepared = output();
    assert_eq!(prepared["schema"], "conduit.browser/creche-local-face@1");
    assert!(prepared["encounter"]["face"]["basis"]["body_id"].is_null());
    let view = &prepared["encounter"]["view"];
    let length = input(
        &json!({"generation":generation,"show_id":view["show_id"],"face_id":view["face_id"],"face_revision":view["face_revision"]}),
    );
    assert_eq!(transport::conduit_creche_local_face_ack(length), 0);
    let action = prepared["encounter"]["face"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["identity"] == "creche.name")
        .unwrap();
    let length = input(&json!({"generation":generation,"interaction":{
        "show_id":view["show_id"],"face_id":view["face_id"],"face_revision":view["face_revision"],
        "action_id":action["identity"],"target":action["target"],"sequence":1,
        "arguments":[{"name":"value","value_kind":conduit_face::UTF8_TEXT_VALUE_KIND,"value":"Café 東京".as_bytes()}]}}));
    assert_eq!(transport::conduit_creche_local_face_interact(length), 0);
    let committed = output();
    assert_eq!(
        committed["delivered_interaction"]["arguments"][0]["value"],
        json!("Café 東京".as_bytes())
    );
    assert_eq!(committed["owner_revision"], 2);
    assert_eq!(committed["source_snapshot"]["friendly_name"], "Café 東京");
    assert_eq!(committed["source_snapshot"]["revision"], 2);
    CURRENT.with(|state| {
        assert_eq!(
            state.borrow().draft.as_ref().unwrap().friendly_name(),
            "Café 東京"
        )
    });
    assert_eq!(transport::conduit_creche_local_face_close(generation), 0);
    let length = input(&json!({"generation":generation,"interaction":{}}));
    assert_eq!(
        transport::conduit_creche_local_face_interact(length),
        ERROR_DRAFT
    );
    assert_eq!(output()["schema"], "conduit.browser/creche-local-refusal@1");
}
