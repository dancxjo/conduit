use super::{fixture_decode_borrowed, Command};
use crate::durable_host_control::image_return_ingress::{inspect, ImageBounds};
fn bounds() -> ImageBounds {
    ImageBounds {
        maximum_images: 32,
        maximum_compressed_bytes: 24 * 1024,
    }
}
fn decode(input: &str) -> Result<Command, serde_json::Error> {
    let frame = format!(
        r#"{{"kind":"body-member-image-return","protocol":1,"token":[7],"command":{input}}}"#
    );
    fixture_decode_borrowed(inspect(&frame, &bounds())?.command)
}
fn begin() -> String {
    serde_json::json!({"operation":"begin","images":[[0,1,255]],"presentation":{
        "kind":"Advertise","schema":"conduit.body/member-return-images@2","signature":[7],
        "credential":{"credential_id":"credential/test","body_id":"body/test","part_id":"part/test","host_id":"host/test","boot_id":"boot/test","issued_at_millis":0},
        "projection":{"context":{"protocol_version":1,"host_id":"host/test","boot_id":"boot/test","offer_generation":1,"profile":"fixture/profile","resources":[],"capabilities":[]},"capability_images":[],"authority_grants":[]}
    }}).to_string()
}
#[test]
fn real_command_and_nested_types_match_legacy_acceptance_and_encoding() {
    let begin = begin();
    let complete=serde_json::json!({"operation":"complete","proof":{"admission_id":"return/test","body_id":"body/test","part_id":"part/test","host_id":"host/test","boot_id":"boot/test","nonce":vec![1;32],"signature":vec![7;64]}}).to_string();
    for input in [
        begin.clone(),
        complete.clone(),
        begin.replace("\"begin\"", "\"be\\u0067in\""),
        begin.replace("\"images\":[[0,1,255]]", "\"images\":[[-0]]"),
        begin.replace("\"images\":[[0,1,255]]", "\"images\":[[256]]"),
        begin.replace(
            "\"operation\":\"begin\"",
            "\"operation\":\"begin\",\"proof\":null",
        ),
        begin.replace(
            "\"credential_id\":\"credential/test\"",
            "\"credential_id\":null",
        ),
        complete.replace("\"proof\":{", "\"proof\":null,\"unknown\":{"),
    ] {
        let old = serde_json::from_str::<Command>(&input);
        let new = decode(&input);
        assert_eq!(new.is_ok(), old.is_ok(), "{input}");
        if let (Ok(old), Ok(new)) = (old, new) {
            assert_eq!(
                serde_json::to_vec(&new).unwrap(),
                serde_json::to_vec(&old).unwrap()
            );
        }
    }
    assert!(decode(&begin).is_ok());
    assert!(decode(&complete).is_ok());
}
#[test]
fn real_command_sequence_variants_remain_supported() {
    for input in [begin(),serde_json::json!({"operation":"complete","proof":{"admission_id":"return/test","body_id":"body/test","part_id":"part/test","host_id":"host/test","boot_id":"boot/test","nonce":vec![1;32],"signature":vec![7;64]}}).to_string()] {
        let value:serde_json::Value=serde_json::from_str(&input).unwrap();
        let sequence=if value["operation"]=="begin" {
            serde_json::json!(["begin",value["presentation"],value["images"]])
        } else {serde_json::json!(["complete",value["proof"]])};
        let input=sequence.to_string();
        let old:Command=serde_json::from_str(&input).unwrap();
        let new=decode(&input).unwrap();
        assert_eq!(serde_json::to_vec(&new).unwrap(),serde_json::to_vec(&old).unwrap());
    }
}
