//! Binary byte acceleration must retain ordinary configuration wire admission.
use conduit_core::{
    encode_count, kind_id, KindId, StructuredConfigurationValue, StructuredInfoType,
    StructuredInfoValue,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Legacy {
    profile: KindId,
    canonical_value: Vec<u8>,
}
fn valid() -> Legacy {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let value = StructuredInfoValue::leaf(ty.clone(), encode_count(7).to_vec()).unwrap();
    Legacy {
        profile: ty.profile().unwrap().value_kind().clone(),
        canonical_value: value.canonical_bytes().unwrap(),
    }
}
#[test]
fn unchanged_json_and_postcard_sequences_preserve_exact_validation() {
    let original = valid();
    let configurations = [
        valid(),
        Legacy {
            profile: kind_id("wrong/profile"),
            ..valid()
        },
        Legacy {
            canonical_value: vec![],
            ..valid()
        },
        Legacy {
            canonical_value: vec![255],
            ..valid()
        },
        Legacy {
            profile: kind_id(""),
            ..valid()
        },
    ];
    for encoded in configurations {
        let expected = StructuredConfigurationValue::new(
            encoded.profile.clone(),
            encoded.canonical_value.clone(),
        );
        let json = serde_json::to_vec(&encoded).unwrap();
        let binary = postcard::to_allocvec(&encoded).unwrap();
        assert_eq!(
            serde_json::from_slice::<StructuredConfigurationValue>(&json).ok(),
            expected
        );
        assert_eq!(
            postcard::from_bytes::<StructuredConfigurationValue>(&binary).ok(),
            expected
        );
        if let Some(value) = expected {
            assert_eq!(serde_json::to_vec(&value).unwrap(), json);
            assert_eq!(postcard::to_allocvec(&value).unwrap(), binary);
        }
    }
    let mut wrong = serde_json::to_value(original).unwrap();
    wrong["canonical_value"] = "this remains a string, not a byte sequence".into();
    assert!(serde_json::from_value::<StructuredConfigurationValue>(wrong).is_err());
}
