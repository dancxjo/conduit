use alloc::{format, string::String, vec::Vec};
use conduit_core::{BoundedResourceRef, ConfigurationValue, PlannedGear};
pub fn snapshot_reference(placement: &PlannedGear) -> Result<BoundedResourceRef, String> {
    let [configuration] = placement.configuration.as_slice() else {
        return Err("snapshot requires one reference".into());
    };
    let ConfigurationValue::Text(hex) = &configuration.value else {
        return Err("snapshot reference is not text".into());
    };
    if configuration.key != "reference" || hex.is_empty() || hex.len() > 1024 || hex.len() % 2 != 0
    {
        return Err("snapshot reference encoding exceeds its bound".into());
    }
    let bytes = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |byte| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            };
            Ok((digit(pair[0]).ok_or("noncanonical reference hex")? << 4)
                | digit(pair[1]).ok_or("noncanonical reference hex")?)
        })
        .collect::<Result<Vec<u8>, &str>>()?;
    let reference = BoundedResourceRef::decode(&bytes).map_err(|error| format!("{error:?}"))?;
    if reference.content_profile.as_str() != conduit_web::JSON_TEXT_INFO_ID {
        return Err("snapshot reference has the wrong content profile".into());
    }
    Ok(reference)
}
